//! Bounded causal probes: two archived Development maps, both legs, two
//! arms per contrast. Policy ids/RNG streams/spawns never change between arms.
#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

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
    types::{AgentId, Faction},
};
use arena_replay::{
    canon,
    recorder::ReplayWriter,
    replay::{decode_log, replay},
};
use arena_runner::balance::record::{BalanceMatchRecord, parse_balance_tsv};

#[derive(Clone, Copy, Debug)]
enum Contrast {
    Second,
    Defense,
    Attack,
}

impl Contrast {
    fn tag(self) -> &'static str {
        match self {
            Self::Second => "second",
            Self::Defense => "defense",
            Self::Attack => "attack",
        }
    }
    fn subject(self) -> PolicyId {
        match self {
            Self::Second | Self::Defense => PolicyId::BuilderDefenderR2,
            Self::Attack => PolicyId::BuilderDisruptorR2,
        }
    }
    fn specs(self, row: &BalanceMatchRecord, enabled: bool) -> (PolicySpec, PolicySpec) {
        let mut a = PolicySpec {
            policy_id: row.policy_a,
            family: row.policy_a_family,
            version: R2_VERSION,
            parameters: row.policy_a_parameters,
        };
        let mut b = PolicySpec {
            policy_id: row.policy_b,
            family: row.policy_b_family,
            version: R2_VERSION,
            parameters: row.policy_b_parameters,
        };
        if !enabled {
            match self {
                Self::Second => match &mut a.parameters {
                    PolicyParameters::BuilderDefender(p) => {
                        p.core.second_plan_discount_permille = 0
                    }
                    _ => unreachable!(),
                },
                Self::Defense => match &mut a.parameters {
                    PolicyParameters::BuilderDefender(p) => p.defense_task_cap = 0,
                    _ => unreachable!(),
                },
                Self::Attack => match &mut b.parameters {
                    PolicyParameters::BuilderDisruptor(p) => p.attack_share_permille = 0,
                    _ => unreachable!(),
                },
            }
        }
        (a, b)
    }
}

#[derive(Default, Clone, Debug)]
struct AgentStats {
    rounds: u64,
    grants: u64,
    submitted: u64,
    ok: u64,
    changed: u64,
    no_candidates: u64,
    reservation_conflict: u64,
    moving: u64,
    stationary: u64,
    captures: u64,
    links: u64,
    fields: u64,
    attacks: u64,
    neutralized: u64,
    recharges: u64,
    hp_restored: u64,
    supply_spent: u64,
    hacks: u64,
    transfers: u64,
}

impl AgentStats {
    fn columns() -> &'static str {
        "agent_rounds\tgrants\tsubmitted\tok_action\tstate_change\tno_candidates\treservation_conflict\tmoving\tstationary\tcaptures\tlinks\tfields_created\tattacks\tneutralized\trecharges\thp_restored\tsupply_spent\thacks\ttransfers"
    }
    fn values(&self) -> String {
        [
            self.rounds,
            self.grants,
            self.submitted,
            self.ok,
            self.changed,
            self.no_candidates,
            self.reservation_conflict,
            self.moving,
            self.stationary,
            self.captures,
            self.links,
            self.fields,
            self.attacks,
            self.neutralized,
            self.recharges,
            self.hp_restored,
            self.supply_spent,
            self.hacks,
            self.transfers,
        ]
        .map(|v| v.to_string())
        .join("\t")
    }
    fn add(&mut self, other: &Self) {
        self.rounds += other.rounds;
        self.grants += other.grants;
        self.submitted += other.submitted;
        self.ok += other.ok;
        self.changed += other.changed;
        self.no_candidates += other.no_candidates;
        self.reservation_conflict += other.reservation_conflict;
        self.moving += other.moving;
        self.stationary += other.stationary;
        self.captures += other.captures;
        self.links += other.links;
        self.fields += other.fields;
        self.attacks += other.attacks;
        self.neutralized += other.neutralized;
        self.recharges += other.recharges;
        self.hp_restored += other.hp_restored;
        self.supply_spent += other.supply_spent;
        self.hacks += other.hacks;
        self.transfers += other.transfers;
    }
}

