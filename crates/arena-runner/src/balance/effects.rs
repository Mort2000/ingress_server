//! The B4 effects statistics (`docs/ai-balance-plan.md` sections 9.1/9.3/
//! 10.3, ADR-0018): per-(variant, pair) paired effects on the integer
//! `pairs.tsv` deltas, map-clustered accounting with the fixed-seed
//! per-map bootstrap, side-effect guardrails vs base, the ONE frozen
//! geometry stratification, the mechanical stability rule that gates the
//! Full roster, and the interaction difference-in-differences with the
//! frozen reversal classification. Everything here is NON-AUTHORITATIVE
//! OFFLINE ANALYSIS: integer raw counts always accompany derived
//! permille, floats exist only inside the Wilson computation, and nothing
//! derived here enters any Game Result, Replay Hash or rule settlement.

use std::collections::BTreeMap;
use std::path::Path;

use arena_core::map::MapDefinition;

use crate::balance::analysis::{MarginQuantiles, nearest_rank_quantile, wilson_permille};
use crate::balance::pairs::PairedResult;
use crate::balance::plan::ExperimentPlan;
use crate::balance::readiness::{drop_gate, records_gate};
use crate::balance::record::{BalanceMatchRecord, BalanceOutcome};
use crate::metrics::Outcome;

// ---------------------------------------------------------------------
// The played-unit filter (shared by every effect statistic)
// ---------------------------------------------------------------------

/// Whether a `pairs.tsv` row settled cleanly (both legs played, no
/// crash, no config failure).
fn row_settled(row: &PairedResult) -> bool {
    row.legs == 2 && row.crashes == 0 && row.config_failures == 0
}

/// The base rows' settledness by pair_id (a variant unit enters the
/// statistics only when the base settled the same unit).
fn base_settled_by_unit<'a>(
    pairs: &'a [PairedResult],
    base_variant: &str,
) -> BTreeMap<&'a str, bool> {
    pairs
        .iter()
        .filter(|r| r.variant_id == base_variant)
        .map(|r| (r.pair_id.as_str(), row_settled(r)))
        .collect()
}

// ---------------------------------------------------------------------
// Per-(variant, pair) paired effects (plan 9.1)
// ---------------------------------------------------------------------

/// One (variant, pair) paired-effect table: integer sign counts of the
/// per-unit deltas, sums, nearest-rank quartiles and the Wilson 95%
/// direction interval on up/(up+down). Never pools variants.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VariantPairEffect {
    pub variant_id: String,
    pub pair_tag: String,
    pub units: u64,
    /// Units that entered the statistics (settled on both sides).
    pub played_units: u64,
    /// Excluded units, counted per category (never silently dropped).
    pub excluded_variant_crash: u64,
    pub excluded_variant_config_failure: u64,
    pub excluded_base_not_settled: u64,
    pub margin_up: u64,
    pub margin_down: u64,
    pub margin_zero: u64,
    pub outcome_up: u64,
    pub outcome_down: u64,
    pub outcome_zero: u64,
    pub margin_delta_sum: i128,
    pub outcome_delta_sum: i64,
    pub margin_quartiles: Option<MarginQuantiles>,
    /// Wilson 95% (floor lower, ceil upper) permille of
    /// margin_up/(margin_up+margin_down); `None` when no decisive unit.
    pub wilson: Option<(u64, u64)>,
}

impl VariantPairEffect {
    /// +1 if the interval lies entirely above 500 permille, -1 entirely
    /// below, 0 otherwise (the frozen direction readout).
    pub fn one_sided(&self) -> i8 {
        match self.wilson {
            Some((low, _)) if low > 500 => 1,
            Some((_, high)) if high < 500 => -1,
            _ => 0,
        }
    }
}

/// The played-unit exclusion category of one variant unit row: which
/// side failed to settle. `None` when the row itself settled (then only
/// the base row can exclude the unit).
fn exclusion_kind(row: &PairedResult) -> Option<&'static str> {
    if row.config_failures > 0 {
        Some("config-failure")
    } else if row.crashes > 0 || row.legs != 2 {
        Some("crash")
    } else {
        None
    }
}

/// Every (variant, pair) effect table, in plan declaration order
/// (variant -> matchup). Base-variant rows are the self-contrast and are
/// NOT emitted.
pub fn paired_effects(plan: &ExperimentPlan, pairs: &[PairedResult]) -> Vec<VariantPairEffect> {
    let base_settled = base_settled_by_unit(pairs, &plan.base_variant);
    let mut out = Vec::new();
    for variant in &plan.variants {
        if variant.variant_id == plan.base_variant {
            continue;
        }
        for matchup in &plan.matchups {
            let rows: Vec<&PairedResult> = pairs
                .iter()
                .filter(|r| r.variant_id == variant.variant_id && r.pair_tag == matchup.tag)
                .collect();
            let mut effect = VariantPairEffect {
                variant_id: variant.variant_id.clone(),
                pair_tag: matchup.tag.clone(),
                units: rows.len() as u64,
                ..Default::default()
            };
            let mut margins: Vec<i128> = Vec::with_capacity(rows.len());
            for row in rows {
                effect.margin_delta_sum += row.margin_delta_vs_base;
                effect.outcome_delta_sum += row.outcome_delta_vs_base;
                let base_ok = base_settled.get(row.pair_id.as_str()).copied() == Some(true);
                if row_settled(row) && base_ok {
                    effect.played_units += 1;
                    margins.push(row.margin_delta_vs_base);
                    match row.margin_delta_vs_base.cmp(&0) {
                        std::cmp::Ordering::Greater => effect.margin_up += 1,
                        std::cmp::Ordering::Less => effect.margin_down += 1,
                        std::cmp::Ordering::Equal => effect.margin_zero += 1,
                    }
                    match row.outcome_delta_vs_base.cmp(&0) {
                        std::cmp::Ordering::Greater => effect.outcome_up += 1,
                        std::cmp::Ordering::Less => effect.outcome_down += 1,
                        std::cmp::Ordering::Equal => effect.outcome_zero += 1,
                    }
                } else if let Some(kind) = exclusion_kind(row) {
                    match kind {
                        "config-failure" => effect.excluded_variant_config_failure += 1,
                        _ => effect.excluded_variant_crash += 1,
                    }
                } else {
                    effect.excluded_base_not_settled += 1;
                }
            }
            effect.margin_quartiles = MarginQuantiles::of(&margins);
            effect.wilson =
                wilson_permille(effect.margin_up, effect.margin_up + effect.margin_down);
            out.push(effect);
        }
    }
    out
}

impl Default for VariantPairEffect {
    fn default() -> Self {
        VariantPairEffect {
            variant_id: String::new(),
            pair_tag: String::new(),
            units: 0,
            played_units: 0,
            excluded_variant_crash: 0,
            excluded_variant_config_failure: 0,
            excluded_base_not_settled: 0,
            margin_up: 0,
            margin_down: 0,
            margin_zero: 0,
            outcome_up: 0,
            outcome_down: 0,
            outcome_zero: 0,
            margin_delta_sum: 0,
            outcome_delta_sum: 0,
            margin_quartiles: None,
            wilson: None,
        }
    }
}

// ---------------------------------------------------------------------
// Map-clustered accounting + fixed-seed per-map bootstrap (plan 9.3)
// ---------------------------------------------------------------------

/// The frozen bootstrap size and algorithm version (ADR-0018).
pub const BOOTSTRAP_ITERATIONS: u64 = 10_000;
pub const BOOTSTRAP_ALGORITHM: &str = "b4-map-clustered-bootstrap-v1";
const BOOTSTRAP_LABEL: &[u8] = b"ingress-arena-r2-b4boot";

/// One map-index draw of the fixed-seed stream: blake3 of
/// (label || seed || replicate || draw), first 8 bytes LE.
fn bootstrap_draw(seed: u64, replicate: u64, draw: u64) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BOOTSTRAP_LABEL);
    hasher.update(&seed.to_le_bytes());
    hasher.update(&replicate.to_le_bytes());
    hasher.update(&draw.to_le_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
    u64::from_le_bytes(out)
}

/// The fixed-seed per-map bootstrap of one variant's pooled
/// `margin_delta` distribution: MAP indices are resampled WITH
/// replacement — one drawn map carries ALL its pair-deltas together,
/// preserving within-map correlation (6 pairs share every map). The
/// replicate statistic is the resampled delta SUM (integer); outputs are
/// nearest-rank percentiles. The seed derives ONLY from
/// (experiment_id, variant_id) — domain-separated, never from the data.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapOutcome {
    pub algorithm: &'static str,
    pub iterations: u64,
    pub maps: u64,
    pub units: u64,
    pub seed: u64,
    /// p025/p500/p975 (nearest rank) of the replicate sums.
    pub p025: i128,
    pub p500: i128,
    pub p975: i128,
    /// The raw resampled sums (integer; recomputable input of the
    /// percentiles, plan 9.3 raw-counts duty).
    pub replicate_sums: Vec<i128>,
}

/// Groups one variant's PLAYED unit deltas per map (a map carries the
/// deltas of every pair on it).
pub fn deltas_per_map(
    plan: &ExperimentPlan,
    pairs: &[PairedResult],
    variant_id: &str,
) -> BTreeMap<u32, Vec<i128>> {
    let base_settled = base_settled_by_unit(pairs, &plan.base_variant);
    let mut per_map: BTreeMap<u32, Vec<i128>> = BTreeMap::new();
    for row in pairs
        .iter()
        .filter(|r| r.variant_id == variant_id && r.variant_id != plan.base_variant)
    {
        let base_ok = base_settled.get(row.pair_id.as_str()).copied() == Some(true);
        if row_settled(row) && base_ok {
            per_map
                .entry(row.map_index)
                .or_default()
                .push(row.margin_delta_vs_base);
        }
    }
    per_map
}

