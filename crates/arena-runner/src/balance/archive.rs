//! Balance artifact archive (B1, plan section 12): writes the complete
//! artifact set of one experiment into a NEW
//! `runs/balance/<experiment-id>/` directory — an existing directory is
//! REFUSED (historical runs are never overwritten). Everything the raw
//! data is the source of: manifest, complete configs, policy specs,
//! content-addressed Map Definitions, per-match records, paired integer
//! deltas, integer-only summary, human report, sampled IARP replays, and
//! content hashes of every artifact (runs/ is untracked; the committed
//! report cites these hashes — plan section 12).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use arena_core::codec::PORTAL_BYTES;
use arena_core::map::MapDefinition;
use arena_core::types::PortalId;

use crate::balance::execute::BalanceRunResult;
use crate::balance::pairs::{PairedResult, VariantSummary, paired_results_to_tsv};
use crate::balance::plan::{
    CONFIG_FIELD_NAMES, ExperimentPlan, bot_seed_derivation, game_seed_derivation,
    map_seed_derivation, schedule_seed_derivation,
};
use crate::balance::record::balance_records_to_tsv;

/// Archive-level failures.
#[derive(Debug)]
pub enum ArchiveError {
    /// The experiment directory already exists: refusing to overwrite a
    /// historical run.
    ExperimentDirExists(PathBuf),
    Io(io::Error),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::ExperimentDirExists(dir) => write!(
                f,
                "{} already exists — historical runs are never overwritten",
                dir.display()
            ),
            ArchiveError::Io(error) => write!(f, "io: {error}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<io::Error> for ArchiveError {
    fn from(error: io::Error) -> Self {
        ArchiveError::Io(error)
    }
}

/// The canonical portable map encoding of the balance archive: portal
/// count (u32 LE) followed by the codec portal encoding of every portal
/// in dense-id order (the D46 replay-header map shape).
pub fn encode_map(map: &MapDefinition) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + map.portals().len() * PORTAL_BYTES);
    out.extend_from_slice(&(map.portals().len() as u32).to_le_bytes());
    for portal in map.portals() {
        arena_core::codec::push_portal(portal, &mut out);
    }
    out
}

/// Inverse of [`encode_map`]: `None` on a wrong length or invalid owner
/// tag.
pub fn decode_map(bytes: &[u8]) -> Option<MapDefinition> {
    if bytes.len() < 4 {
        return None;
    }
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if bytes.len() != 4 + count * PORTAL_BYTES {
        return None;
    }
    let mut portals = Vec::with_capacity(count);
    for index in 0..count {
        let start = 4 + index * PORTAL_BYTES;
        portals.push(arena_core::codec::portal_from_bytes(
            PortalId(index as u32),
            &bytes[start..start + PORTAL_BYTES],
        )?);
    }
    Some(MapDefinition::new(portals))
}

fn blake3_hex(bytes: &[u8]) -> String {
    arena_replay::canon::hex(blake3::hash(bytes).as_bytes())
}