struct Files {
    matches: BufWriter<File>,
    agents: BufWriter<File>,
    decisions: BufWriter<File>,
    plans: BufWriter<File>,
    events: BufWriter<File>,
    replays: BufWriter<File>,
}

impl Files {
    fn new(out: &Path) -> Result<Self, Box<dyn Error>> {
        fs::create_dir_all(out.join("replays"))?;
        let open = |name: &str| -> Result<BufWriter<File>, std::io::Error> {
            Ok(BufWriter::new(File::create(out.join(name))?))
        };
        let mut files = Self {
            matches: open("matches.tsv")?,
            agents: open("agents.tsv")?,
            decisions: open("decisions.tsv")?,
            plans: open("plans.tsv")?,
            events: open("events.tsv")?,
            replays: open("replays.tsv")?,
        };
        writeln!(
            files.matches,
            "contrast\tarm\tsource_index\tmap_id\tleg\tsubject\tsubject_faction\tversion\tgame_seed\tbot_seed\tsubject_score2\topponent_score2\tsubject_margin\tfinal_hash\tpolicy_a_parameters\tpolicy_b_parameters\t{}",
            AgentStats::columns()
        )?;
        writeln!(
            files.agents,
            "match\tfaction\tagent\t{}",
            AgentStats::columns()
        )?;
        writeln!(
            files.decisions,
            "match\tround\tfaction\tagent\tcandidates\ttask\tportal\tsource\ttarget\treceiver\tkey\tsubmitted\tok\tstate_change\tsupply_before\tsupply_after"
        )?;
        writeln!(
            files.plans,
            "match\tround\tfaction\tprimary\tsecondary\tscore2_delta"
        )?;
        writeln!(files.events, "match\tround\tevent\tfaction\tagent\tobject")?;
        writeln!(
            files.replays,
            "match\tfile\tblake3\tsource_file\tsource_blake3\trules_verified"
        )?;
        Ok(files)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.matches.flush()?;
        self.agents.flush()?;
        self.decisions.flush()?;
        self.plans.flush()?;
        self.events.flush()?;
        self.replays.flush()
    }
}

fn faction_tag(f: Faction) -> &'static str {
    match f {
        Faction::Enl => "enl",
        Faction::Res => "res",
    }
}