/// Runs the bootstrap over a per-map delta grouping.
pub fn map_clustered_bootstrap(
    experiment_id: &str,
    variant_id: &str,
    per_map: &BTreeMap<u32, Vec<i128>>,
) -> BootstrapOutcome {
    let mut payload = Vec::with_capacity(experiment_id.len() + variant_id.len());
    payload.extend_from_slice(experiment_id.as_bytes());
    payload.extend_from_slice(variant_id.as_bytes());
    let seed = crate::derive_seed(BOOTSTRAP_LABEL, &payload);

    let maps: Vec<&Vec<i128>> = per_map.values().collect();
    let map_count = maps.len() as u64;
    let units: u64 = maps.iter().map(|deltas| deltas.len() as u64).sum();
    let mut replicate_sums = Vec::with_capacity(BOOTSTRAP_ITERATIONS as usize);
    if map_count == 0 {
        return BootstrapOutcome {
            algorithm: BOOTSTRAP_ALGORITHM,
            iterations: BOOTSTRAP_ITERATIONS,
            maps: 0,
            units: 0,
            seed,
            p025: 0,
            p500: 0,
            p975: 0,
            replicate_sums,
        };
    }
    for replicate in 0..BOOTSTRAP_ITERATIONS {
        let mut sum: i128 = 0;
        for draw in 0..map_count {
            let index = (bootstrap_draw(seed, replicate, draw) % map_count as u64) as usize;
            for delta in maps[index] {
                sum += delta;
            }
        }
        replicate_sums.push(sum);
    }
    BootstrapOutcome {
        algorithm: BOOTSTRAP_ALGORITHM,
        iterations: BOOTSTRAP_ITERATIONS,
        maps: map_count,
        units,
        seed,
        p025: nearest_rank_quantile(&replicate_sums, 25).expect("non-empty"),
        p500: nearest_rank_quantile(&replicate_sums, 500).expect("non-empty"),
        p975: nearest_rank_quantile(&replicate_sums, 975).expect("non-empty"),
        replicate_sums,
    }
}

// ---------------------------------------------------------------------
// Side-effect guardrails (plan 10.3, per candidate vs base, pooled over
// pairs) — integer raw counts with permille beside, never instead
// ---------------------------------------------------------------------

/// One mechanism-utilization row: the variant's pooled integer sum over
/// its settled records (both factions), the base's, and the signed
/// relative change in permille (`None` when the base sum is 0 — the raw
/// counts stay the record).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MechanicRow {
    pub field: &'static str,
    pub variant_sum: u128,
    pub base_sum: u128,
    pub delta_permille: Option<i64>,
}

/// The pooled mechanism sums of one side's settled records (both
/// factions — the variant's own matches).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct MechanicSums {
    pub transfers: u128,
    pub recharges: u128,
    pub attacks: u128,
    pub attack_hits: u128,
    pub hacks: u128,
    pub links_created: u128,
    pub fields_created: u128,
    pub first_field_count: u128,
    pub first_field_round_sum: u128,
    pub field_lifetime_rounds: u128,
}

fn mechanic_sums(records: &[&BalanceMatchRecord]) -> MechanicSums {
    let mut sums = MechanicSums::default();
    for record in records {
        for side in [&record.stats.enl, &record.stats.res] {
            sums.transfers += side.transfers as u128;
            sums.recharges += side.recharges as u128;
            sums.attacks += side.attacks as u128;
            sums.attack_hits += side.attack_hits as u128;
            sums.hacks += side.hacks as u128;
            sums.links_created += side.links_created as u128;
            sums.fields_created += side.fields_created as u128;
            if let Some(round) = side.first_field_round {
                sums.first_field_count += 1;
                sums.first_field_round_sum += round as u128;
            }
            sums.field_lifetime_rounds += side.destroyed_lifetime_rounds_sum as u128;
            sums.field_lifetime_rounds += side.surviving_lifetime_rounds_sum as u128;
        }
    }
    sums
}

fn mechanic_row(field: &'static str, variant: u128, base: u128) -> MechanicRow {
    let delta_permille = if base == 0 {
        None
    } else {
        Some(((variant as i128 - base as i128) * 1000 / base as i128) as i64)
    };
    MechanicRow {
        field,
        variant_sum: variant,
        base_sum: base,
        delta_permille,
    }
}

/// One candidate's side-effect report vs base, pooled over pairs: raw
/// counts of the frozen guardrails (no-score matches, draws, crashes,
/// config failures, command drop, faction split) plus mechanism
/// utilization deltas. `flags` names every frozen regression verdict.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SideEffects {
    pub variant_id: String,
    pub records: u64,
    pub settled: u64,
    pub crashes: u64,
    pub config_failures: u64,
    /// Settled matches where both factions scored zero.
    pub both_zero: u64,
    pub both_zero_base: u64,
    pub draws: u64,
    pub draws_base: u64,
    pub enl_wins: u64,
    pub res_wins: u64,
    pub drop_submitted: u64,
    pub drop_dropped: u64,
    pub drop_permille: u64,
    /// The frozen band verdict: pooled drop < 50 permille.
    pub drop_below_band: bool,
    pub mechanics: Vec<MechanicRow>,
    pub flags: Vec<String>,
}

fn record_settled(record: &BalanceMatchRecord) -> bool {
    !matches!(
        record.result,
        BalanceOutcome::ConfigFailure | BalanceOutcome::Played(Outcome::Crash)
    )
}

/// The side-effect guardrails of one non-base variant vs the base
/// variant, pooled over the experiment's pairs.
pub fn side_effects(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    variant_id: &str,
) -> SideEffects {
    let of = |id: &str| -> Vec<&BalanceMatchRecord> {
        records.iter().filter(|r| r.variant_id == id).collect()
    };
    let variant_rows = of(variant_id);
    let base_rows = of(&plan.base_variant);
    let settled_rows: Vec<&BalanceMatchRecord> = variant_rows
        .iter()
        .copied()
        .filter(|r| record_settled(r))
        .collect();
    let base_settled: Vec<&BalanceMatchRecord> = base_rows
        .iter()
        .copied()
        .filter(|r| record_settled(r))
        .collect();

    let mut both_zero = 0;
    let mut draws = 0;
    let mut enl_wins = 0;
    let mut res_wins = 0;
    let mut drop_submitted = 0;
    let mut drop_dropped = 0;
    for record in &settled_rows {
        if record.enl_score2 == 0 && record.res_score2 == 0 {
            both_zero += 1;
        }
        match record.result {
            BalanceOutcome::Played(Outcome::Draw) => draws += 1,
            BalanceOutcome::Played(Outcome::EnlWin) => enl_wins += 1,
            BalanceOutcome::Played(Outcome::ResWin) => res_wins += 1,
            _ => {}
        }
        drop_submitted += record.stats.enl.commands_submitted + record.stats.res.commands_submitted;
        drop_dropped += record.stats.enl.commands_dropped + record.stats.res.commands_dropped;
    }
    let both_zero_base = base_settled
        .iter()
        .filter(|r| r.enl_score2 == 0 && r.res_score2 == 0)
        .count() as u64;
    let draws_base = base_settled
        .iter()
        .filter(|r| r.result == BalanceOutcome::Played(Outcome::Draw))
        .count() as u64;

    let sums = mechanic_sums(&settled_rows);
    let base_sums = mechanic_sums(&base_settled);
    let mechanics = vec![
        mechanic_row("transfers", sums.transfers, base_sums.transfers),
        mechanic_row("recharges", sums.recharges, base_sums.recharges),
        mechanic_row("attacks", sums.attacks, base_sums.attacks),
        mechanic_row("attack_hits", sums.attack_hits, base_sums.attack_hits),
        mechanic_row("hacks", sums.hacks, base_sums.hacks),
        mechanic_row("links_created", sums.links_created, base_sums.links_created),
        mechanic_row(
            "fields_created",
            sums.fields_created,
            base_sums.fields_created,
        ),
        mechanic_row(
            "first_field_count",
            sums.first_field_count,
            base_sums.first_field_count,
        ),
        mechanic_row(
            "first_field_round_sum",
            sums.first_field_round_sum,
            base_sums.first_field_round_sum,
        ),
        mechanic_row(
            "field_lifetime_rounds",
            sums.field_lifetime_rounds,
            base_sums.field_lifetime_rounds,
        ),
    ];

    let drop_permille = if drop_submitted == 0 {
        0
    } else {
        drop_dropped * 1000 / drop_submitted
    };
    let drop_below_band = drop_submitted > 0 && drop_permille < 50;
    let crashes = variant_rows
        .iter()
        .filter(|r| r.result == BalanceOutcome::Played(Outcome::Crash))
        .count() as u64;
    let config_failures = variant_rows
        .iter()
        .filter(|r| r.result == BalanceOutcome::ConfigFailure)
        .count() as u64;

    // The frozen regression verdicts (plan 10.3: no unexplained worsening
    // of no-score rate, draw rate or command drop).
    let mut flags = Vec::new();
    if crashes > 0 || config_failures > 0 {
        flags.push(format!(
            "failures: {crashes} crashes, {config_failures} config failures (must be zero)"
        ));
    }
    if !drop_below_band {
        flags.push(format!(
            "command drop {drop_dropped}/{drop_submitted} = {drop_permille} permille >= 50 band"
        ));
    }
    if both_zero > both_zero_base {
        flags.push(format!(
            "no-score matches rose {both_zero_base} -> {both_zero}"
        ));
    }
    if draws > draws_base {
        flags.push(format!("draws rose {draws_base} -> {draws}"));
    }

    SideEffects {
        variant_id: variant_id.to_string(),
        records: variant_rows.len() as u64,
        settled: settled_rows.len() as u64,
        crashes,
        config_failures,
        both_zero,
        both_zero_base,
        draws,
        draws_base,
        enl_wins,
        res_wins,
        drop_submitted,
        drop_dropped,
        drop_permille,
        drop_below_band,
        mechanics,
        flags,
    }
}

// ---------------------------------------------------------------------
// The ONE frozen geometry stratification (plan 9.3 / plan 11 map row)
// ---------------------------------------------------------------------

/// floor(sqrt(d2)) — exact integer square root of a non-negative i64.
fn isqrt(d2: i64) -> i64 {
    let mut root = (d2 as f64).sqrt() as i64;
    while root > 0 && root * root > d2 {
        root -= 1;
    }
    while (root + 1) * (root + 1) <= d2 {
        root += 1;
    }
    root
}

/// The frozen map-geometry feature: the nearest-rank MEDIAN over portals
/// of the floor distance (cm) to the nearest OTHER portal (integer; a
/// 2-portal map yields their distance).
pub fn nearest_neighbor_feature(map: &MapDefinition) -> i128 {
    let portals = map.portals();
    let mut nearest: Vec<i128> = Vec::with_capacity(portals.len());
    for (index, portal) in portals.iter().enumerate() {
        let mut best: Option<i64> = None;
        for (other, other_portal) in portals.iter().enumerate() {
            if other == index {
                continue;
            }
            let distance = isqrt(arena_core::geometry::dist2(
                portal.position,
                other_portal.position,
            ));
            best = Some(match best {
                None => distance,
                Some(current) => current.min(distance),
            });
        }
        if let Some(distance) = best {
            nearest.push(distance as i128);
        }
    }
    nearest_rank_quantile(&nearest, 500).unwrap_or(0)
}

/// One (variant, stratum) direction block over played units.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct GeometryRow {
    pub variant_id: String,
    /// 1 = low-density third, 3 = high (nearest-rank terciles of the
    /// base maps' features; values outside the base range clamp in).
    pub stratum: u8,
    pub up: u64,
    pub down: u64,
    pub zero: u64,
}

