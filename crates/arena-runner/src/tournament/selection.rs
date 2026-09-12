//! Clone frozen logical inputs into a new development run; never retry a winner.
use super::{archive::Json, document::*, *};
use std::{collections::BTreeSet, fs, io, path::Path};

pub fn execute_cli(args: &[String]) -> io::Result<Option<String>> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    if !matches!(verb, "select" | "compare-plan") {
        return Ok(None);
    }
    let opts = assets_cli::options(
        &args[1..],
        if verb == "select" {
            &["--from", "--items", "--out"]
        } else {
            &["--from", "--candidate", "--build", "--out"]
        },
    )?;
    let from = Path::new(assets_cli::required(&opts, "--from")?).canonicalize()?;
    let mut plan = durable::load(&from)?;
    let mut ext = plan
        .extension
        .take()
        .ok_or_else(|| invalid("selection/comparison requires a batch-two frozen event"))?;
    let mut entries = array(&ext, "entrants")?.to_vec();
    for (i, e) in entries.iter_mut().enumerate() {
        set(
            e,
            "build",
            string(from.join(format!("plan/builds/{i}")).display()),
        )?;
    }
    let old_samples = array(&ext, "samples")?.to_vec();
    let mut samples = Vec::new();
    let mut items = Vec::new();
    if verb == "select" {
        let selected = assets_cli::required(&opts, "--items")?
            .split(',')
            .map(|s| s.parse::<usize>().map_err(invalid))
            .collect::<io::Result<BTreeSet<_>>>()?;
        if selected.is_empty() || selected.iter().any(|&i| i >= plan.items.len()) {
            return Err(invalid("unknown/empty item subset"));
        }
        for i in selected {
            let mut item = plan.items[i].clone();
            item.index = items.len() as u64;
            items.push(item);
            samples.push(old_samples[i].clone());
        }
    } else {
        if text(
            ext.get("schedule")
                .ok_or_else(|| invalid("missing schedule"))?,
            "kind",
        )? == "comparison"
        {
            return Err(invalid("compare-plan source must be single-version event"));
        }
        let candidate = assets_cli::required(&opts, "--candidate")?;
        let id = plan
            .config
            .entrants
            .iter()
            .position(|e| e.name == candidate)
            .ok_or_else(|| invalid("unknown candidate"))? as u32;
        let build = Path::new(assets_cli::required(&opts, "--build")?).canonicalize()?;
        let manifest = builds::inspect(&build)?;
        let request = manifest
            .get("request")
            .ok_or_else(|| invalid("missing request"))?;
        if text(request, "entrant_id")? != text(&entries[id as usize], "entrant_id")? {
            return Err(invalid("replacement must be same entrant"));
        }
        if text(&ext, "mode")? != "functional"
            && text(
                request
                    .get("audit")
                    .ok_or_else(|| invalid("audit missing"))?,
                "status",
            )? != "pass"
        {
            return Err(invalid("replacement requires passing audit"));
        }
        let mut entrant = plan.config.entrants[id as usize].clone();
        entrant.name = format!("{candidate}-after");
        identifier(&entrant.name)?;
        if plan.config.entrants.iter().any(|e| e.name == entrant.name) {
            return Err(invalid("generated after entry ID already exists"));
        }
        let EntrantKind::External { binary, .. } = &mut entrant.kind;
        *binary = build.join("binary");
        entrant.provenance = EntrantProvenance {
            binary_blake3: Some(text(&manifest, "binary_hash")?.into()),
            bot_version: Some(text(request, "version")?.into()),
            tree_hash: Some(text(request, "source_tree")?.into()),
            lock_hash: Some(text(request, "lock_hash")?.into()),
            toolchain_hash: Some(text(request, "toolchain")?.into()),
            manifest_id: Some(fs::read_to_string(build.join("identity"))?),
        };
        let new_id = plan.config.entrants.len() as u32;
        let new_name = entrant.name.clone();
        plan.config.entrants.push(entrant);
        let mut entry = entries[id as usize].clone();
        set(&mut entry, "id", string(&new_name))?;
        set(&mut entry, "build", string(build.display()))?;
        set(&mut entry, "manifest", manifest)?;
        entries.push(entry);
        for (i, item) in plan
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.a == id || i.b == id)
        {
            let baseline = plan
                .config
                .entrant(if item.a == id { item.b } else { item.a })
                .name
                .clone();
            for after in [false, true] {
                let mut cloned = item.clone();
                cloned.index = items.len() as u64;
                let mut sample = old_samples[i].clone();
                set(
                    &mut sample,
                    "variant",
                    string(if after { "after" } else { "before" }),
                )?;
                set(
                    &mut sample,
                    "candidate",
                    string(if after { &new_name } else { candidate }),
                )?;
                set(&mut sample, "baseline", string(&baseline))?;
                if after {
                    if cloned.a == id {
                        cloned.a = new_id;
                    } else {
                        cloned.b = new_id;
                    }
                    cloned.tag.push_str("-after");
                }
                items.push(cloned);
                samples.push(sample);
            }
        }
        if items.is_empty() {
            return Err(invalid("candidate has no selected matches"));
        }
        set(
            &mut ext,
            "schedule",
            object(vec![
                ("kind", string("comparison")),
                ("before", string(candidate)),
                ("after", string(new_name)),
            ]),
        )?;
    }
    let mut matchups = Vec::<TMatchup>::new();
    for item in &items {
        if let Some(m) = matchups.iter_mut().find(|m| m.tag == item.tag) {
            m.count += 1;
        } else {
            matchups.push(TMatchup::new(&item.tag, item.a, item.b, 1));
        }
    }
    plan.config.matchups = matchups;
    plan.items = items;
    set(&mut ext, "entrants", Json::Array(entries))?;
    set(&mut ext, "samples", Json::Array(samples))?;
    set(
        &mut ext,
        "origin",
        object(vec![
            (
                "manifest",
                string(blake3::hash(&fs::read(from.join("plan/manifest"))?).to_hex()),
            ),
            ("operation", string(verb)),
        ]),
    )?;
    plan.extension = Some(ext);
    let preview = event::preview(&plan)?;
    let out = Path::new(assets_cli::required(&opts, "--out")?);
    fs::create_dir(out)?;
    durable::publish(plan, out)?;
    Ok(Some(preview))
}