/// The `manifest.txt` body: full experiment provenance.
pub fn manifest_text(
    plan: &ExperimentPlan,
    run: &BalanceRunResult,
    base_commit: Option<&str>,
) -> String {
    let mut text = String::new();
    let line = |text: &mut String, key: &str, value: String| {
        text.push_str(key);
        text.push('=');
        text.push_str(&value);
        text.push('\n');
    };
    line(&mut text, "experiment_id", plan.experiment_id.clone());
    line(&mut text, "stage", plan.stage.tag().to_string());
    line(&mut text, "seed_set", plan.seed_set.tag().to_string());
    line(
        &mut text,
        "seed_set_master_seed",
        format!("{:#018x}", plan.seed_set.master_seed()),
    );
    line(
        &mut text,
        "seed_set_map_count_frozen",
        plan.seed_set.map_count().to_string(),
    );
    line(&mut text, "maps_used", plan.map_count.to_string());
    line(&mut text, "base_variant", plan.base_variant.clone());
    line(
        &mut text,
        "variants",
        plan.variants
            .iter()
            .map(|v| {
                format!(
                    "{}:{}",
                    v.variant_id,
                    if v.declared_changed_fields.is_empty() {
                        "-".to_string()
                    } else {
                        v.declared_changed_fields.join("+")
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(","),
    );
    line(
        &mut text,
        "policies",
        plan.policies
            .iter()
            .map(|s| {
                format!(
                    "{}={}/{}:{}",
                    s.policy_id.tag(),
                    s.family.tag(),
                    s.version,
                    s.parameters.tag()
                )
            })
            .collect::<Vec<_>>()
            .join(","),
    );
    line(
        &mut text,
        "matchups",
        plan.matchups
            .iter()
            .map(|m| format!("{}={}x{}", m.tag, m.a.tag(), m.b.tag()))
            .collect::<Vec<_>>()
            .join(","),
    );
    line(&mut text, "bucket_policy", plan.bucket_policy.tag());
    line(
        &mut text,
        "gates",
        if plan.gates.is_empty() {
            "none".to_string()
        } else {
            format!("{} registered", plan.gates.len())
        },
    );
    line(&mut text, "total_matches", run.records.len().to_string());
    line(
        &mut text,
        "config_hashes",
        plan.variants
            .iter()
            .map(|v| format!("{}={:#018x}", v.variant_id, v.config_hash()))
            .collect::<Vec<_>>()
            .join(","),
    );
    line(
        &mut text,
        "schedule_seed",
        format!("{:#018x}", plan.schedule_seed()),
    );
    line(&mut text, "map_seed_derivation", map_seed_derivation());
    line(&mut text, "game_seed_derivation", game_seed_derivation());
    line(&mut text, "bot_seed_derivation", bot_seed_derivation());
    line(
        &mut text,
        "schedule_seed_derivation",
        schedule_seed_derivation(),
    );
    line(
        &mut text,
        "pairing",
        "unit=(map, game_seed, bot_seed, policy pair, bucket policy); leg0=policy_a as ENL, leg1=policy_a as RES; variants reuse identical unit seeds".to_string(),
    );
    line(
        &mut text,
        "base_commit",
        base_commit.unwrap_or("unrecorded").to_string(),
    );
    line(
        &mut text,
        "reproduce",
        format!(
            "cargo run --release -p arena-runner -- {} runs <base_commit> (experiment {})",
            match plan.stage {
                crate::balance::plan::Stage::Readiness => "balance-b2",
                crate::balance::plan::Stage::Ablation => "balance-b3",
                _ => "balance-b1",
            },
            plan.experiment_id
        ),
    );
    line(&mut text, "workers", run.workers.to_string());
    line(&mut text, "elapsed_ms", run.elapsed_ms.to_string());
    line(
        &mut text,
        "note",
        "runs/ is untracked; docs/balance-b1-smoke.md carries the report and these content hashes (plan section 12)".to_string(),
    );
    text
}

/// The `configs.tsv` body: one row per variant — the COMPLETE 31-word
/// config plus its config hash and the declared changed fields.
pub fn configs_tsv(plan: &ExperimentPlan) -> String {
    let mut header = vec!["variant_id".to_string()];
    header.extend(CONFIG_FIELD_NAMES.iter().map(|s| s.to_string()));
    header.push("config_hash".to_string());
    header.push("changed_fields_vs_base".to_string());
    let mut text = String::new();
    text.push_str(&header.join("\t"));
    text.push('\n');
    let base = plan.base();
    for variant in &plan.variants {
        let mut words = Vec::with_capacity(CONFIG_FIELD_NAMES.len());
        let encoded = {
            let mut bytes = Vec::with_capacity(arena_core::codec::CONFIG_BYTES);
            arena_core::codec::push_config(&variant.config, &mut bytes);
            bytes
        };
        for index in 0..CONFIG_FIELD_NAMES.len() {
            words.push(format!(
                "{}",
                u32::from_le_bytes(
                    encoded[index * 4..index * 4 + 4]
                        .try_into()
                        .expect("4 bytes")
                )
            ));
        }
        let changed = variant.changed_fields_against(base);
        let mut row = vec![variant.variant_id.clone()];
        row.extend(words);
        row.push(format!("{:016x}", variant.config_hash()));
        row.push(if changed.is_empty() {
            "-".to_string()
        } else {
            changed.join("+")
        });
        text.push_str(&row.join("\t"));
        text.push('\n');
    }
    text
}

/// The `policies.tsv` body: id, family, version, parameters.
pub fn policies_tsv(plan: &ExperimentPlan) -> String {
    let mut text = String::from("policy_id\tfamily\tversion\tparameters\tfamily_class\n");
    for spec in &plan.policies {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            spec.policy_id.tag(),
            spec.family.tag(),
            spec.version,
            spec.parameters.tag(),
            if spec.family.is_diagnostic() {
                "diagnostic"
            } else {
                "competitive"
            },
        ));
    }
    text
}

/// One archived map: its content hash and file name.
pub struct ArchivedMap {
    pub map_id: String,
    pub map_seed: u64,
    pub content_hash: String,
    pub file: String,
}

/// Encodes, content-addresses and writes the plan's base-variant maps
/// into `maps/` (each distinct content once), returning the `maps.tsv`
/// index rows in map order.
pub fn write_maps(dir: &Path, plan: &ExperimentPlan) -> Result<Vec<ArchivedMap>, ArchiveError> {
    let maps_dir = dir.join("maps");
    fs::create_dir_all(&maps_dir)?;
    let base = plan.base();
    let mut index = Vec::with_capacity(plan.map_count as usize);
    let mut by_content: BTreeMap<String, String> = BTreeMap::new();
    for k in 0..plan.map_count {
        let seed = crate::balance::plan::map_seed(plan.seed_set, k);
        let map = arena_core::map::generate_map(seed, &base.config)
            .map_err(|_| ArchiveError::Io(io::Error::other("base map does not regenerate")))?;
        let bytes = encode_map(&map);
        let content_hash = blake3_hex(&bytes);
        let file = by_content
            .get(&content_hash)
            .cloned()
            .unwrap_or_else(|| format!("map-{content_hash}.bin"));
        if !by_content.contains_key(&content_hash) {
            let path = maps_dir.join(&file);
            if !path.exists() {
                fs::write(&path, &bytes)?;
            }
            by_content.insert(content_hash.clone(), file.clone());
        }
        index.push(ArchivedMap {
            map_id: crate::balance::schedule::map_id(plan.seed_set, k),
            map_seed: seed,
            content_hash,
            file,
        });
    }
    Ok(index)
}

/// The `maps.tsv` index body.
pub fn maps_tsv(maps: &[ArchivedMap]) -> String {
    let mut text = String::from("map_id\tmap_seed\tcontent_hash\tfile\n");
    for map in maps {
        text.push_str(&format!(
            "{}\t{:#018x}\t{}\tmaps/{}\n",
            map.map_id, map.map_seed, map.content_hash, map.file
        ));
    }
    text
}

/// The `replays.tsv` index body: one row per captured replay plus one per
/// crashed cell whose replay is unavailable (a panic unwinds past the
/// recorder — the crash itself is in `matches.tsv`).
pub fn replays_index(run: &BalanceRunResult) -> String {
    let mut text =
        String::from("cell_index\tvariant_id\tpair_id\tleg\tmap_id\tstatus\tfile\tblake3\n");
    for replay in &run.replays {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\tcaptured\treplays/replay-{}.iarp\t{}\n",
            replay.cell_index,
            replay.variant_id,
            replay.pair_id,
            replay.leg,
            replay.map_id,
            replay.cell_index,
            blake3_hex(&replay.bytes)
        ));
    }
    for record in &run.records {
        if record.result
            == crate::balance::record::BalanceOutcome::Played(crate::metrics::Outcome::Crash)
        {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\tunavailable-crashed\t-\t-\n",
                record.index, record.variant_id, record.pair_id, record.leg, record.map_id
            ));
        }
    }
    text
}