/// The frozen geometry stratification (ADR-0018): tercile cut points from
/// the BASE maps' nearest-neighbor features (nearest-rank p333/p667),
/// each variant's units classified by ITS OWN archived map's feature
/// (map-regime variants stratify on their own maps), direction counts of
/// the played per-unit margin deltas per (variant, stratum).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GeometryStrata {
    pub cut_low: i128,
    pub cut_high: i128,
    pub rows: Vec<GeometryRow>,
}

/// Loads the archived maps of a run directory: `(variant_id, map_index,
/// MapDefinition)` per `maps.tsv` row (base rows plus map-regime rows —
/// ADR-0018's variant dimension).
pub fn load_archived_maps(dir: &Path) -> Result<Vec<(String, u32, MapDefinition)>, String> {
    let text =
        std::fs::read_to_string(dir.join("maps.tsv")).map_err(|e| format!("maps.tsv: {e}"))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty maps.tsv")?.split('\t').collect();
    if header != ["map_id", "map_seed", "content_hash", "file", "variant_id"] {
        return Err("maps.tsv header mismatch".to_string());
    }
    let mut out = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let row: Vec<&str> = line.split('\t').collect();
        if row.len() != 5 {
            return Err(format!("maps.tsv row width {}", row.len()));
        }
        let k: u32 = row[0]
            .rsplit('-')
            .next()
            .and_then(|suffix| suffix.parse().ok())
            .ok_or_else(|| format!("map id {}", row[0]))?;
        let bytes = std::fs::read(dir.join(row[3])).map_err(|e| format!("{}: {e}", row[3]))?;
        let map = crate::balance::archive::decode_map(&bytes).ok_or("map decode")?;
        out.push((row[4].to_string(), k, map));
    }
    Ok(out)
}

/// Computes the geometry strata of every non-base variant from the
/// archived maps (a variant without its own rows byte-shares the base
/// map of the same index — executor-verified).
pub fn geometry_strata(
    plan: &ExperimentPlan,
    pairs: &[PairedResult],
    archived: &[(String, u32, MapDefinition)],
) -> GeometryStrata {
    // Base features per map index, then the frozen tercile cut points.
    let base_feature: BTreeMap<u32, i128> = archived
        .iter()
        .filter(|(variant, _, _)| *variant == plan.base_variant)
        .map(|(_, k, map)| (*k, nearest_neighbor_feature(map)))
        .collect();
    let base_values: Vec<i128> = base_feature.values().copied().collect();
    let (cut_low, cut_high) = (
        nearest_rank_quantile(&base_values, 333).unwrap_or(0),
        nearest_rank_quantile(&base_values, 667).unwrap_or(0),
    );

    // Each (variant, k) resolves to its own archived map, else the base
    // map of k (byte-shared).
    let feature_of = |variant_id: &str, k: u32| -> i128 {
        let own = archived
            .iter()
            .find(|(variant, index, _)| variant == variant_id && *index == k);
        match own {
            Some((_, _, map)) => nearest_neighbor_feature(map),
            None => *base_feature.get(&k).unwrap_or(&0),
        }
    };

    let mut rows: Vec<GeometryRow> = Vec::new();
    let base_settled = base_settled_by_unit(pairs, &plan.base_variant);
    for variant in &plan.variants {
        if variant.variant_id == plan.base_variant {
            continue;
        }
        let mut by_stratum: BTreeMap<u8, GeometryRow> = BTreeMap::new();
        for row in pairs.iter().filter(|r| r.variant_id == variant.variant_id) {
            let base_ok = base_settled.get(row.pair_id.as_str()).copied() == Some(true);
            if !(row_settled(row) && base_ok) {
                continue;
            }
            let feature = feature_of(&variant.variant_id, row.map_index);
            let stratum = if feature <= cut_low {
                1
            } else if feature <= cut_high {
                2
            } else {
                3
            };
            let entry = by_stratum.entry(stratum).or_default();
            entry.variant_id = variant.variant_id.clone();
            entry.stratum = stratum;
            match row.margin_delta_vs_base.cmp(&0) {
                std::cmp::Ordering::Greater => entry.up += 1,
                std::cmp::Ordering::Less => entry.down += 1,
                std::cmp::Ordering::Equal => entry.zero += 1,
            }
        }
        rows.extend(by_stratum.into_values());
    }
    GeometryStrata {
        cut_low,
        cut_high,
        rows,
    }
}

// ---------------------------------------------------------------------
// The frozen stability rule (plan 7.1/8.3; the ONLY bridge pilot -> Full)
// ---------------------------------------------------------------------

/// One candidate's mechanical stability verdict (ADR-0018): stable :=
/// zero crashes AND zero config failures AND pooled drop < 50 permille
/// AND >= 3 pair_tags with the margin-direction Wilson 95% entirely on
/// one side of 500 permille, all agreeing AND pooled median
/// margin_delta != 0. Every failing leg is recorded — an unstable
/// direction is STOPPED and recorded (plan 7.1 else-branch), never
/// silently dropped.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StabilityVerdict {
    pub variant_id: String,
    pub stable: bool,
    /// +1/-1 of the agreeing one-sided pairs (0 when they disagree or
    /// none reach the bar).
    pub direction: i8,
    /// The pair tags whose interval cleared 500 permille WITH the
    /// qualifying direction.
    pub agreeing_pairs: Vec<(String, i8)>,
    /// One-sided pairs pointing the OTHER way (recorded, never hidden —
    /// plan 10.3 sets a >=3-pair threshold, not unanimity).
    pub dissenting_pairs: Vec<(String, i8)>,
    /// Pooled nearest-rank median of the played margin deltas.
    pub margin_median: i128,
    pub drop_permille: u64,
    pub crashes: u64,
    pub config_failures: u64,
    /// Which legs of the rule failed (empty when stable).
    pub failed: Vec<String>,
}

/// Evaluates the stability rule for every non-base variant of the plan.
pub fn stability(
    plan: &ExperimentPlan,
    pairs: &[PairedResult],
    records: &[BalanceMatchRecord],
) -> Vec<StabilityVerdict> {
    let effects = paired_effects(plan, pairs);
    stability_from_effects(plan, pairs, records, &effects)
}

/// The stability rule over pre-computed effect tables (the renderer and
/// the roster bridge share one evaluation).
pub fn stability_from_effects(
    plan: &ExperimentPlan,
    pairs: &[PairedResult],
    records: &[BalanceMatchRecord],
    effects: &[VariantPairEffect],
) -> Vec<StabilityVerdict> {
    let mut verdicts = Vec::new();
    for variant in &plan.variants {
        if variant.variant_id == plan.base_variant {
            continue;
        }
        let id = variant.variant_id.as_str();
        let rows: Vec<&VariantPairEffect> = effects.iter().filter(|e| e.variant_id == id).collect();
        let one_sided: Vec<(String, i8)> = rows
            .iter()
            .filter(|e| e.one_sided() != 0)
            .map(|e| (e.pair_tag.clone(), e.one_sided()))
            .collect();
        // The plan-10.3 threshold: at least THREE independent pairs with
        // the same direction — not unanimity. The qualifying direction is
        // the side with the most one-sided pairs, requiring >= 3 and
        // strictly more than the opposite side (a 3+3 tie has none).
        let plus = one_sided.iter().filter(|(_, side)| *side > 0).count();
        let minus = one_sided.iter().filter(|(_, side)| *side < 0).count();
        let (direction, agreeing, dissenting) = if plus >= 3 && plus > minus {
            (
                1,
                one_sided
                    .iter()
                    .filter(|(_, s)| *s > 0)
                    .cloned()
                    .collect::<Vec<_>>(),
                one_sided
                    .iter()
                    .filter(|(_, s)| *s < 0)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        } else if minus >= 3 && minus > plus {
            (
                -1,
                one_sided
                    .iter()
                    .filter(|(_, s)| *s < 0)
                    .cloned()
                    .collect::<Vec<_>>(),
                one_sided
                    .iter()
                    .filter(|(_, s)| *s > 0)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        } else {
            (0, Vec::new(), one_sided)
        };

        let per_map = deltas_per_map(plan, pairs, id);
        let pooled: Vec<i128> = per_map.values().flatten().copied().collect();
        let margin_median = nearest_rank_quantile(&pooled, 500).unwrap_or(0);

        let side = side_effects(plan, records, id);
        let mut failed = Vec::new();
        if side.crashes > 0 || side.config_failures > 0 {
            failed.push(format!(
                "failures: {} crashes, {} config failures",
                side.crashes, side.config_failures
            ));
        }
        if !side.drop_below_band {
            failed.push(format!(
                "pooled drop {} permille >= 50 (raw {}/{})",
                side.drop_permille, side.drop_dropped, side.drop_submitted
            ));
        }
        if direction == 0 {
            if plus >= 3 && plus == minus {
                failed.push(format!(
                    "one-sided pairs tie {plus}+/{minus}- — no direction"
                ));
            } else {
                failed.push(format!(
                    "{} one-sided pair(s) with one direction (need >= 3 of {} pairs)",
                    plus.max(minus),
                    rows.len()
                ));
            }
        }
        if margin_median == 0 {
            failed.push(
                "pooled median margin_delta = 0 (no practically meaningful effect)".to_string(),
            );
        }

        verdicts.push(StabilityVerdict {
            variant_id: variant.variant_id.clone(),
            stable: failed.is_empty(),
            direction,
            agreeing_pairs: agreeing,
            dissenting_pairs: dissenting,
            margin_median,
            drop_permille: side.drop_permille,
            crashes: side.crashes,
            config_failures: side.config_failures,
            failed,
        });
    }
    verdicts
}

/// The Full-roster derivation: the stable variant ids in plan
/// declaration order — the ONLY bridge from pilot data to the Full
/// experiment (pre-registered and auditable, ADR-0018).
pub fn stable_candidates(verdicts: &[StabilityVerdict]) -> Vec<String> {
    verdicts
        .iter()
        .filter(|v| v.stable)
        .map(|v| v.variant_id.clone())
        .collect()
}

// ---------------------------------------------------------------------
// Interaction difference-in-differences (plan 7.4, ADR-0018)
// ---------------------------------------------------------------------

/// One pair's DiD table over its units: sign counts of
/// `delta(A+B) - delta(A) - delta(B)` (margin and outcome) and of the
/// conditional direction `delta(A+B) - delta(B)`.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct DidPairRow {
    pub pair_tag: String,
    pub units: u64,
    pub margin_did_sum: i128,
    pub margin_did_up: u64,
    pub margin_did_down: u64,
    pub margin_did_zero: u64,
    pub outcome_did_sum: i64,
    /// Conditional direction `delta(A+B) - delta(B)` sign counts.
    pub conditional_up: u64,
    pub conditional_down: u64,
    pub conditional_zero: u64,
}

/// The interaction readout: per-pair DiD sign counts, A's main-effect
/// direction from the same run, and the frozen reversal classification
/// (conditional direction interval entirely opposite to A's one-sided
/// main effect).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InteractionDid {
    pub factor_a: String,
    pub factor_b: String,
    pub combined: String,
    pub rows: Vec<DidPairRow>,
    /// Wilson 95% on A's pooled delta direction (up/(up+down)).
    pub a_wilson: Option<(u64, u64)>,
    /// Wilson 95% on the pooled conditional direction.
    pub conditional_wilson: Option<(u64, u64)>,
    pub reversal: bool,
    pub reversal_reason: String,
}

