//! Declarative events: stable IDs, content-addressed maps, explicit paired samples.
use super::{
    archive::Json, document::*, durable::Plan, exec_external::ClientLaunch, maps::MapAsset, *,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

pub(crate) fn launches(extension: Option<&Json>) -> io::Result<Vec<ClientLaunch>> {
    let Some(ext) = extension else {
        return Ok(Vec::new());
    };
    array(ext, "entrants")?
        .iter()
        .map(|e| {
            Ok(ClientLaunch {
                registered_id: text(e, "entrant_id")?.into(),
                display_name: text(e, "name")?.into(),
                args: array(e, "args")?
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| invalid("argv must be string"))
                    })
                    .collect::<io::Result<_>>()?,
            })
        })
        .collect()
}

/// Resolve all external inputs and validate before any archive is created.
pub fn resolve(spec: &Path) -> io::Result<Plan> {
    let j = read(spec)?;
    keys(
        &j,
        &[
            "schema",
            "name",
            "seed",
            "mode",
            "map_set",
            "entrants",
            "schedule",
            "resources",
            "attempt_limit",
        ],
    )?;
    if text(&j, "schema")? != "arena-event-v1" {
        return Err(invalid("unsupported event schema"));
    }
    let base = spec.parent().unwrap_or(Path::new("."));
    let mode = text(&j, "mode")?;
    if !matches!(mode, "functional" | "timing" | "competition") {
        return Err(invalid("mode must be functional, timing or competition"));
    }
    let set = read(&base.join(text(&j, "map_set")?))?;
    if text(&set, "schema")? != "arena-map-set-v1" {
        return Err(invalid("unsupported map set"));
    }
    let mut assets = array(&set, "maps")?
        .iter()
        .map(|v| MapAsset::from_json(&encode(v)))
        .collect::<io::Result<Vec<_>>>()?;
    if assets.is_empty() {
        return Err(invalid("map set is empty"));
    }
    assets.sort_by_key(|a| {
        (
            schedule::map_codec_hash(&a.map),
            config::config_identity(&a.config),
        )
    });
    let ids = assets
        .iter()
        .map(|a| {
            format!(
                "{}:{:016x}",
                schedule::map_codec_hash(&a.map),
                config::config_identity(&a.config)
            )
        })
        .collect::<Vec<_>>();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(invalid("duplicate map/config content"));
    }
    if text(&set, "content_id")? != blake3::hash(ids.join("\n").as_bytes()).to_hex().as_str() {
        return Err(invalid("map set content identity mismatch"));
    }
    let mut entries = array(&j, "entrants")?.to_vec();
    entries.sort_by_key(|e| text(e, "id").unwrap_or("").to_owned());
    let mut entrants = Vec::new();
    let mut metadata = Vec::new();
    let mut by_id = BTreeMap::new();
    for entry in entries {
        keys(&entry, &["id", "name", "build", "args", "env"])?;
        let id = text(&entry, "id")?;
        identifier(id)?;
        if by_id.insert(id.to_owned(), entrants.len() as u32).is_some() {
            return Err(invalid("duplicate entry ID"));
        }
        let display = text(&entry, "name")?;
        if display.is_empty() || display.len() > 4096 {
            return Err(invalid("display name must be 1..4096 bytes"));
        }
        let build = base.join(text(&entry, "build")?).canonicalize()?;
        let manifest = builds::inspect(&build)?;
        let request = manifest
            .get("request")
            .ok_or_else(|| invalid("missing build request"))?;
        let audit = request
            .get("audit")
            .ok_or_else(|| invalid("missing audit"))?;
        if mode != "functional" && text(audit, "status")? != "pass" {
            return Err(invalid(
                "timing/competition require operator-recorded passing audit",
            ));
        }
        let args = array(&entry, "args")?;
        if args.len() > 128
            || args.iter().any(|v| {
                v.as_str()
                    .is_none_or(|s| s.contains('\0') || s.len() > 4096)
            })
        {
            return Err(invalid("invalid structured argv"));
        }
        let env = entry
            .get("env")
            .and_then(Json::as_object)
            .ok_or_else(|| invalid("env must be object"))?;
        let mut seen = BTreeSet::new();
        let mut variables = Vec::new();
        for (key, value) in env {
            if !(key.starts_with("ARENA_") || key.starts_with("PROBE_"))
                || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || !seen.insert(key)
            {
                return Err(invalid(
                    "env requires unique ARENA_/PROBE_ keys allowed by sandbox",
                ));
            }
            let value = value
                .as_str()
                .filter(|s| !s.contains('\0'))
                .ok_or_else(|| invalid("invalid env value"))?;
            variables.push((key.clone(), value.into()));
        }
        entrants.push(Entrant {
            name: id.into(),
            kind: EntrantKind::External {
                binary: build.join("binary"),
                env: variables,
            },
            provenance: EntrantProvenance {
                binary_blake3: Some(text(&manifest, "binary_hash")?.into()),
                tree_hash: Some(text(request, "source_tree")?.into()),
                lock_hash: Some(text(request, "lock_hash")?.into()),
                toolchain_hash: Some(text(request, "toolchain")?.into()),
                manifest_id: Some(fs::read_to_string(build.join("identity"))?),
                bot_version: Some(text(request, "version")?.into()),
            },
        });
        metadata.push(object(vec![
            ("id", string(id)),
            ("entrant_id", string(text(request, "entrant_id")?)),
            ("name", string(display)),
            ("build", string(build.display())),
            ("args", Json::Array(args.to_vec())),
            ("manifest", manifest),
        ]));
    }
    if entrants.len() < 2 {
        return Err(invalid("at least two entries required"));
    }
    let resources = j
        .get("resources")
        .ok_or_else(|| invalid("resources required"))?;
    keys(
        resources,
        &[
            "workers",
            "per_match_budget_ms",
            "sandbox",
            "address_space_bytes",
        ],
    )?;
    let budget = number(resources, "per_match_budget_ms")? as u64;
    if number(resources, "address_space_bytes")? == 0 {
        return Err(invalid("address_space_bytes must be positive"));
    }
    let sandbox = match resources.get("sandbox") {
        Some(Json::Null) => ClientSandbox::Off,
        Some(Json::Str(p)) => ClientSandbox::SandboxRunner {
            runner: base.join(p).canonicalize()?,
            wall_ms: budget,
            address_space_bytes: number(resources, "address_space_bytes")? as u64,
        },
        _ => return Err(invalid("sandbox must be path or null")),
    };
    if mode == "competition" && matches!(sandbox, ClientSandbox::Off) {
        return Err(invalid("competition mode requires sandbox"));
    }
    let mut cfg = TournamentConfig {
        name: text(&j, "name")?.into(),
        master_seed: seed(&j, "seed")?,
        entrants,
        matchups: Vec::new(),
        map_count: assets.len() as u32,
        rounds_per_match: assets[0].config.total_rounds,
        config: assets[0].config,
        elo: EloConfig::default(),
        concurrency: number(resources, "workers")? as usize,
        sandbox,
        per_match_budget_ms: budget,
    };
    let policy = j
        .get("schedule")
        .ok_or_else(|| invalid("schedule required"))?;
    keys(
        policy,
        &[
            "kind",
            "repeats",
            "candidates",
            "baselines",
            "before",
            "after",
        ],
    )?;
    let repeats = number(policy, "repeats")?;
    if !(1..=10000).contains(&repeats) {
        return Err(invalid("repeats must be 1..10000"));
    }
    let lookup = |name: &str| {
        by_id
            .get(name)
            .copied()
            .ok_or_else(|| invalid(format!("unknown entry {name}")))
    };
    let list = |key: &str| -> io::Result<Vec<u32>> {
        let ids = array(policy, key)?
            .iter()
            .map(|v| {
                lookup(
                    v.as_str()
                        .ok_or_else(|| invalid("entry ID must be string"))?,
                )
            })
            .collect::<io::Result<Vec<_>>>()?;
        if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(invalid("entry set is empty or has duplicates"));
        }
        Ok(ids)
    };
    // A pair's sample namespace omits the candidate version only for explicit
    // before/after comparisons. Other events use the ordered stable entry IDs.
    let kind = text(policy, "kind")?;
    let mut pairs = Vec::new();
    match kind {
        "round-robin" => {
            for a in 0..cfg.entrants.len() as u32 {
                for b in a + 1..cfg.entrants.len() as u32 {
                    pairs.push((a, b, "single"));
                }
            }
        }
        "challenge" => {
            for a in list("candidates")? {
                for b in list("baselines")? {
                    pairs.push((a, b, "single"));
                }
            }
        }
        "comparison" => {
            let a = lookup(text(policy, "before")?)?;
            let b = lookup(text(policy, "after")?)?;
            if a == b
                || text(&metadata[a as usize], "entrant_id")?
                    != text(&metadata[b as usize], "entrant_id")?
            {
                return Err(invalid(
                    "before/after must be distinct entries of the same entrant",
                ));
            }
            for baseline in list("baselines")? {
                pairs.push((a, baseline, "before"));
                pairs.push((b, baseline, "after"));
            }
        }
        _ => return Err(invalid("unsupported schedule kind")),
    }
    pairs.sort();
    let total = pairs
        .len()
        .checked_mul(assets.len())
        .and_then(|n| n.checked_mul(repeats as usize))
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| invalid("schedule size overflow"))?;
    if total > 100000 {
        return Err(invalid("plan exceeds 100000 matches"));
    }
    let mut items = Vec::new();
    let mut samples = Vec::new();
    let mut seen = BTreeSet::new();
    for (a, b, variant) in pairs {
        if a == b || !seen.insert((a.min(b), a.max(b))) {
            return Err(invalid(
                "self/duplicate matchup or overlapping candidate/baseline sets",
            ));
        }
        let aid = if kind == "comparison" {
            text(&metadata[a as usize], "entrant_id")?
        } else {
            &cfg.entrant(a).name
        }
        .to_owned();
        let bid = cfg.entrant(b).name.clone();
        let tag = blake3::hash(
            encode(&Json::Array(vec![
                string(&cfg.entrant(a).name),
                string(&bid),
            ]))
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        cfg.matchups.push(TMatchup::new(
            &tag,
            a,
            b,
            (assets.len() as u32) * repeats * 2,
        ));
        for (mi, asset) in assets.iter().enumerate() {
            for repeat in 0..repeats {
                let address = encode(&Json::Array(vec![
                    string(&aid),
                    string(&bid),
                    string(&ids[mi]),
                    string(cfg.master_seed),
                    Json::Int(repeat as i64),
                ]));
                let game_seed = crate::derive_seed(b"arena-event-game-v1", address.as_bytes());
                let client_seed = crate::derive_seed(b"arena-event-client-v1", address.as_bytes());
                let pair_id = blake3::hash(address.as_bytes()).to_hex().to_string();
                for leg in 0..2 {
                    samples.push(object(vec![
                        ("pair", string(&pair_id)),
                        ("variant", string(variant)),
                        ("leg", Json::Int(leg)),
                        ("repeat", Json::Int(repeat as i64)),
                        ("candidate", string(&cfg.entrant(a).name)),
                        ("baseline", string(&bid)),
                    ]));
                    items.push(TournamentItem {
                        index: items.len() as u64,
                        tag: tag.clone(),
                        j: (mi as u32 * repeats + repeat) * 2 + leg as u32,
                        a,
                        b,
                        a_is_enl: leg == 0,
                        map_index: mi as u32,
                        map_seed: asset
                            .source
                            .get("seed")
                            .and_then(Json::as_str)
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                        map_hash: schedule::map_codec_hash(&asset.map),
                        game_seed,
                        client_seed,
                        schedule_seed: crate::derive_seed(
                            b"arena-event-schedule-v1",
                            &cfg.master_seed.to_le_bytes(),
                        ),
                        config_hash: config::config_identity(&asset.config),
                    });
                }
            }
        }
    }
    cfg.validate().map_err(invalid)?;
    let attempt_limit = number(&j, "attempt_limit")?;
    if !(1..=100).contains(&attempt_limit) {
        return Err(invalid("attempt_limit must be 1..100"));
    }
    let extension = object(vec![
        ("schema", string("arena-frozen-event-v1")),
        ("scoring", string("normal-only-v1")),
        ("analysis", string("paired-descriptive-v1")),
        ("resources", resources.clone()),
        ("mode", string(mode)),
        ("schedule", policy.clone()),
        ("map_set", set),
        ("entrants", Json::Array(metadata)),
        (
            "maps",
            Json::Array(
                assets
                    .iter()
                    .map(|a| archive::parse_json(&a.to_json()).unwrap())
                    .collect(),
            ),
        ),
        ("samples", Json::Array(samples)),
    ]);
    Ok(Plan {
        map_configs: assets.iter().map(|a| a.config).collect(),
        config: cfg,
        items,
        maps: assets.into_iter().map(|a| a.map).collect(),
        attempt_limit,
        extension: Some(extension),
    })
}