fn task_columns(task: Option<Task>) -> String {
    let mut values = [
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    values[0] = task.map_or("none", |t| t.kind_tag()).to_string();
    match task {
        Some(
            Task::Capture { portal }
            | Task::KeyFarm { portal }
            | Task::Recharge { portal }
            | Task::Standby { portal }
            | Task::Attack { portal },
        ) => values[1] = portal.0.to_string(),
        Some(Task::Link { source, target }) => {
            values[2] = source.0.to_string();
            values[3] = target.0.to_string();
        }
        Some(Task::Transfer { receiver, key }) => {
            values[4] = receiver.0.to_string();
            values[5] = key.0.to_string();
        }
        None => {}
    }
    values.join("\t")
}

fn run(
    archive: &Path,
    out: &Path,
    files: &mut Files,
    row: &BalanceMatchRecord,
    source: &str,
    source_hash: &str,
    contrast: Contrast,
    enabled: bool,
) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(archive.join(source))?;
    assert_eq!(
        blake3::hash(&bytes).to_hex().as_str(),
        source_hash,
        "source replay digest"
    );
    let log = decode_log(&bytes)?;
    let h = &log.header;
    assert_eq!(h.game_seed, row.game_seed);
    assert_eq!(h.config, arena_core::config::GameConfig::default());
    assert_eq!(
        row.bucket_policy,
        arena_runner::policy::BucketPolicy::Flat(0)
    );
    let (a, b) = contrast.specs(row, enabled);
    let (enl_spec, res_spec) = if row.policy_a_is_enl {
        (&a, &b)
    } else {
        (&b, &a)
    };
    let mut enl_player =
        make_policy_player(enl_spec, Faction::Enl, row.bot_seed, &h.config, &h.map);
    let mut res_player =
        make_policy_player(res_spec, Faction::Res, row.bot_seed, &h.config, &h.map);
    let (enl_roster, res_roster) = GameState::initial_roster(&h.config);
    let enl_entries = enl_player.choose_spawns(&h.config, &h.map, &enl_roster);
    let res_entries = res_player.choose_spawns(&h.config, &h.map, &res_roster);
    assert_eq!(enl_entries, h.enl_spawns, "fixed ENL initialization");
    assert_eq!(res_entries, h.res_spawns, "fixed RES initialization");
    let enl = validate_initial_spawn_set(&enl_roster, &enl_entries, &h.config)
        .map_err(|e| format!("ENL spawns: {e:?}"))?;
    let res = validate_initial_spawn_set(&res_roster, &res_entries, &h.config)
        .map_err(|e| format!("RES spawns: {e:?}"))?;
    let mut state =
        GameState::new(h.config, &h.map, &enl, &res).map_err(|e| format!("state: {e:?}"))?;
    let resolver = Resolver::with_seed(h.game_seed);
    let mut writer = ReplayWriter::new(h.game_seed, &h.config, &h.map, &enl, &res);
    let arm = if enabled { "on" } else { "off" };
    let name = format!("{}-{arm}-{}", contrast.tag(), row.index);
    let subject = if enl_spec.policy_id == contrast.subject() {
        Faction::Enl
    } else {
        Faction::Res
    };
    let mut agents: BTreeMap<(Faction, AgentId), AgentStats> = BTreeMap::new();
    for _ in 0..h.config.total_rounds {
        let round = state.round();
        let observations = [
            Observation::for_faction(&state, Faction::Enl),
            Observation::for_faction(&state, Faction::Res),
        ];
        let submissions = [
            enl_player.submit_round(&observations[0]),
            res_player.submit_round(&observations[1]),
        ];
        let traces = [
            enl_player
                .decision_trace()
                .expect("instrumented policy")
                .clone(),
            res_player
                .decision_trace()
                .expect("instrumented policy")
                .clone(),
        ];
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
        let previous_events = state.events().len();
        let report = resolver.run_round(&mut state, &streams);
        for (index, faction) in [Faction::Enl, Faction::Res].into_iter().enumerate() {
            let trace = &traces[index];
            if !enabled && matches!(contrast, Contrast::Second) && faction == subject {
                assert!(
                    trace.second.is_none(),
                    "second-plan off arm must stay disabled"
                );
            }
            let plan_tag = |p: Option<arena_bots::r2::value::FieldCandidate>| {
                p.map(|p| p.vertices.map(|v| v.0.to_string()).join(","))
                    .unwrap_or_default()
            };
            writeln!(
                files.plans,
                "{name}\t{round}\t{}\t{}\t{}\t{}",
                faction_tag(faction),
                plan_tag(trace.active),
                plan_tag(trace.second),
                report.score2_delta[&faction]
            )?;
            for me in &observations[index].own_agents {
                let decision = trace
                    .agents
                    .iter()
                    .find(|a| a.agent == me.id)
                    .expect("pre-round roster trace");
                let submitted = streams
                    .iter()
                    .any(|s| s.faction == faction && s.agent_id == me.id && !s.commands.is_empty());
                let mut delta = AgentStats {
                    rounds: 1,
                    grants: u64::from(decision.grant.is_some()),
                    submitted: u64::from(submitted),
                    no_candidates: u64::from(
                        decision.grant.is_none() && decision.candidate_count == 0,
                    ),
                    reservation_conflict: u64::from(
                        decision.grant.is_none() && decision.candidate_count > 0,
                    ),
                    ..AgentStats::default()
                };
                for outcome in report
                    .execution_order
                    .iter()
                    .filter(|s| s.faction == faction && s.agent_id == me.id)
                    .flat_map(|s| &s.outcomes)
                {
                    if let CommandOutcome::Ok(effect) = outcome {
                        delta.ok = 1;
                        match effect {
                            CommandEffect::Moved { from, to, .. } => {
                                if from != to {
                                    delta.moving = 1;
                                    delta.changed = 1;
                                } else {
                                    delta.stationary = 1;
                                }
                            }
                            CommandEffect::Captured { .. } => {
                                delta.captures += 1;
                                delta.changed = 1;
                            }
                            CommandEffect::Linked { fields, .. } => {
                                delta.links += 1;
                                delta.fields += fields.len() as u64;
                                delta.changed = 1;
                            }
                            CommandEffect::Attacked { hits } => {
                                delta.attacks += 1;
                                delta.neutralized +=
                                    hits.iter().filter(|h| h.neutralized).count() as u64;
                                delta.changed |= u64::from(!hits.is_empty());
                            }
                            CommandEffect::Recharged {
                                hp_before,
                                hp_after,
                                ..
                            } => {
                                delta.recharges += 1;
                                delta.hp_restored += (hp_after - hp_before) as u64;
                                delta.supply_spent += h.config.recharge_cost_supply as u64;
                                delta.changed |= u64::from(hp_after > hp_before);
                            }
                            CommandEffect::Hacked {
                                keys_gained,
                                supply_granted,
                                ..
                            } => {
                                delta.hacks += 1;
                                delta.changed |= u64::from(*keys_gained > 0 || *supply_granted > 0);
                            }
                            CommandEffect::Transferred { supply, keys, .. } => {
                                delta.transfers += 1;
                                delta.changed |=
                                    u64::from(*supply > 0 || keys.values().any(|&n| n > 0));
                            }
                        }
                    }
                }
                writeln!(
                    files.decisions,
                    "{name}\t{round}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    faction_tag(faction),
                    me.id.0,
                    decision.candidate_count,
                    task_columns(decision.grant),
                    delta.submitted,
                    delta.ok,
                    delta.changed,
                    me.supply,
                    state.agent(me.id).unwrap().supply
                )?;
                agents.entry((faction, me.id)).or_default().add(&delta);
            }
        }
        for event in &state.events()[previous_events..] {
            let (kind, faction, agent, object) = match &event.kind {
                EventKind::PortalNeutralized {
                    faction,
                    agent_id,
                    portal,
                } => (
                    "neutralized",
                    *faction,
                    agent_id.0.to_string(),
                    portal.0.to_string(),
                ),
                EventKind::FieldCreated {
                    faction,
                    agent_id,
                    field,
                } => (
                    "field-created",
                    *faction,
                    agent_id.0.to_string(),
                    [field.a, field.b, field.c]
                        .map(|p| p.0.to_string())
                        .join(","),
                ),
                EventKind::FieldDestroyed {
                    faction,
                    agent_id,
                    field,
                } => (
                    "field-destroyed-by",
                    *faction,
                    agent_id.0.to_string(),
                    [field.a, field.b, field.c]
                        .map(|p| p.0.to_string())
                        .join(","),
                ),
                _ => continue,
            };
            writeln!(
                files.events,
                "{name}\t{round}\t{kind}\t{}\t{agent}\t{object}",
                faction_tag(faction)
            )?;
        }
        state.advance_round();
        writer.record_round(&report, &state)?;
    }
    let replay_bytes = writer.finish(&state)?;
    let verified = replay(&decode_log(&replay_bytes)?)?;
    assert_eq!(verified.final_state_hash, canon::state_hash(&state));
    fs::write(
        out.join("replays").join(format!("{name}.iarp")),
        &replay_bytes,
    )?;
    writeln!(
        files.replays,
        "{name}\treplays/{name}.iarp\t{}\t{source}\t{source_hash}\ttrue",
        blake3::hash(&replay_bytes).to_hex()
    )?;
    let mut totals = AgentStats::default();
    for ((faction, agent), stats) in &agents {
        writeln!(
            files.agents,
            "{name}\t{}\t{}\t{}",
            faction_tag(*faction),
            agent.0,
            stats.values()
        )?;
        if *faction == subject {
            totals.add(stats);
        }
    }
    assert_eq!(totals.rounds, 2500);
    if !enabled {
        match contrast {
            Contrast::Defense => assert_eq!(totals.recharges, 0),
            Contrast::Attack => assert_eq!(totals.attacks, 0),
            Contrast::Second => {}
        }
    }
    let score = state.score(subject);
    let opponent = state.score(subject.opposite());
    writeln!(
        files.matches,
        "{}\t{arm}\t{}\t{}\t{}\t{}\t{}\t{R2_VERSION}\t{}\t{}\t{score}\t{opponent}\t{}\t{}\t{}\t{}\t{}",
        contrast.tag(),
        row.index,
        row.map_id,
        row.leg,
        contrast.subject().tag(),
        faction_tag(subject),
        h.game_seed,
        row.bot_seed,
        score as i128 - opponent as i128,
        canon::hex(&verified.final_state_hash),
        a.parameters.tag(),
        b.parameters.tag(),
        totals.values()
    )?;
    files.flush()?;
    eprintln!(
        "{name}: subject margin {} ; {} / {} task rounds; replay verified",
        score as i128 - opponent as i128,
        totals.grants,
        totals.rounds
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    assert_eq!(
        R2_VERSION, "r2.2",
        "use the frozen source for this experiment version"
    );
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: r22-micro <r21-archive-directory> <new-output-directory>".into());
    }
    let archive = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    if out.exists() {
        return Err("output already exists; choose a new directory".into());
    }
    let records = parse_balance_tsv(&fs::read_to_string(archive.join("matches.tsv"))?)?;
    let selected: Vec<_> = records
        .iter()
        .filter(|r| {
            r.map_index < 2
                && r.variant_id == "default"
                && r.policy_a == PolicyId::BuilderDefenderR2
                && r.policy_b == PolicyId::BuilderDisruptorR2
        })
        .collect();
    assert_eq!(selected.len(), 4, "two maps times both legs");
    assert_eq!(
        selected
            .iter()
            .map(|row| (row.map_index, row.leg))
            .collect::<std::collections::BTreeSet<_>>(),
        [(0, 0), (0, 1), (1, 0), (1, 1)].into_iter().collect(),
        "one cell for each map and leg"
    );
    for row in &selected {
        assert_eq!(row.policy_a_version, "r2.1");
        assert_eq!(row.policy_b_version, "r2.1");
        assert_eq!(row.seed_set_tag, "development");
    }
    let replay_text = fs::read_to_string(archive.join("replays.tsv"))?;
    let mut lines = replay_text.lines();
    let columns: Vec<_> = lines
        .next()
        .ok_or("empty replay mapping")?
        .split('\t')
        .collect();
    let index = |name: &str| {
        columns
            .iter()
            .position(|&s| s == name)
            .expect("replay mapping column")
    };
    let mut mapping = BTreeMap::new();
    for line in lines {
        let cells: Vec<_> = line.split('\t').collect();
        if cells[index("status")] == "captured" {
            mapping.insert(
                cells[index("cell_index")].parse::<u64>()?,
                (
                    cells[index("file")].to_string(),
                    cells[index("blake3")].to_string(),
                ),
            );
        }
    }
    let mut files = Files::new(out)?;
    fs::write(
        out.join("manifest.txt"),
        format!(
            "status=running\nversion={R2_VERSION}\nsource_archive={}\ncontrasts=second,defense,attack\nmaps=development-000,development-001\nmatches_per_contrast=8\ntotal_matches=24\nseeds_and_spawns=exact archived inputs, asserted for every arm\n",
            archive.display()
        ),
    )?;
    for contrast in [Contrast::Second, Contrast::Defense, Contrast::Attack] {
        for row in &selected {
            let (source, hash) = &mapping[&row.index];
            for enabled in [false, true] {
                run(
                    archive, out, &mut files, row, source, hash, contrast, enabled,
                )?;
            }
        }
    }
    files.flush()?;
    let manifest = fs::read_to_string(out.join("manifest.txt"))?;
    fs::write(
        out.join("manifest.txt"),
        manifest.replace("status=running", "status=complete\nrules_verified=24"),
    )?;
    Ok(())
}