impl InteractionDid {
    /// A's main-effect direction (+1/-1/0) with interval evidence.
    pub fn a_direction(&self) -> i8 {
        match self.a_wilson {
            Some((low, _)) if low > 500 => 1,
            Some((_, high)) if high < 500 => -1,
            _ => 0,
        }
    }
}

/// Computes the interaction DiD of a `[base, A, B, A+B]` plan from its
/// own `pairs.tsv` rows (played units only — same filter as every other
/// B4 statistic).
pub fn interaction_did(
    plan: &ExperimentPlan,
    pairs: &[PairedResult],
) -> Result<InteractionDid, String> {
    if plan.variants.len() != 4 {
        return Err(format!(
            "an interaction plan has [base, A, B, A+B]; {} variants",
            plan.variants.len()
        ));
    }
    let delta_of = |variant_id: &str, pair_id: &str| -> Option<&PairedResult> {
        pairs
            .iter()
            .find(|r| r.variant_id == variant_id && r.pair_id == pair_id)
    };
    let base_settled = base_settled_by_unit(pairs, &plan.base_variant);
    let a = &plan.variants[1];
    let b = &plan.variants[2];
    let combined = &plan.variants[3];

    let mut rows: BTreeMap<String, DidPairRow> = BTreeMap::new();
    let mut a_up = 0u64;
    let mut a_down = 0u64;
    let mut cond_up = 0u64;
    let mut cond_down = 0u64;
    for row in pairs.iter().filter(|r| r.variant_id == combined.variant_id) {
        let base_ok = base_settled.get(row.pair_id.as_str()).copied() == Some(true);
        let (Some(a_row), Some(b_row)) = (
            delta_of(&a.variant_id, &row.pair_id),
            delta_of(&b.variant_id, &row.pair_id),
        ) else {
            return Err(format!(
                "interaction unit {} missing an arm row (A or B)",
                row.pair_id
            ));
        };
        let entry = rows.entry(row.pair_tag.clone()).or_default();
        entry.pair_tag = row.pair_tag.clone();
        if row_settled(row) && row_settled(a_row) && row_settled(b_row) && base_ok {
            entry.units += 1;
            let margin_did =
                row.margin_delta_vs_base - a_row.margin_delta_vs_base - b_row.margin_delta_vs_base;
            let outcome_did = row.outcome_delta_vs_base
                - a_row.outcome_delta_vs_base
                - b_row.outcome_delta_vs_base;
            entry.margin_did_sum += margin_did;
            entry.outcome_did_sum += outcome_did;
            match margin_did.cmp(&0) {
                std::cmp::Ordering::Greater => entry.margin_did_up += 1,
                std::cmp::Ordering::Less => entry.margin_did_down += 1,
                std::cmp::Ordering::Equal => entry.margin_did_zero += 1,
            }
            match a_row.margin_delta_vs_base.cmp(&0) {
                std::cmp::Ordering::Greater => a_up += 1,
                std::cmp::Ordering::Less => a_down += 1,
                std::cmp::Ordering::Equal => {}
            }
            let conditional = row.margin_delta_vs_base - b_row.margin_delta_vs_base;
            match conditional.cmp(&0) {
                std::cmp::Ordering::Greater => {
                    cond_up += 1;
                    entry.conditional_up += 1;
                }
                std::cmp::Ordering::Less => {
                    cond_down += 1;
                    entry.conditional_down += 1;
                }
                std::cmp::Ordering::Equal => entry.conditional_zero += 1,
            }
        }
    }
    let a_wilson = wilson_permille(a_up, a_up + a_down);
    let conditional_wilson = wilson_permille(cond_up, cond_up + cond_down);
    let mut reversal = false;
    let mut reversal_reason = "no reversal: ".to_string();
    let a_direction = match a_wilson {
        Some((low, _)) if low > 500 => 1,
        Some((_, high)) if high < 500 => -1,
        _ => 0,
    };
    let conditional_direction = match conditional_wilson {
        Some((low, _)) if low > 500 => 1,
        Some((_, high)) if high < 500 => -1,
        _ => 0,
    };
    if a_direction != 0 && conditional_direction == -a_direction {
        reversal = true;
        reversal_reason = format!(
            "REVERSAL: A's main effect one-sided {} (up {a_up}/down {a_down}) but the conditional \
             direction delta(A+B)-delta(B) is one-sided {} (up {cond_up}/down {cond_down})",
            if a_direction > 0 { "up" } else { "down" },
            if conditional_direction > 0 {
                "up"
            } else {
                "down"
            },
        );
    } else {
        reversal_reason.push_str(&format!(
            "A direction {a_direction}, conditional direction {conditional_direction} \
             (raw A up {a_up}/down {a_down}; conditional up {cond_up}/down {cond_down})"
        ));
    }
    Ok(InteractionDid {
        factor_a: a.variant_id.clone(),
        factor_b: b.variant_id.clone(),
        combined: combined.variant_id.clone(),
        rows: rows.into_values().collect(),
        a_wilson,
        conditional_wilson,
        reversal,
        reversal_reason,
    })
}

// ---------------------------------------------------------------------
// The effects.txt renderer (machine-recomputable; raw counts everywhere)
// ---------------------------------------------------------------------