pub fn preview(plan: &Plan) -> io::Result<String> {
    let mut out = format!(
        "matches={} maps={} workers={} per_match_budget_ms={} sandbox={}\n",
        plan.items.len(),
        plan.maps.len(),
        plan.config.concurrency,
        plan.config.per_match_budget_ms,
        plan.config.sandbox.tag()
    );
    if let Some(ext) = &plan.extension {
        let resources = ext
            .get("resources")
            .ok_or_else(|| invalid("resources missing"))?;
        out.push_str(&format!(
            "address_space_bytes={} address_space_enforced={}\n",
            number(resources, "address_space_bytes")?,
            !matches!(plan.config.sandbox, ClientSandbox::Off)
        ));
        out.push_str(&format!(
            "mode={} scoring=normal-only-v1 analysis=paired-descriptive-v1\n",
            text(ext, "mode")?
        ));
        for (i, m) in array(ext, "maps")?.iter().enumerate() {
            let c = document::config(
                m.get("config")
                    .ok_or_else(|| invalid("map rules missing"))?,
            )?;
            out.push_str(&format!("map={i} dimensions={}x{} portals={} min_distance={} rounds={} rules={:016x} tags={}\n",c.map_width_cm,c.map_height_cm,c.portal_count,c.portal_min_distance_cm,c.total_rounds,config::config_identity(&c),encode(m.get("tags").unwrap_or(&Json::Null))));
        }
        out.push_str(&format!(
            "schedule={}\n",
            encode(
                ext.get("schedule")
                    .ok_or_else(|| invalid("schedule missing"))?
            )
        ));
    }
    for pair in &plan.config.matchups {
        for (mi, map) in plan.maps.iter().enumerate() {
            let rows = plan
                .items
                .iter()
                .filter(|i| i.tag == pair.tag && i.map_index as usize == mi)
                .collect::<Vec<_>>();
            let enl = rows.iter().filter(|i| i.a_is_enl).count();
            out.push_str(&format!(
                "{} vs {} map={} hash={} matches={} A_ENL={} A_RES={} {}\n",
                plan.config.entrant(pair.a).name,
                plan.config.entrant(pair.b).name,
                mi,
                schedule::map_codec_hash(map),
                rows.len(),
                enl,
                rows.len() - enl,
                if rows.is_empty() { "UNCOVERED" } else { "" }
            ));
        }
    }
    Ok(out)
}
pub fn execute_cli(args: &[String]) -> io::Result<Option<String>> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    if !matches!(verb, "plan" | "tournament") || !args.iter().any(|s| s == "--spec") {
        return Ok(None);
    }
    let opts = assets_cli::options(&args[1..], &["--spec", "--out"])?;
    let plan = resolve(Path::new(assets_cli::required(&opts, "--spec")?))?;
    let preview = preview(&plan)?;
    let out = Path::new(assets_cli::required(&opts, "--out")?);
    fs::create_dir(out)?;
    durable::publish(plan, out)?;
    if verb == "tournament" {
        durable::execute(out, durable::Action::Resume)?;
    }
    Ok(Some(preview))
}