/// The `summary.txt` body: integer raw counts only (rates and intervals
/// are B4 offline analysis, never mixed into the archived summary).
pub fn summary_text(
    plan: &ExperimentPlan,
    run: &BalanceRunResult,
    summaries: &[VariantSummary],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# arena-runner balance summary ({}.{})\n",
        plan.experiment_id,
        plan.stage.tag()
    ));
    out.push_str("# integer raw counts only; derived rates are offline analysis (plan 9.3)\n\n");
    out.push_str(&format!(
        "total={} played={} crashes={} config_failures={} replays_captured={} maps={} variants={}\n\n",
        run.records.len(),
        run.played(),
        run.crashes(),
        run.config_failures,
        run.replays.len(),
        plan.map_count,
        plan.variants.len()
    ));
    for summary in summaries {
        out.push_str(&format!(
            "## variant {}{}\n",
            summary.variant_id,
            if summary.variant_id == plan.base_variant {
                " (base)".to_string()
            } else {
                format!(" (contrast vs {})", plan.base_variant)
            }
        ));
        out.push_str(&format!(
            "  units={} maps={} legs={} a_wins={} b_wins={} draws={} crashes={} config_failures={}\n",
            summary.units,
            summary.maps,
            summary.legs,
            summary.policy_a_wins,
            summary.policy_b_wins,
            summary.draws,
            summary.crashes,
            summary.config_failures
        ));
        out.push_str(&format!(
            "  margin_sum={} outcome_delta_sum_vs_base={} margin_delta_sum_vs_base={}\n\n",
            summary.margin_sum, summary.outcome_delta_sum_vs_base, summary.margin_delta_sum_vs_base
        ));
    }
    out
}