/// Renders the complete `effects.txt` of one B4 experiment: gate
/// evaluations (records-integrity, no-crashes, command-drop reusing the
/// readiness evaluators) and, per candidate, the paired-effect tables,
/// map-clustered accounting, fixed-seed bootstrap, side-effect
/// guardrails, geometry strata and the stability verdict (an interaction
/// plan renders its DiD readout instead of stability). Interaction-stage
/// plans need exactly [base, A, B, A+B].
pub fn effects_text(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    pairs: &[PairedResult],
    archived: &[(String, u32, MapDefinition)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# B4 effects ({}.{}) — non-authoritative offline analysis (plan 9.3)\n",
        plan.experiment_id,
        plan.stage.tag()
    ));
    out.push_str("# integer raw counts everywhere; permille is display-only; nothing here enters Game Result, Replay Hash or settlement\n");
    out.push_str(&format!(
        "# bootstrap: {} x {} replicates, per-map resampling with replacement, seed domain-separated from (experiment_id, variant_id)\n\n",
        BOOTSTRAP_ALGORITHM,
        BOOTSTRAP_ITERATIONS
    ));

    // Gates (plan 10.1/10.2).
    let integrity = records_gate(records, plan);
    let drop = drop_gate(records);
    let crashes = records
        .iter()
        .filter(|r| r.result == BalanceOutcome::Played(Outcome::Crash))
        .count() as u64;
    let config_failures = records
        .iter()
        .filter(|r| r.result == BalanceOutcome::ConfigFailure)
        .count() as u64;
    let no_crashes = crate::balance::readiness::GateCount {
        gate_id: "no-crashes".to_string(),
        statement: "zero crash and zero config-failure records in the experiment".to_string(),
        numerator: crashes + config_failures,
        denominator: records.len() as u64,
        permille: if records.is_empty() {
            0
        } else {
            (crashes + config_failures) * 1000 / records.len() as u64
        },
        passed: crashes == 0 && config_failures == 0,
        detail: vec![
            format!("crashes={crashes}"),
            format!("config_failures={config_failures}"),
        ],
    };
    out.push_str("## gates\n");
    for gate in [&integrity, &no_crashes, &drop] {
        out.push_str(&format!(
            "  {} {} — {}\n",
            gate.count_line(),
            if gate.passed { "PASS" } else { "FAIL" },
            gate.statement
        ));
    }
    out.push('\n');

    // Interaction plans render the DiD readout.
    if plan.stage == crate::balance::plan::Stage::Interaction {
        match interaction_did(plan, pairs) {
            Ok(did) => {
                out.push_str(&format!(
                    "## interaction DiD — A={} B={} A+B={}\n",
                    did.factor_a, did.factor_b, did.combined
                ));
                out.push_str(&format!(
                    "  reversal={} — {}\n",
                    did.reversal, did.reversal_reason
                ));
                if let Some((low, high)) = did.a_wilson {
                    out.push_str(&format!(
                        "  A main-effect Wilson95 [{low},{high}] permille (raw)\n"
                    ));
                }
                if let Some((low, high)) = did.conditional_wilson {
                    out.push_str(&format!(
                        "  conditional Wilson95 [{low},{high}] permille (raw)\n"
                    ));
                }
                out.push_str("  pair units margin_did_sum did_up did_down did_zero outcome_did_sum cond_up cond_down cond_zero\n");
                for row in &did.rows {
                    out.push_str(&format!(
                        "  {} {} {} {} {} {} {} {} {} {}\n",
                        row.pair_tag,
                        row.units,
                        row.margin_did_sum,
                        row.margin_did_up,
                        row.margin_did_down,
                        row.margin_did_zero,
                        row.outcome_did_sum,
                        row.conditional_up,
                        row.conditional_down,
                        row.conditional_zero
                    ));
                }
                return out;
            }
            Err(error) => {
                out.push_str(&format!("## interaction DiD — ERROR: {error}\n"));
                return out;
            }
        }
    }

    // Sensitivity: per-candidate blocks.
    let effects = paired_effects(plan, pairs);
    let verdicts = stability_from_effects(plan, pairs, records, &effects);
    let geometry = geometry_strata(plan, pairs, archived);
    for variant in &plan.variants {
        if variant.variant_id == plan.base_variant {
            continue;
        }
        let id = variant.variant_id.as_str();
        let map_regime_note = if variant.is_map_regime() {
            " — MAP-REGIME variant: paired deltas include the map-regime change (ADR-0018)"
        } else {
            ""
        };
        out.push_str(&format!("## candidate {id}{map_regime_note}\n"));

        out.push_str("  pair units played excl_crash excl_cfg excl_base m_up m_down m_zero o_up o_down o_zero m_delta_sum o_delta_sum wilson_low wilson_high quartiles(p25/p50/p75 min max)\n");
        for effect in effects.iter().filter(|e| e.variant_id == id) {
            // `None` (no decisive unit) renders as "-"; a fabricated 0/0
            // would read as a fully one-sided-down interval.
            let (low_text, high_text) = match effect.wilson {
                Some((low, high)) => (low.to_string(), high.to_string()),
                None => ("-".to_string(), "-".to_string()),
            };
            let quartiles = match &effect.margin_quartiles {
                Some(q) => format!("{}/{}/{} {} {}", q.p25, q.median, q.p75, q.min, q.max),
                None => "-".to_string(),
            };
            out.push_str(&format!(
                "  {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
                effect.pair_tag,
                effect.units,
                effect.played_units,
                effect.excluded_variant_crash,
                effect.excluded_variant_config_failure,
                effect.excluded_base_not_settled,
                effect.margin_up,
                effect.margin_down,
                effect.margin_zero,
                effect.outcome_up,
                effect.outcome_down,
                effect.outcome_zero,
                effect.margin_delta_sum,
                effect.outcome_delta_sum,
                low_text,
                high_text
            ));
            out.push_str(&format!("    quartiles {quartiles}\n"));
        }

        // Map-clustered accounting + bootstrap.
        let per_map = deltas_per_map(plan, pairs, id);
        let units: u64 = per_map.values().map(|d| d.len() as u64).sum();
        let maps = per_map.len() as u64;
        let bootstrap = map_clustered_bootstrap(&plan.experiment_id, id, &per_map);
        out.push_str(&format!(
            "  accounting: maps={maps} units={units} (same-map repeats are never independent maps)\n"
        ));
        out.push_str(&format!(
            "  bootstrap {} iterations={} seed={:#018x} p025={} p500={} p975={} (integer nearest-rank of the replicate sums)\n",
            bootstrap.algorithm, bootstrap.iterations, bootstrap.seed, bootstrap.p025, bootstrap.p500, bootstrap.p975
        ));

        // Side-effect guardrails vs base.
        let side = side_effects(plan, records, id);
        out.push_str(&format!(
            "  side-effects: records={} settled={} crashes={} config_failures={} both_zero={}/base {} draws={}/base {} enl_wins={} res_wins={} drop={}/{}={}permille band<50={}\n",
            side.records,
            side.settled,
            side.crashes,
            side.config_failures,
            side.both_zero,
            side.both_zero_base,
            side.draws,
            side.draws_base,
            side.enl_wins,
            side.res_wins,
            side.drop_dropped,
            side.drop_submitted,
            side.drop_permille,
            side.drop_below_band
        ));
        out.push_str("  mechanics: field variant base delta_permille\n");
        for row in &side.mechanics {
            let delta = row
                .delta_permille
                .map(|d| d.to_string())
                .unwrap_or_else(|| "-".to_string());
            out.push_str(&format!(
                "    {} {} {} {}\n",
                row.field, row.variant_sum, row.base_sum, delta
            ));
        }
        for flag in &side.flags {
            out.push_str(&format!("  SIDE-EFFECT FLAG: {flag}\n"));
        }

        // Geometry strata (plan 11 map row).
        out.push_str(&format!(
            "  geometry strata (nearest-neighbor median cm, tercile cuts {} / {}): stratum up down zero\n",
            geometry.cut_low, geometry.cut_high
        ));
        for row in geometry.rows.iter().filter(|r| r.variant_id == id) {
            out.push_str(&format!(
                "    {} {} {} {}\n",
                row.stratum, row.up, row.down, row.zero
            ));
        }

        // Stability verdict.
        if let Some(verdict) = verdicts.iter().find(|v| v.variant_id == id) {
            out.push_str(&format!(
                "  stability: stable={} direction={} median_margin_delta={} drop={} crashes={} config_failures={}\n",
                verdict.stable,
                verdict.direction,
                verdict.margin_median,
                verdict.drop_permille,
                verdict.crashes,
                verdict.config_failures
            ));
            for (pair, side) in &verdict.agreeing_pairs {
                out.push_str(&format!("    one-sided pair {pair} ({side})\n"));
            }
            for (pair, side) in &verdict.dissenting_pairs {
                out.push_str(&format!(
                    "    one-sided DISSENTING pair {pair} ({side}) — recorded, plan 10.3 needs >=3 agreeing, not unanimity\n"
                ));
            }
            if verdict.stable {
                out.push_str("    verdict: STABLE — enters the Full roster (stable_candidates)\n");
            } else {
                for reason in &verdict.failed {
                    out.push_str(&format!("    STOPPED: {reason}\n"));
                }
            }
        }
        out.push('\n');
    }

    // The derived Full roster (the audit bridge).
    if plan.stage == crate::balance::plan::Stage::Sensitivity {
        let roster = stable_candidates(&verdicts);
        if roster.is_empty() {
            out.push_str("full_roster: EMPTY — no candidate survived the stability rule; every direction is stopped and recorded above (plan 7.1 else-branch)\n");
        } else {
            out.push_str(&format!(
                "full_roster: {} (derived, the only pilot->Full bridge)\n",
                roster.join(",")
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balance::pairs::PairedResult;
    use crate::balance::plan::{PolicyMatchup, SeedSet, Stage};
    use crate::balance::record::BalanceMatchRecord;
    use crate::balance::sensitivity::{B4_CANDIDATES, candidate_variant};
    use crate::policy::BucketPolicy;
    use arena_bots::policy::{PolicyFamily, PolicyId, PolicyParameters};

    /// A tiny sensitivity plan: base + the named grid candidates, one
    /// matchup per pair tag supplied by the test.
    fn sensitivity_plan(
        experiment_id: &str,
        candidate_ids: &[&str],
        pair_tags: &[&str],
    ) -> ExperimentPlan {
        let variants = std::iter::once(crate::balance::sensitivity::b4_base_variant())
            .chain(candidate_ids.iter().map(|id| {
                candidate_variant(B4_CANDIDATES.iter().find(|c| c.variant_id == *id).unwrap())
            }))
            .collect();
        ExperimentPlan {
            experiment_id: experiment_id.to_string(),
            stage: Stage::Sensitivity,
            seed_set: SeedSet::Development,
            map_count: 4,
            base_variant: "default".to_string(),
            variants,
            policies: vec![
                PolicyId::GreedyCoordinatedR2.r2_spec(),
                PolicyId::BuilderDefenderR2.r2_spec(),
            ],
            matchups: pair_tags
                .iter()
                .map(|tag| {
                    let mut parts = tag.split("--");
                    let a = parts.next().unwrap();
                    let b = parts.next().unwrap();
                    PolicyMatchup::new(
                        tag,
                        PolicyId::from_tag(a).unwrap(),
                        PolicyId::from_tag(b).unwrap(),
                    )
                })
                .collect(),
            bucket_policy: BucketPolicy::Flat(0),
            gates: Vec::new(),
        }
    }

    /// A hand-built settled pairs row: margin_delta and outcome_delta
    /// chosen by the test.
    fn settled_row(
        plan: &ExperimentPlan,
        variant_id: &str,
        pair_tag: &str,
        map_index: u32,
        margin_delta: i128,
        outcome_delta: i64,
    ) -> PairedResult {
        PairedResult {
            pair_id: format!("{pair_tag}#m{map_index:03}"),
            pair_tag: pair_tag.to_string(),
            variant_id: variant_id.to_string(),
            base_variant_id: plan.base_variant.clone(),
            map_index,
            legs: 2,
            policy_a_wins: 1,
            policy_b_wins: 1,
            draws: 0,
            crashes: 0,
            config_failures: 0,
            a_score2_leg0: 100,
            a_score2_leg1: 60,
            b_score2_leg0: 60,
            b_score2_leg1: 100,
            margin_sum: 0,
            outcome_delta_vs_base: outcome_delta,
            margin_delta_vs_base: margin_delta,
        }
    }

    /// A hand-built match record with minimal identity and chosen stats.
    fn match_record(
        plan: &ExperimentPlan,
        variant_id: &str,
        result: BalanceOutcome,
        enl_score: u128,
        res_score: u128,
        stats: crate::metrics::MatchStats,
    ) -> BalanceMatchRecord {
        BalanceMatchRecord {
            index: 0,
            experiment_id: plan.experiment_id.clone(),
            seed_set_tag: plan.seed_set.tag().to_string(),
            variant_id: variant_id.to_string(),
            pair_tag: "greedy-coordinated-r2--builder-defender-r2".to_string(),
            pair_id: String::new(),
            leg: 0,
            map_id: String::new(),
            map_index: 0,
            policy_a: PolicyId::GreedyCoordinatedR2,
            policy_a_family: PolicyFamily::FieldBuilder,
            policy_a_version: "r2.0".to_string(),
            policy_a_parameters: PolicyParameters::GreedyCoordinated(Default::default()),
            policy_b: PolicyId::BuilderDefenderR2,
            policy_b_family: PolicyFamily::Defender,
            policy_b_version: "r2.0".to_string(),
            policy_b_parameters: PolicyParameters::BuilderDefender(Default::default()),
            policy_a_is_enl: true,
            map_seed: 1,
            game_seed: 2,
            bot_seed: 3,
            schedule_seed: 4,
            bucket_policy: BucketPolicy::Flat(0),
            config_hash: 5,
            result,
            enl_score2: enl_score,
            res_score2: res_score,
            final_state_hash: [0u8; 32],
            stats,
            elapsed_ms: 0,
        }
    }

    fn stats_with(submitted: u64, dropped: u64, transfers: u64) -> crate::metrics::MatchStats {
        let mut stats = crate::metrics::MatchStats::new();
        for side in [&mut stats.enl, &mut stats.res] {
            side.commands_submitted = submitted;
            side.commands_dropped = dropped;
            side.transfers = transfers;
            side.fields_created = 3;
            side.first_field_round = Some(10);
            side.destroyed_lifetime_rounds_sum = 100;
            side.surviving_lifetime_rounds_sum = 200;
        }
        stats
    }

    // ------------------------------------------------------------------
    // Paired effects: keyed by variant AND pair; hand arithmetic exact
    // ------------------------------------------------------------------

    #[test]
    fn paired_effects_are_keyed_by_variant_and_pair_and_match_hand_arithmetic() {
        let pair_a = "greedy-coordinated-r2--builder-defender-r2";
        let pair_b = "greedy-coordinated-r2--builder-disruptor-r2";
        let plan = sensitivity_plan(
            "effects-keying",
            &["attack-damage-25", "initial-supply-40"],
            &[pair_a, pair_b],
        );
        // attack-damage-25: pair_a +100 x2 units, -50 x1, 0 x1 (one
        // config-failure unit excluded); pair_b +30 x2.
        // initial-supply-40: pair_a -70 x4.
        let mut rows = Vec::new();
        for (pair_tag, map, delta) in [
            (pair_a, 0u32, 100i128),
            (pair_a, 1, 100),
            (pair_a, 2, -50),
            (pair_a, 3, 0),
            (pair_b, 0, 30),
            (pair_b, 1, 30),
        ] {
            rows.push(settled_row(
                &plan,
                "attack-damage-25",
                pair_tag,
                map,
                delta,
                0,
            ));
        }
        // One config-failure unit of attack-damage-25 on pair_b (excluded,
        // counted — and its fake zeros never enter the statistics).
        let mut failed = settled_row(&plan, "attack-damage-25", pair_b, 2, 0, 0);
        failed.config_failures = 2;
        failed.margin_delta_vs_base = -999_999;
        rows.push(failed);
        // Base rows for every unit (settled, delta 0 by construction).
        for pair_tag in [pair_a, pair_b] {
            for map in 0..4u32 {
                rows.push(settled_row(&plan, "default", pair_tag, map, 0, 0));
            }
        }
        for map in 0..4u32 {
            rows.push(settled_row(
                &plan,
                "initial-supply-40",
                pair_a,
                map,
                -70,
                -1,
            ));
        }

        let effects = paired_effects(&plan, &rows);
        // Declaration order: attack-damage-25 (pair_a, pair_b), then
        // initial-supply-40 (pair_a, pair_b).
        assert_eq!(effects.len(), 4);
        let a0 = &effects[0];
        assert_eq!(a0.variant_id, "attack-damage-25");
        assert_eq!(a0.pair_tag, pair_a);
        assert_eq!(a0.units, 4);
        assert_eq!(a0.played_units, 4);
        assert_eq!(a0.margin_up, 2);
        assert_eq!(a0.margin_down, 1);
        assert_eq!(a0.margin_zero, 1);
        // Hand sums: +100 +100 -50 +0 = +150.
        assert_eq!(a0.margin_delta_sum, 150);
        // Quartiles of [100, 100, -50, 0] ascending [-50, 0, 100, 100]:
        // nearest-rank p25 = rank ceil(0.25*4)=1 -> -50; p50 = rank 2 -> 0;
        // p75 = rank 3 -> 100.
        let quartiles = a0.margin_quartiles.as_ref().unwrap();
        assert_eq!(quartiles.p25, -50);
        assert_eq!(quartiles.median, 0);
        assert_eq!(quartiles.p75, 100);
        assert_eq!(quartiles.min, -50);
        assert_eq!(quartiles.max, 100);
        // Wilson on 2/(2+1): lower <= 679 <= upper (computed, exact bounds
        // are analysis.rs' job) — assert the interval exists and is one
        // of the two expected kinds.
        let (low, high) = a0.wilson.unwrap();
        assert!(
            low <= 500 && high > 500,
            "2/3 is not one-sided: [{low},{high}]"
        );
        assert_eq!(a0.one_sided(), 0);

        let a1 = &effects[1];
        assert_eq!(a1.pair_tag, pair_b);
        assert_eq!(a1.units, 3);
        assert_eq!(a1.played_units, 2);
        assert_eq!(a1.excluded_variant_config_failure, 1);
        assert_eq!(a1.margin_up, 2);
        assert_eq!(
            a1.margin_delta_sum,
            30 + 30 - 999_999,
            "sums use every unit row"
        );
        assert_eq!(a1.margin_quartiles.as_ref().unwrap().units, 2);

        let b0 = &effects[2];
        assert_eq!(b0.variant_id, "initial-supply-40");
        assert_eq!(b0.margin_down, 4);
        assert_eq!(b0.outcome_down, 4);
        assert_eq!(b0.margin_delta_sum, -280);
        // 0/4 decisive -> Wilson entirely below 500: one-sided -1.
        assert_eq!(b0.one_sided(), -1);

        // The variant-pooling wash-out companion: B3's pair_summaries
        // pools across variants, so the same records through that lens
        // cannot reproduce the per-variant keying.
        let pooled: Vec<&PairedResult> = rows.iter().filter(|r| r.pair_tag == pair_a).collect();
        let up_pooled = pooled
            .iter()
            .filter(|r| r.margin_delta_vs_base > 0 && r.variant_id != "default")
            .count();
        assert_eq!(
            up_pooled, 2,
            "the hand rows only make sense per-variant; any cross-variant pool mixes keys"
        );

        // Excluded BASE rows also exclude the unit.
        let mut base_crashed = settled_row(&plan, "default", pair_b, 3, 0, 0);
        base_crashed.crashes = 1;
        let mut variant_row = settled_row(&plan, "attack-damage-25", pair_b, 3, 42, 0);
        variant_row.pair_id = base_crashed.pair_id.clone();
        let mut rows2 = rows.clone();
        rows2.retain(|r| {
            !((r.variant_id == "default" && r.pair_tag == pair_b && r.map_index == 3)
                || (r.variant_id == "attack-damage-25" && r.pair_tag == pair_b))
        });
        rows2.push(base_crashed);
        rows2.push(variant_row);
        let effects2 = paired_effects(&plan, &rows2);
        let a1b = effects2
            .iter()
            .find(|e| e.variant_id == "attack-damage-25" && e.pair_tag == pair_b)
            .unwrap();
        assert_eq!(
            a1b.excluded_base_not_settled, 1,
            "a crashed base excludes the unit"
        );
        assert_eq!(a1b.played_units, 0);
    }

    // ------------------------------------------------------------------
    // Bootstrap: deterministic, seed-tagged, map-clustered, integer
    // ------------------------------------------------------------------

    #[test]
    fn bootstrap_is_deterministic_seed_tagged_and_map_clustered() {
        // Two maps: map 0 carries ALL of (+10, +20) = +30; map 1 carries
        // (-5). Every replicate draws 2 map indices; a drawn map
        // contributes its WHOLE delta vector.
        let mut per_map = BTreeMap::new();
        per_map.insert(0u32, vec![10i128, 20]);
        per_map.insert(1u32, vec![-5i128]);
        let first = map_clustered_bootstrap("b4-x", "attack-damage-25", &per_map);
        let again = map_clustered_bootstrap("b4-x", "attack-damage-25", &per_map);
        assert_eq!(
            first, again,
            "identical inputs and tags -> identical output"
        );
        assert_eq!(first.algorithm, BOOTSTRAP_ALGORITHM);
        assert_eq!(first.iterations, BOOTSTRAP_ITERATIONS);
        assert_eq!(first.replicate_sums.len(), BOOTSTRAP_ITERATIONS as usize);
        assert_eq!(first.maps, 2);
        assert_eq!(first.units, 3, "2 + 1 deltas");

        // Possible replicate sums: map multisets of size 2 over {+30, -5}:
        // {0,0}->+60 {0,1}->+25 {1,1}->-10. Every replicate is one of
        // these exact integers (a drawn map carried ALL its deltas).
        for sum in &first.replicate_sums {
            assert!(
                *sum == 60 || *sum == 25 || *sum == -10,
                "replicate {sum} is not a whole-map combination"
            );
        }
        for value in [first.p025, first.p500, first.p975] {
            assert!(
                value == -10 || value == 25 || value == 60,
                "integer percentile {value}"
            );
        }
        assert!(first.p025 <= first.p500 && first.p500 <= first.p975);

        // Different (experiment, variant) tag -> different seed -> a
        // different stream (domain separation; the tags alone move it).
        let other_variant = map_clustered_bootstrap("b4-x", "portal-count-80", &per_map);
        assert_ne!(first.seed, other_variant.seed);
        let other_experiment = map_clustered_bootstrap("b4-y", "attack-damage-25", &per_map);
        assert_ne!(first.seed, other_experiment.seed);
        // The seed never derives from the data: same tags, different
        // deltas -> same seed.
        let mut other_data = BTreeMap::new();
        other_data.insert(0u32, vec![999i128]);
        let same_tags = map_clustered_bootstrap("b4-x", "attack-damage-25", &other_data);
        assert_eq!(first.seed, same_tags.seed);

        // The 6-pair x 100-map shape keeps maps and units distinct.
        let mut full = BTreeMap::new();
        for k in 0..100u32 {
            full.insert(k, vec![7i128; 6]);
        }
        let outcome = map_clustered_bootstrap("b4-sensitivity-pilot-r1", "recharge-cost-2", &full);
        assert_eq!(outcome.maps, 100);
        assert_eq!(outcome.units, 600);
        // Every replicate resampled exactly 100 maps x 6 deltas.
        for sum in outcome.replicate_sums.iter().take(50) {
            assert_eq!(*sum, 100 * 6 * 7);
        }
    }

    // ------------------------------------------------------------------
    // Side-effect guardrails
    // ------------------------------------------------------------------

    #[test]
    fn side_effects_flag_an_unexplained_regression() {
        let pair_a = "greedy-coordinated-r2--builder-defender-r2";
        let plan = sensitivity_plan("side-effects", &["recharge-cost-2"], &[pair_a]);
        // Base: 2 settled matches, no no-score, no draws, drop 1/200.
        let base_good = stats_with(100, 1, 5);
        let mut records = vec![
            match_record(
                &plan,
                "default",
                BalanceOutcome::Played(Outcome::EnlWin),
                100,
                40,
                base_good.clone(),
            ),
            match_record(
                &plan,
                "default",
                BalanceOutcome::Played(Outcome::ResWin),
                40,
                100,
                base_good,
            ),
        ];
        // Variant: doubled both-zero matches (2/2), drops 20/200 (=100
        // permille, over the band), transfers collapsed 5 -> 0.
        let variant_bad = stats_with(100, 10, 0);
        records.extend(vec![
            match_record(
                &plan,
                "recharge-cost-2",
                BalanceOutcome::Played(Outcome::Draw),
                0,
                0,
                variant_bad.clone(),
            ),
            match_record(
                &plan,
                "recharge-cost-2",
                BalanceOutcome::Played(Outcome::Draw),
                0,
                0,
                variant_bad,
            ),
        ]);
        let side = side_effects(&plan, &records, "recharge-cost-2");
        assert_eq!(side.settled, 2);
        assert_eq!(side.both_zero, 2);
        assert_eq!(side.both_zero_base, 0, "BOTH raw counts carried");
        assert_eq!(side.draws, 2);
        assert_eq!(side.draws_base, 0);
        assert_eq!(side.drop_dropped, 40);
        assert_eq!(side.drop_submitted, 400);
        assert_eq!(side.drop_permille, 100);
        assert!(!side.drop_below_band);
        // Flags name the frozen regressions.
        assert!(
            side.flags
                .iter()
                .any(|f| f.contains("no-score matches rose 0 -> 2"))
        );
        assert!(side.flags.iter().any(|f| f.contains("draws rose 0 -> 2")));
        assert!(
            side.flags
                .iter()
                .any(|f| f.contains("40/400 = 100 permille >= 50"))
        );
        // Mechanics carry variant AND base sums with permille beside.
        let transfers = side
            .mechanics
            .iter()
            .find(|m| m.field == "transfers")
            .unwrap();
        assert_eq!(transfers.variant_sum, 0);
        assert_eq!(transfers.base_sum, 20, "2 matches x 2 factions x 5");
        assert_eq!(transfers.delta_permille, Some(-1000));
        let fields = side
            .mechanics
            .iter()
            .find(|m| m.field == "fields_created")
            .unwrap();
        assert_eq!(fields.variant_sum, 12);
        assert_eq!(fields.base_sum, 12);
        assert_eq!(fields.delta_permille, Some(0));

        // An identical-to-base variant: zero deltas, no flags.
        let mut clean = vec![
            match_record(
                &plan,
                "default",
                BalanceOutcome::Played(Outcome::EnlWin),
                100,
                40,
                stats_with(100, 1, 5),
            ),
            match_record(
                &plan,
                "default",
                BalanceOutcome::Played(Outcome::EnlWin),
                100,
                40,
                stats_with(100, 1, 5),
            ),
        ];
        let variant_id = "recharge-cost-2";
        clean.push(match_record(
            &plan,
            variant_id,
            BalanceOutcome::Played(Outcome::EnlWin),
            100,
            40,
            stats_with(100, 1, 5),
        ));
        clean.push(match_record(
            &plan,
            variant_id,
            BalanceOutcome::Played(Outcome::EnlWin),
            100,
            40,
            stats_with(100, 1, 5),
        ));
        let identical = side_effects(&plan, &clean, variant_id);
        assert_eq!(identical.both_zero, identical.both_zero_base);
        assert_eq!(identical.draws, identical.draws_base);
        assert!(identical.drop_below_band);
        // Identical sums: 0-permille delta wherever the base used the
        // mechanism, and an undefined (None) delta where BOTH sides are 0
        // (permille never replaces the raw counts).
        assert!(
            identical
                .mechanics
                .iter()
                .all(|m| m.delta_permille == Some(0) || m.base_sum == 0)
        );
        assert!(
            identical
                .mechanics
                .iter()
                .all(|m| m.variant_sum == m.base_sum)
        );
        assert!(identical.flags.is_empty(), "no unexplained regression");

        // Boundary: exactly 50 permille drop is OUTSIDE the band (< 50).
        let boundary = stats_with(200, 10, 0); // 10 dropped per faction / 200 submitted
        let mut boundary_records = vec![match_record(
            &plan,
            "default",
            BalanceOutcome::Played(Outcome::EnlWin),
            10,
            0,
            stats_with(200, 0, 0),
        )];
        boundary_records.push(match_record(
            &plan,
            variant_id,
            BalanceOutcome::Played(Outcome::EnlWin),
            10,
            0,
            boundary,
        ));
        let at_band = side_effects(&plan, &boundary_records, variant_id);
        assert_eq!(at_band.drop_dropped, 20);
        assert_eq!(at_band.drop_submitted, 400);
        assert_eq!(at_band.drop_permille, 50);
        assert!(!at_band.drop_below_band, "the band is strictly below 50");
    }

    // ------------------------------------------------------------------
    // The stability rule and the Full-roster bridge
    // ------------------------------------------------------------------

    #[test]
    fn stability_rule_requires_three_agreeing_pairs_and_derives_the_full_roster() {
        let pairs_tags = [
            "greedy-coordinated-r2--builder-defender-r2",
            "greedy-coordinated-r2--builder-disruptor-r2",
            "builder-defender-r2--builder-disruptor-r2",
            "greedy-coordinated-r2--logistics-builder-r2",
            "logistics-builder-r2--blocker-builder-r2",
            "builder-disruptor-r2--adaptive-hybrid-r2",
        ];
        let plan = sensitivity_plan(
            "stability",
            &[
                "attack-damage-25",
                "recharge-amount-10",
                "initial-supply-10",
                "max-link-2500m",
            ],
            &pairs_tags,
        );
        let mut rows = Vec::new();
        // Base everywhere (8 maps per pair — 8/8 decisive units make a
        // Wilson one-sided pair: 8-0 gives lower 675 permille).
        for pair in pairs_tags {
            for map in 0..8u32 {
                rows.push(settled_row(&plan, "default", pair, map, 0, 0));
            }
        }
        // attack-damage-25: 4 agreeing pairs, 8/0 up each -> stable.
        for pair in pairs_tags {
            for map in 0..8u32 {
                rows.push(settled_row(&plan, "attack-damage-25", pair, map, 250, 2));
            }
        }
        // recharge-amount-10: pairs 0-1 one-sided down, pairs 2-3 split
        // -> only 2 one-sided pairs -> stopped.
        for (index, pair) in pairs_tags.iter().enumerate() {
            for map in 0..8u32 {
                let delta = if index < 2 {
                    -80
                } else if map % 2 == 0 {
                    1
                } else {
                    -1
                };
                rows.push(settled_row(
                    &plan,
                    "recharge-amount-10",
                    pair,
                    map,
                    delta,
                    -2,
                ));
            }
        }
        // initial-supply-10: pairs 0-2 one-sided +, pair 3 one-sided -,
        // pairs 4-5 split — the plan-10.3 threshold (>= 3 pairs one
        // direction) holds; the dissent is recorded, arm stable.
        for (index, pair) in pairs_tags.iter().enumerate() {
            for map in 0..8u32 {
                let delta = match index {
                    0 | 1 | 2 => 90,
                    3 => -90,
                    _ if map % 2 == 0 => 1,
                    _ => -1,
                };
                rows.push(settled_row(&plan, "initial-supply-10", pair, map, delta, 1));
            }
        }
        // max-link-2500m: a 3+3 tie of one-sided pairs — no direction.
        for (index, pair) in pairs_tags.iter().enumerate() {
            for map in 0..8u32 {
                let delta = if index < 3 { 70 } else { -70 };
                rows.push(settled_row(&plan, "max-link-2500m", pair, map, delta, 1));
            }
        }
        // Clean records for every variant (drops well under the band).
        let mut records = Vec::new();
        for variant in [
            "default",
            "attack-damage-25",
            "recharge-amount-10",
            "initial-supply-10",
        ] {
            for _ in 0..8 {
                records.push(match_record(
                    &plan,
                    variant,
                    BalanceOutcome::Played(Outcome::EnlWin),
                    10,
                    0,
                    stats_with(1000, 1, 1),
                ));
            }
        }

        let verdicts = stability(&plan, &rows, &records);
        assert_eq!(verdicts.len(), 4);
        let stable = verdicts
            .iter()
            .find(|v| v.variant_id == "attack-damage-25")
            .unwrap();
        assert!(stable.stable, "failed legs: {:?}", stable.failed);
        assert_eq!(stable.direction, 1);
        assert_eq!(stable.agreeing_pairs.len(), 6);
        assert_eq!(stable.margin_median, 250);
        assert_eq!(stable.drop_permille, 1);

        let two_pairs = verdicts
            .iter()
            .find(|v| v.variant_id == "recharge-amount-10")
            .unwrap();
        assert!(!two_pairs.stable);
        assert!(
            two_pairs.failed.iter().any(
                |f| f.contains("2 one-sided pair(s) with one direction (need >= 3 of 6 pairs)")
            )
        );

        let dissent = verdicts
            .iter()
            .find(|v| v.variant_id == "initial-supply-10")
            .unwrap();
        assert!(
            dissent.stable,
            "3 of 4 one-sided pairs agreeing passes the plan-10.3 threshold"
        );
        assert_eq!(dissent.direction, 1);
        assert_eq!(
            dissent.dissenting_pairs,
            vec![(pairs_tags[3].to_string(), -1)],
            "the dissenting one-sided pair is recorded, never hidden"
        );

        let tie = verdicts
            .iter()
            .find(|v| v.variant_id == "max-link-2500m")
            .unwrap();
        assert!(!tie.stable, "a 3+3 tie has no direction");
        assert!(tie.failed.iter().any(|f| f.contains("tie")));

        // The roster bridge carries EXACTLY the stable arms, in plan order.
        assert_eq!(
            stable_candidates(&verdicts),
            vec![
                "attack-damage-25".to_string(),
                "initial-supply-10".to_string()
            ]
        );
        // And the full builder refuses an empty roster (the recorded
        // reason is pinned in sensitivity.rs's own tests).
        let empty = stable_candidates(&[]);
        assert!(empty.is_empty());
        assert!(
            crate::balance::sensitivity::b4_full_plan(
                SeedSet::Development,
                300,
                &empty,
                "b4-sensitivity-full-dev-r1"
            )
            .is_err()
        );
    }

    #[test]
    fn stability_stops_on_failures_drop_band_and_zero_median() {
        let pair_a = "greedy-coordinated-r2--builder-defender-r2";
        let pair_b = "greedy-coordinated-r2--builder-disruptor-r2";
        let pair_c = "builder-defender-r2--builder-disruptor-r2";
        let plan = sensitivity_plan(
            "stability-stops",
            &["max-link-2500m"],
            &[pair_a, pair_b, pair_c],
        );
        let mut rows = Vec::new();
        for pair in [pair_a, pair_b, pair_c] {
            for map in 0..4u32 {
                rows.push(settled_row(&plan, "default", pair, map, 0, 0));
                // Median 0: each pair contributes +100, -100, 0, 0 —
                // pooled 12 values sort to [-100x3, 0x6, +100x3] whose
                // nearest-rank median (rank 6 of 12) is exactly 0.
                let delta = match map {
                    0 => 100,
                    1 => -100,
                    _ => 0,
                };
                rows.push(settled_row(&plan, "max-link-2500m", pair, map, delta, 0));
            }
        }
        // One crash + one config failure + a 100-permille drop.
        let mut records = Vec::new();
        for _ in 0..4 {
            records.push(match_record(
                &plan,
                "default",
                BalanceOutcome::Played(Outcome::EnlWin),
                10,
                0,
                stats_with(1000, 0, 0),
            ));
        }
        for index in 0..4 {
            let result = if index == 0 {
                BalanceOutcome::Played(Outcome::Crash)
            } else if index == 1 {
                BalanceOutcome::ConfigFailure
            } else {
                BalanceOutcome::Played(Outcome::EnlWin)
            };
            records.push(match_record(
                &plan,
                "max-link-2500m",
                result,
                10,
                0,
                stats_with(1000, 100, 0),
            ));
        }
        let verdicts = stability(&plan, &rows, &records);
        let verdict = verdicts.first().unwrap();
        assert!(!verdict.stable);
        assert!(
            verdict
                .failed
                .iter()
                .any(|f| f.contains("1 crashes, 1 config failures"))
        );
        assert!(
            verdict
                .failed
                .iter()
                .any(|f| f.contains("100 permille >= 50"))
        );
        assert!(
            verdict
                .failed
                .iter()
                .any(|f| f.contains("median margin_delta = 0"))
        );
        assert_eq!(verdict.crashes, 1);
        assert_eq!(verdict.config_failures, 1);
    }

    // ------------------------------------------------------------------
    // Geometry stratification (the ONE frozen stratification)
    // ------------------------------------------------------------------

    fn map_with_positions(positions: &[(i32, i32)]) -> MapDefinition {
        use arena_core::types::Portal;
        use arena_core::types::{Owner, PortalId};
        positions
            .iter()
            .enumerate()
            .map(|(index, (x, y))| Portal {
                id: PortalId(index as u32),
                position: arena_core::types::Coord::new(*x, *y),
                owner: Owner::Neutral,
                hp: 100,
            })
            .collect::<Vec<_>>()
            .pipe(MapDefinition::new)
    }

    /// Small pipe helper (keeps the map builder a one-liner).
    trait Pipe: Sized {
        fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
            f(self)
        }
    }
    impl<T> Pipe for T {}

    #[test]
    fn geometry_feature_is_the_median_nearest_neighbor_distance() {
        // A line of portals 1000 cm apart: every interior portal's nearest
        // neighbor is 1000 cm; the ends too. Median = 1000.
        let line = map_with_positions(&[(0, 0), (1000, 0), (2000, 0), (3000, 0)]);
        assert_eq!(nearest_neighbor_feature(&line), 1000);
        // Two portals 5000 cm apart: feature = 5000.
        let pair = map_with_positions(&[(0, 0), (3000, 4000)]);
        assert_eq!(nearest_neighbor_feature(&pair), 5000, "3-4-5 triangle");
        // One dense cluster beside one outlier: median picks the cluster.
        let clustered =
            map_with_positions(&[(0, 0), (100, 0), (0, 100), (100, 100), (500_000, 500_000)]);
        // Nearest neighbors: 100, 100, 100, 100, then the outlier's huge
        // distance — median (nearest-rank of 5, rank 3) = 100.
        assert_eq!(nearest_neighbor_feature(&clustered), 100);
    }

    #[test]
    fn geometry_strata_cut_on_base_terciles_and_count_directions() {
        let pair_a = "greedy-coordinated-r2--builder-defender-r2";
        let plan = sensitivity_plan("geometry", &["move-budget-240m"], &[pair_a]);
        // Five base maps with hand-positioned features 100..500 cm: the
        // frozen tercile cuts are nearest-rank p333/p667 of the five
        // features — rank ceil(.333*5)=2 -> 200 and ceil(.667*5)=4 -> 400.
        let line_at = |spacing: i32| map_with_positions(&[(0, 0), (spacing, 0), (90_000, 0)]);
        let base_maps = [
            line_at(100),
            line_at(200),
            line_at(300),
            line_at(400),
            line_at(500),
        ];
        assert_eq!(
            base_maps
                .iter()
                .map(nearest_neighbor_feature)
                .collect::<Vec<_>>(),
            vec![100, 200, 300, 400, 500]
        );
        let mut archived: Vec<(String, u32, MapDefinition)> = Vec::new();
        for (k, map) in base_maps.iter().enumerate() {
            archived.push(("default".to_string(), k as u32, map.clone()));
        }
        // Unit deltas by map: stratum 1 (features 100, 200) both down;
        // stratum 2 (300, 400) one up one zero; stratum 3 (500) up.
        let rows = (0..5u32)
            .flat_map(|map| {
                let delta = match map {
                    0 | 1 => -5,
                    2 => 7,
                    3 => 0,
                    _ => 9,
                };
                vec![
                    settled_row(&plan, "default", pair_a, map, 0, 0),
                    settled_row(&plan, "move-budget-240m", pair_a, map, delta, 0),
                ]
            })
            .collect::<Vec<_>>();
        let strata = geometry_strata(&plan, &rows, &archived);
        assert_eq!(strata.cut_low, 200);
        assert_eq!(strata.cut_high, 400);
        assert_eq!(strata.rows.len(), 3);
        for row in &strata.rows {
            match row.stratum {
                1 => assert_eq!((row.up, row.down, row.zero), (0, 2, 0)),
                2 => assert_eq!((row.up, row.down, row.zero), (1, 0, 1)),
                3 => assert_eq!((row.up, row.down, row.zero), (1, 0, 0)),
                other => panic!("stratum {other}"),
            }
        }
        // A variant without own rows resolves to the base map of the same
        // index (byte-shared, executor-verified) — map-regime variants
        // would carry their own rows.
        assert!(
            strata
                .rows
                .iter()
                .all(|r| r.variant_id == "move-budget-240m")
        );
    }

    // ------------------------------------------------------------------
    // Interaction difference-in-differences
    // ------------------------------------------------------------------

    fn interaction_rows(
        plan: &ExperimentPlan,
        pair_tag: &str,
        maps: u32,
        a_delta: i128,
        b_delta: i128,
        combined_delta: i128,
    ) -> Vec<PairedResult> {
        let mut rows = Vec::new();
        for map in 0..maps {
            rows.push(settled_row(&plan, "default", pair_tag, map, 0, 0));
            rows.push(settled_row(
                &plan,
                "move-budget-240m",
                pair_tag,
                map,
                a_delta,
                0,
            ));
            rows.push(settled_row(
                &plan,
                "max-link-2500m",
                pair_tag,
                map,
                b_delta,
                0,
            ));
            rows.push(settled_row(
                &plan,
                "move-budget-240m+max-link-2500m",
                pair_tag,
                map,
                combined_delta,
                0,
            ));
        }
        rows
    }

    #[test]
    fn interaction_did_detects_a_reversal_from_hand_built_units() {
        let id = crate::balance::sensitivity::interaction_experiment_id(
            "move_budget_cm",
            "max_link_distance_cm",
        );
        let a = B4_CANDIDATES
            .iter()
            .find(|c| c.variant_id == "move-budget-240m")
            .unwrap();
        let b = B4_CANDIDATES
            .iter()
            .find(|c| c.variant_id == "max-link-2500m")
            .unwrap();
        let plan = crate::balance::sensitivity::b4_interaction_plan(a, b, 8, &id);
        let pair_tag = "greedy-coordinated-r2--builder-defender-r2";
        // A alone: +100 on every unit (strongly up). B alone: 0.
        // A+B: -100 on every unit — adding A on top of B flips.
        let rows = interaction_rows(&plan, pair_tag, 8, 100, 0, -100);
        let did = interaction_did(&plan, &rows).expect("computes");
        assert_eq!(did.factor_a, "move-budget-240m");
        assert_eq!(did.factor_b, "max-link-2500m");
        assert!(
            did.reversal,
            "A's +100 effect flips under B: {}",
            did.reversal_reason
        );
        assert_eq!(did.a_direction(), 1);
        let row = did.rows.first().unwrap();
        assert_eq!(row.units, 8);
        // Exact integer DiD: -100 - 100 - 0 = -200 per unit, sum -1600.
        assert_eq!(row.margin_did_sum, -1600);
        assert_eq!(row.margin_did_down, 8);
        assert_eq!(row.margin_did_up, 0);
        assert_eq!(row.margin_did_zero, 0);
        // Conditional delta = -100 - 0 = -100 every unit.
        assert_eq!(row.conditional_down, 8);
        assert_eq!(row.conditional_up, 0);

        // Purely additive effects: DiD zeros everywhere, no flag.
        let additive = interaction_rows(&plan, pair_tag, 8, 100, 50, 150);
        let did = interaction_did(&plan, &additive).expect("computes");
        assert!(!did.reversal);
        let row = did.rows.first().unwrap();
        assert_eq!(row.margin_did_sum, 0);
        assert_eq!(row.margin_did_zero, 8);
        assert_eq!(row.margin_did_up, 0);
        assert_eq!(row.margin_did_down, 0);

        // A non-[base, A, B, A+B] plan is refused with a recorded reason.
        let bad = sensitivity_plan("not-interaction", &["attack-damage-25"], &[pair_tag]);
        assert!(interaction_did(&bad, &rows).is_err());
    }

    // ------------------------------------------------------------------
    // The renderer
    // ------------------------------------------------------------------

    #[test]
    fn effects_text_renders_every_candidate_and_the_gates() {
        let pair_a = "greedy-coordinated-r2--builder-defender-r2";
        let pair_b = "greedy-coordinated-r2--builder-disruptor-r2";
        let pair_c = "builder-defender-r2--builder-disruptor-r2";
        let plan = sensitivity_plan(
            "b4-render",
            &["attack-damage-25", "portal-count-80"],
            &[pair_a, pair_b, pair_c],
        );
        // 8 maps per pair: attack-damage-25 goes 8/0 up on every pair
        // (Wilson one-sided), portal-count-80 stays mixed.
        let mut rows = Vec::new();
        for pair in [pair_a, pair_b, pair_c] {
            for map in 0..8u32 {
                rows.push(settled_row(&plan, "default", pair, map, 0, 0));
                rows.push(settled_row(&plan, "attack-damage-25", pair, map, 55, 1));
                rows.push(settled_row(&plan, "portal-count-80", pair, map, -11, -1));
            }
        }
        let mut records = Vec::new();
        for variant in ["default", "attack-damage-25", "portal-count-80"] {
            for _ in 0..8 {
                records.push(match_record(
                    &plan,
                    variant,
                    BalanceOutcome::Played(Outcome::EnlWin),
                    10,
                    0,
                    stats_with(1000, 1, 1),
                ));
            }
        }
        let archived = vec![
            (
                "default".to_string(),
                0u32,
                map_with_positions(&[(0, 0), (100, 0), (1000, 0)]),
            ),
            (
                "portal-count-80".to_string(),
                0u32,
                map_with_positions(&[(0, 0), (5000, 0), (90000, 0)]),
            ),
        ];
        let text = effects_text(&plan, &records, &rows, &archived);
        // Gates section with the three frozen gate ids.
        for gate in ["records-integrity", "no-crashes", "command-drop"] {
            assert!(text.contains(gate), "gate {gate} rendered");
        }
        // A paired-effect AND a side-effect block for EVERY candidate
        // (the milestone's completion condition), plus the map-regime
        // caveat on the portal-count arm.
        for candidate in ["attack-damage-25", "portal-count-80"] {
            assert!(text.contains(&format!("## candidate {candidate}")));
            assert!(text.contains("side-effects:"));
        }
        assert!(text.contains("MAP-REGIME variant"));
        // Bootstrap echo: algorithm version, iterations, seed.
        assert!(text.contains(BOOTSTRAP_ALGORITHM));
        assert!(text.contains("iterations=10000"));
        assert!(text.contains("seed=0x"));
        // Accounting keeps maps and units distinct.
        assert!(text.contains("maps=8 units=24"));
        // The Full roster bridge is present.
        assert!(text.contains("full_roster: attack-damage-25"));
        // And stability verdicts with stop records where due.
        assert!(text.contains("stability:"));
    }
}
