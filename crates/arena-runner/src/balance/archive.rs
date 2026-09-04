//! Balance artifact archive (plan section 12, built in B1, shared by
//! every stage): writes the complete
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
    CONFIG_FIELD_NAMES, ExperimentPlan, Gate, Stage, bot_seed_derivation, game_seed_derivation,
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

/// The CLI subcommand that re-runs this stage's frozen experiments — the
/// reproduce line of BOTH `manifest.txt` and `report.md` (Smoke=B1,
/// Readiness=B2, Ablation=B3, Sensitivity/Interaction=B4). A later stage
/// registers its subcommand here when its operator entry lands; the
/// explicit marker keeps an unwired stage visibly wrong instead of
/// silently re-issuing the B1 batch (the archived `-r1`/`-r2`/`-r3`
/// B2/B3 reports carried a stale `balance-b1` reproduce line for exactly
/// that reason).
fn stage_subcommand(stage: Stage) -> &'static str {
    match stage {
        Stage::Smoke => "balance-b1",
        Stage::Readiness => "balance-b2",
        Stage::Ablation => "balance-b3",
        Stage::Sensitivity | Stage::Interaction => "balance-b4",
        Stage::Retune | Stage::Confirm => "balance-<unwired-stage>",
    }
}

/// The committed doc that carries this stage's report and artifact
/// content hashes while `runs/` stays untracked (plan section 12).
/// `None` for stages whose report doc is not frozen yet (Retune); the
/// manifest note then states the plan-section-12 duty without naming a
/// wrong file.
fn stage_report_doc(stage: Stage) -> Option<&'static str> {
    match stage {
        Stage::Smoke => Some("docs/balance-b1-smoke.md"),
        Stage::Readiness => Some("docs/balance-b2-readiness.md"),
        Stage::Ablation => Some("docs/balance-b3-ablation.md"),
        Stage::Sensitivity | Stage::Interaction => Some("docs/balance-b4-sensitivity.md"),
        // Plan section 12: the final confirmation report syncs to
        // docs/balance-report-r2.md.
        Stage::Confirm => Some("docs/balance-report-r2.md"),
        Stage::Retune => None,
    }
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
    line(&mut text, "map_regime_variants", {
        let regime: Vec<&str> = plan
            .variants
            .iter()
            .filter(|v| v.is_map_regime())
            .map(|v| v.variant_id.as_str())
            .collect();
        if regime.is_empty() {
            "-".to_string()
        } else {
            format!(
                "{} (own MapDefinition from the same map_seed, ADR-0018; paired deltas include the map-regime change)",
                regime.join(",")
            )
        }
    });
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
            stage_subcommand(plan.stage),
            plan.experiment_id
        ),
    );
    line(&mut text, "workers", run.workers.to_string());
    line(&mut text, "elapsed_ms", run.elapsed_ms.to_string());
    line(
        &mut text,
        "note",
        match stage_report_doc(plan.stage) {
            Some(doc) => format!(
                "runs/ is untracked; {doc} carries the report and these content hashes (plan section 12)"
            ),
            None => "runs/ is untracked; no committed report doc is frozen for this stage yet — commit the report and these content hashes per plan section 12".to_string(),
        },
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

/// One archived map: the variant it belongs to (base rows carry the base
/// variant id; map-regime variants carry their own — ADR-0018), its
/// content hash and file name.
pub struct ArchivedMap {
    pub variant_id: String,
    pub map_id: String,
    pub map_seed: u64,
    pub content_hash: String,
    pub file: String,
}

/// Encodes, content-addresses and writes the plan's maps into `maps/`
/// (each distinct content once): the base-variant maps plus, for every
/// MAP-REGIME variant (ADR-0018 — a variant declaring a map-defining
/// field), that variant's own map per map seed (its cells ran on it).
/// An infeasible variant map has no row — its cells are the archive's
/// `config-failure` records, never a silent skip. Rule-only variants
/// byte-share the base rows by construction and executor verification.
pub fn write_maps(dir: &Path, plan: &ExperimentPlan) -> Result<Vec<ArchivedMap>, ArchiveError> {
    let maps_dir = dir.join("maps");
    fs::create_dir_all(&maps_dir)?;
    let mut by_content: BTreeMap<String, String> = BTreeMap::new();
    let mut store = |map: arena_core::map::MapDefinition| -> (String, String) {
        let bytes = encode_map(&map);
        let content_hash = blake3_hex(&bytes);
        let file = by_content
            .get(&content_hash)
            .cloned()
            .unwrap_or_else(|| format!("map-{content_hash}.bin"));
        if !by_content.contains_key(&content_hash) {
            let path = maps_dir.join(&file);
            if !path.exists() {
                fs::write(&path, &bytes).expect("map file writes");
            }
            by_content.insert(content_hash.clone(), file.clone());
        }
        (content_hash, file)
    };

    // Which variants contribute their OWN map rows: the base variant,
    // plus every map-regime variant.
    let mut row_variants: Vec<&crate::balance::plan::ConfigVariant> = vec![
        plan.variants
            .iter()
            .find(|v| v.variant_id == plan.base_variant)
            .expect("base"),
    ];
    for variant in &plan.variants {
        if variant.variant_id != plan.base_variant && variant.is_map_regime() {
            row_variants.push(variant);
        }
    }

    let mut index = Vec::with_capacity(row_variants.len() * plan.map_count as usize);
    for variant in row_variants {
        for k in 0..plan.map_count {
            let seed = crate::balance::plan::map_seed(plan.seed_set, k);
            let map = match arena_core::map::generate_map(seed, &variant.config) {
                Ok(map) => map,
                // Only reachable for a map-regime variant whose cells the
                // executor recorded as config failures (the base pool
                // regenerated above them); no row, no silent skip.
                Err(_) if variant.variant_id != plan.base_variant => continue,
                Err(_) => {
                    return Err(ArchiveError::Io(io::Error::other(
                        "base map does not regenerate",
                    )));
                }
            };
            let (content_hash, file) = store(map);
            index.push(ArchivedMap {
                variant_id: variant.variant_id.clone(),
                map_id: crate::balance::schedule::map_id(plan.seed_set, k),
                map_seed: seed,
                content_hash,
                file,
            });
        }
    }
    Ok(index)
}

/// The `maps.tsv` index body (the base format plus the trailing
/// `variant_id` column — ADR-0018).
pub fn maps_tsv(maps: &[ArchivedMap]) -> String {
    let mut text = String::from("map_id\tmap_seed\tcontent_hash\tfile\tvariant_id\n");
    for map in maps {
        text.push_str(&format!(
            "{}\t{:#018x}\t{}\tmaps/{}\t{}\n",
            map.map_id, map.map_seed, map.content_hash, map.file, map.variant_id
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
    out.push_str("## Gates\n\n");
    if plan.gates.is_empty() {
        if plan.stage == Stage::Smoke {
            out.push_str("None registered — B1 is the experiment-infrastructure milestone; gate evaluation arrives with the B3/B4 analysis layer.\n\n");
        } else {
            out.push_str("None registered for this experiment.\n\n");
        }
    } else {
        // The report is written before the evaluation artifact exists;
        // name where the evaluation lands in this same archive.
        out.push_str(&format!(
            "{} registered (declared at plan-build time; the evaluation is appended to this archive as {}):\n\n",
            plan.gates.len(),
            match plan.stage {
                Stage::Readiness => "gates.txt",
                Stage::Ablation => "analysis.txt",
                Stage::Sensitivity | Stage::Interaction => "effects.txt",
                _ => "the stage's analysis artifact",
            }
        ));
        for gate in &plan.gates {
            let Gate::Pending { gate_id, statement } = gate;
            out.push_str(&format!("- `{gate_id}` — {statement}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Archive\n\n");
    out.push_str(&format!(
        "- {} map index row(s) over content-addressed files under `maps/` (one file per distinct Map Definition; rule-only variants share the base file by construction and by executor verification, map-regime variants add their own rows — ADR-0018)\n",
        maps.len()
    ));
    out.push_str(&format!(
        "- {} sampled IARP replay(s) under `replays/`; crashed cells are listed in `replays.tsv` as unavailable\n",
        run.replays.len()
    ));
    out.push_str("- `hashes.txt` carries the blake3 of every artifact (runs/ is untracked; the committed doc cites these)\n\n");
    out.push_str(&format!(
        "Reproduce: `cargo run --release -p arena-runner -- {} runs <base_commit>` (base_commit recorded: {}).\n",
        stage_subcommand(plan.stage),
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

    // maps.tsv carries the variant dimension (ADR-0018): base rows plus
    // one row per map-regime variant map, content-addressed and deduped
    // by content; a rule-only variant shares the base row by construction.
    #[test]
    fn maps_tsv_carries_variant_rows_and_dedups_by_content() {
        let plan = crate::balance::sensitivity::b4_pilot_plan(2, "maps-variant-check");
        let root =
            std::env::temp_dir().join(format!("b4-maps-test-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("balance").join(&plan.experiment_id);
        std::fs::create_dir_all(&dir).expect("dir");
        let maps = write_maps(&dir, &plan).expect("writes");
        // 2 base + 2 portal-count-80 + 2 portal-count-160 rows; the other
        // eleven candidates byte-share the base rows (no rows of their own).
        assert_eq!(maps.len(), 6);
        let variants: Vec<&str> = maps.iter().map(|m| m.variant_id.as_str()).collect();
        assert_eq!(variants[0], "default");
        assert!(variants.contains(&"portal-count-80"));
        assert!(variants.contains(&"portal-count-160"));
        assert!(!variants.contains(&"move-budget-160m"));

        let text = maps_tsv(&maps);
        let mut lines = text.lines();
        assert_eq!(
            lines.next().expect("header"),
            "map_id\tmap_seed\tcontent_hash\tfile\tvariant_id"
        );
        let mut files = std::collections::BTreeSet::new();
        let mut hashes = std::collections::BTreeSet::new();
        for line in lines {
            let row: Vec<&str> = line.split('\t').collect();
            assert_eq!(row.len(), 5);
            let bytes = std::fs::read(dir.join(row[3])).expect("map file exists");
            assert_eq!(blake3_hex(&bytes), row[2], "content-addressed");
            hashes.insert(row[2].to_string());
            files.insert(row[3].to_string());
            // The row's map regenerates from its own variant config (the
            // base row under the base config, the regime row under its own).
            let variant = plan
                .variants
                .iter()
                .find(|v| v.variant_id == row[4])
                .expect("variant of the row");
            let seed = u64::from_str_radix(row[1].trim_start_matches("0x"), 16).expect("seed");
            let map = arena_core::map::generate_map(seed, &variant.config).expect("regenerates");
            assert_eq!(decode_map(&bytes).as_ref(), Some(&map));
            if row[4] == "portal-count-80" {
                assert_eq!(map.portals().len(), 80);
            }
        }
        assert_eq!(files.len(), hashes.len(), "deduped by content");
        assert_eq!(hashes.len(), 6, "six distinct maps");
        let _ = std::fs::remove_dir_all(&root);
    }

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

    /// The manifest and report are STAGE-AWARE: the reproduce subcommand,
    /// the gates section and the committed-doc note all follow the plan's
    /// stage. Regression pin for the archived-B3/B2 defect: report.md
    /// carried the B1 smoke boilerplate (a `balance-b1` reproduce line,
    /// "None registered" gates contradicting the manifest's gate count,
    /// and a note naming docs/balance-b1-smoke.md).
    #[test]
    fn manifest_and_report_are_stage_aware() {
        let run = BalanceRunResult {
            records: Vec::new(),
            replays: Vec::new(),
            config_failures: 0,
            elapsed_ms: 0,
            workers: 1,
        };
        let cases: Vec<(
            &str,
            ExperimentPlan,
            &'static str,
            &'static str,
            usize,
            Vec<&str>,
        )> = vec![
            (
                "smoke",
                crate::balance::smoke::b1_smoke_plan(1),
                "balance-b1",
                "docs/balance-b1-smoke.md",
                0,
                vec![],
            ),
            (
                "readiness",
                crate::balance::readiness::b2_idle_plan(1, "stage-aware-pin"),
                "balance-b2",
                "docs/balance-b2-readiness.md",
                1,
                vec!["idle-scoring"],
            ),
            (
                "ablation-matrix",
                crate::balance::ablation::b3_matrix_plan(1, "stage-aware-pin"),
                "balance-b3",
                "docs/balance-b3-ablation.md",
                2,
                vec!["records-integrity", "no-crashes"],
            ),
            (
                "ablation-idle",
                crate::balance::ablation::b3_idle_plan(1, "stage-aware-pin"),
                "balance-b3",
                "docs/balance-b3-ablation.md",
                1,
                vec!["idle-scoring"],
            ),
            (
                "sensitivity-pilot",
                crate::balance::sensitivity::b4_pilot_plan(1, "stage-aware-pin"),
                "balance-b4",
                "docs/balance-b4-sensitivity.md",
                3,
                vec!["records-integrity", "no-crashes", "command-drop"],
            ),
            (
                "interaction",
                crate::balance::sensitivity::b4_interaction_plan(
                    &crate::balance::sensitivity::B4_CANDIDATES[1],
                    &crate::balance::sensitivity::B4_CANDIDATES[3],
                    1,
                    "stage-aware-pin",
                ),
                "balance-b4",
                "docs/balance-b4-sensitivity.md",
                3,
                vec!["records-integrity", "no-crashes", "command-drop"],
            ),
        ];
        for (name, plan, subcommand, doc, gate_count, gate_ids) in cases {
            let manifest = manifest_text(&plan, &run, Some("pin-commit"));
            assert!(
                manifest.contains(&format!(
                    "reproduce=cargo run --release -p arena-runner -- {subcommand} runs <base_commit>"
                )),
                "{name}: manifest reproduce line names {subcommand}"
            );
            assert!(
                manifest.contains(&format!("note=runs/ is untracked; {doc} carries")),
                "{name}: manifest note names {doc}"
            );

            let report = report_text(&plan, &run, &[], &[], &[], Some("pin-commit"));
            assert!(
                report.contains(&format!(
                    "Reproduce: `cargo run --release -p arena-runner -- {subcommand} runs <base_commit>`"
                )),
                "{name}: report reproduce line names {subcommand}"
            );
            if gate_count == 0 {
                assert!(
                    report.contains("None registered"),
                    "{name}: zero-gates report says none registered"
                );
            } else {
                assert!(
                    report.contains(&format!(
                        "\n{gate_count} registered (declared at plan-build time"
                    )),
                    "{name}: report gates count matches the plan ({gate_count})"
                );
                assert!(
                    !report.contains("None registered"),
                    "{name}: registered gates are listed, not the B1 boilerplate"
                );
                for gate_id in gate_ids {
                    assert!(
                        report.contains(&format!("`{gate_id}`")),
                        "{name}: gate {gate_id} rendered"
                    );
                }
                assert!(
                    report.contains(match plan.stage {
                        Stage::Readiness => "appended to this archive as gates.txt",
                        Stage::Ablation => "appended to this archive as analysis.txt",
                        Stage::Sensitivity | Stage::Interaction => {
                            "appended to this archive as effects.txt"
                        }
                        _ => "appended to this archive as",
                    }),
                    "{name}: gates section names the stage's evaluation artifact"
                );
            }
            if plan.stage != Stage::Smoke {
                assert!(
                    !manifest.contains("balance-b1") && !report.contains("balance-b1"),
                    "{name}: no stale B1 boilerplate in manifest or report"
                );
            }
        }
    }
}