/// The `report.md` body: the human-readable experiment record.
pub fn report_text(
    plan: &ExperimentPlan,
    run: &BalanceRunResult,
    summaries: &[VariantSummary],
    _pairs: &[PairedResult],
    maps: &[ArchivedMap],
    base_commit: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Balance experiment {} (stage {})\n\n",
        plan.experiment_id,
        plan.stage.tag()
    ));
    out.push_str(&format!(
        "Balance experiment ({} stage): {} config variants x {} policies x {} maps x 2 swap legs = {} matches on the {} seed set.\n\n",
        plan.stage.tag(),
        plan.variants.len(),
        plan.policies.len(),
        plan.map_count,
        run.records.len(),
        plan.seed_set.tag()
    ));
    out.push_str("## Design\n\n");
    out.push_str("| Variant | Changed fields vs base | config hash |\n|---|---|---|\n");
    let base = plan.base();
    for variant in &plan.variants {
        let changed = variant.changed_fields_against(base);
        out.push_str(&format!(
            "| {} | {} | `{:016x}` |\n",
            variant.variant_id,
            if changed.is_empty() {
                "- (base)".to_string()
            } else {
                changed.join(", ")
            },
            variant.config_hash()
        ));
    }
    out.push_str("\n| Policy | Family | Version | Parameters |\n|---|---|---|---|\n");
    for spec in &plan.policies {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            spec.policy_id.tag(),
            spec.family.tag(),
            spec.version,
            spec.parameters.tag()
        ));
    }
    out.push_str("\nPairing: each comparable unit (map, game_seed, bot_seed, policy pair, bucket policy) is played twice per variant with opposite faction assignments; variants reuse the identical unit seeds (common random numbers, ADR-0014).\n\n");
    out.push_str("## Results (integer raw counts)\n\n");
    out.push_str("| Variant | Units | a_wins | b_wins | draws | crashes | config_failures | margin_sum | outcome_delta_vs_base | margin_delta_vs_base |\n|---|---|---|---|---|---|---|---|---|---|\n");
    for summary in summaries {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            summary.variant_id,
            summary.units,
            summary.policy_a_wins,
            summary.policy_b_wins,
            summary.draws,
            summary.crashes,
            summary.config_failures,
            summary.margin_sum,
            summary.outcome_delta_sum_vs_base,
            summary.margin_delta_sum_vs_base
        ));
    }
    out.push_str("\nPer-map paired rows are in `pairs.tsv` (one row per variant per unit, integer deltas only).\n\n");
    out.push_str("## Gates\n\nNone registered — B1 is the experiment-infrastructure milestone; gate evaluation arrives with the B3/B4 analysis layer.\n\n");
    out.push_str("## Archive\n\n");
    out.push_str(&format!(
        "- {} content-addressed map file(s) under `maps/` (one file per distinct Map Definition; cross-variant units share the same file by construction and by executor verification)\n",
        maps.len()
    ));
    out.push_str(&format!(
        "- {} sampled IARP replay(s) under `replays/`; crashed cells are listed in `replays.tsv` as unavailable\n",
        run.replays.len()
    ));
    out.push_str("- `hashes.txt` carries the blake3 of every artifact (runs/ is untracked; the committed doc cites these)\n\n");
    out.push_str(&format!(
        "Reproduce: `cargo run --release -p arena-runner -- balance-b1 runs <base_commit>` (base_commit recorded: {}).\n",
        base_commit.unwrap_or("unrecorded")
    ));
    out
}

/// Recursively collects every file under `dir` as sorted relative paths.
fn collect_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Writes the complete artifact set of one experiment into a NEW
/// `<output_root>/balance/<experiment-id>/` directory and returns the
/// directory. An existing experiment directory is refused; nothing
/// outside the new directory is touched.
pub fn write_balance_artifacts(
    output_root: &Path,
    plan: &ExperimentPlan,
    run: &BalanceRunResult,
    pairs: &[PairedResult],
    summaries: &[VariantSummary],
    base_commit: Option<&str>,
) -> Result<PathBuf, ArchiveError> {
    let dir = output_root.join("balance").join(&plan.experiment_id);
    if dir.exists() {
        return Err(ArchiveError::ExperimentDirExists(dir));
    }
    fs::create_dir_all(dir.join("replays"))?;

    let maps = write_maps(&dir, plan)?;
    let files: Vec<(&str, String)> = vec![
        ("manifest.txt", manifest_text(plan, run, base_commit)),
        ("configs.tsv", configs_tsv(plan)),
        ("policies.tsv", policies_tsv(plan)),
        ("maps.tsv", maps_tsv(&maps)),
        ("matches.tsv", balance_records_to_tsv(&run.records)),
        ("pairs.tsv", paired_results_to_tsv(pairs)),
        ("summary.txt", summary_text(plan, run, summaries)),
        (
            "report.md",
            report_text(plan, run, summaries, pairs, &maps, base_commit),
        ),
        ("replays.tsv", replays_index(run)),
    ];
    for (name, body) in &files {
        fs::write(dir.join(name), body)?;
    }
    for replay in &run.replays {
        fs::write(
            dir.join("replays")
                .join(format!("replay-{}.iarp", replay.cell_index)),
            &replay.bytes,
        )?;
    }

    // Content hashes of every artifact, written last (hashes.txt never
    // hashes itself).
    let mut hashes = String::new();
    for path in collect_files(&dir)? {
        let relative = path
            .strip_prefix(&dir)
            .expect("collected under the experiment dir");
        let bytes = fs::read(&path)?;
        hashes.push_str(&format!(
            "blake3:{}  {}\n",
            blake3_hex(&bytes),
            relative.display()
        ));
    }
    fs::write(dir.join("hashes.txt"), &hashes)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_encoding_round_trips() {
        let cfg = arena_core::config::GameConfig::default();
        let map = arena_core::map::generate_map(0x00A0_C111_0000_0001, &cfg).expect("generates");
        let bytes = encode_map(&map);
        assert_eq!(bytes.len(), 4 + map.portals().len() * PORTAL_BYTES);
        assert_eq!(decode_map(&bytes).as_ref(), Some(&map));
        assert_eq!(decode_map(&bytes[..10]), None, "wrong length refused");
        let tampered = {
            let mut b = bytes.clone();
            b[4] = 9; // invalid owner tag
            b
        };
        assert_eq!(decode_map(&tampered), None, "invalid owner refused");
        // Identical content -> identical content hash (content addressing).
        assert_eq!(blake3_hex(&bytes), blake3_hex(&encode_map(&map)));
    }
}
