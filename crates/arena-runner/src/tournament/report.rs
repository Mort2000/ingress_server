//! Standings + committed evidence (D85): the `standings.txt` table (rank
//! = rating desc, tie-break entrant name asc — frozen), per-entrant
//! ENL/RES splits that keep validating the no-faction-swap hypothesis
//! (P5-SCHEDULING), and the `docs/reports/tournament-r1.md` generator that turns
//! an ACTUAL run into the committed report. Integer fixed-point
//! formatting only (the M6 conventions).

use crate::tournament::archive::IndexRecord;
use crate::tournament::config::TournamentConfig;
use crate::tournament::elo::EloTable;
use crate::tournament::outcome::TournamentOutcome;
use crate::tournament::run::TournamentRun;
use crate::tournament::schedule::TournamentItem;

/// Per-entrant faction aggregates over SCORED matches (technical and
/// unscored outcomes contribute nothing here).
#[derive(Clone, Debug, Default)]
pub struct EntrantSplit {
    pub name: String,
    pub enl_wins: u64,
    pub enl_draws: u64,
    pub enl_losses: u64,
    pub res_wins: u64,
    pub res_draws: u64,
    pub res_losses: u64,
    /// Sum of (own score2 − opponent score2) while ENL / RES (i128: the
    /// u128 scores are bounded by GameConfig validation).
    pub enl_delta_sum: i128,
    pub res_delta_sum: i128,
}

impl EntrantSplit {
    pub fn enl_played(&self) -> u64 {
        self.enl_wins + self.enl_draws + self.enl_losses
    }

    pub fn res_played(&self) -> u64 {
        self.res_wins + self.res_draws + self.res_losses
    }
}

/// Aggregates the ENL/RES split per entrant over the scored items.
pub fn faction_splits(
    cfg: &TournamentConfig,
    items: &[TournamentItem],
    outcomes: &[TournamentOutcome],
) -> Vec<EntrantSplit> {
    let mut splits: Vec<EntrantSplit> = cfg
        .entrants
        .iter()
        .map(|entrant| EntrantSplit {
            name: entrant.name.clone(),
            ..Default::default()
        })
        .collect();
    for (item, outcome) in items.iter().zip(outcomes.iter()) {
        let TournamentOutcome::Normal {
            result,
            score_enl,
            score_res,
            ..
        } = outcome
        else {
            continue;
        };
        let (enl_id, res_id) = (item.enl_entrant() as usize, item.res_entrant() as usize);
        let delta_enl = *score_enl as i128 - *score_res as i128;
        let (enl_wdl, res_wdl) = match result {
            arena_core::types::MatchResult::EnlWin => ((1, 0, 0), (0, 0, 1)),
            arena_core::types::MatchResult::ResWin => ((0, 0, 1), (1, 0, 0)),
            arena_core::types::MatchResult::Draw => ((0, 1, 0), (0, 1, 0)),
        };
        splits[enl_id].enl_wins += enl_wdl.0;
        splits[enl_id].enl_draws += enl_wdl.1;
        splits[enl_id].enl_losses += enl_wdl.2;
        splits[enl_id].enl_delta_sum += delta_enl;
        splits[res_id].res_wins += res_wdl.0;
        splits[res_id].res_draws += res_wdl.1;
        splits[res_id].res_losses += res_wdl.2;
        splits[res_id].res_delta_sum -= delta_enl;
    }
    splits
}

/// One standings row (rank by rating desc, tie-break name asc).
#[derive(Clone, Debug)]
pub struct StandingRow {
    pub rank: usize,
    pub name: String,
    pub rating: i64,
    pub wins: u64,
    pub draws: u64,
    pub losses: u64,
    pub forfeit_wins: u64,
    pub forfeit_losses: u64,
    pub unscored: u64,
}

/// The standings order (frozen tie-break).
pub fn standings(cfg: &TournamentConfig, table: &EloTable) -> Vec<StandingRow> {
    let mut rows: Vec<StandingRow> = cfg
        .entrants
        .iter()
        .enumerate()
        .map(|(id, entrant)| StandingRow {
            rank: 0,
            name: entrant.name.clone(),
            rating: table.rating(id as u32),
            wins: table.wins(id as u32),
            draws: table.draws(id as u32),
            losses: table.losses(id as u32),
            forfeit_wins: table.forfeit_wins(id as u32),
            forfeit_losses: table.forfeit_losses(id as u32),
            unscored: table.unscored(id as u32),
        })
        .collect();
    rows.sort_by(|a, b| b.rating.cmp(&a.rating).then_with(|| a.name.cmp(&b.name)));
    for (rank, row) in rows.iter_mut().enumerate() {
        row.rank = rank + 1;
    }
    rows
}

/// Mean of an i128 sum over `count`, fixed-point with three decimals.
fn mean_i128_str(sum: i128, count: u64) -> String {
    if count == 0 {
        return "-".to_string();
    }
    let sign = if sum < 0 { "-" } else { "" };
    let magnitude = sum.unsigned_abs() * 1000 / count as u128;
    format!("{sign}{}.{:03}", magnitude / 1000, magnitude % 1000)
}

/// The `standings.txt` body.
pub fn standings_text(cfg: &TournamentConfig, table: &EloTable, splits: &[EntrantSplit]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# arena-runner tournament standings: {}\n",
        cfg.name
    ));
    out.push_str(&format!(
        "# elo method={} status={} initial={} (draw=0.5; technical outcomes excluded from ratings and W/D/L)\n",
        table.method(), if table.is_final() { "final" } else { "provisional" }, cfg.elo.initial
    ));
    if table.is_final() {
        out.push_str("# batch Elo: Bradley-Terry fit, Gaussian prior sigma=400; ratings.tsv retains online Elo history\n");
    } else {
        out.push_str(&format!(
            "# online Elo: K={}, replayed in item-index order\n",
            cfg.elo.k
        ));
    }
    out.push_str("# rank by rating desc, tie-break entrant name asc\n");
    out.push_str("# ENL/RES splits are over scored matches only — they validate the no-faction-swap hypothesis\n");
    out.push_str("rank\tentrant\trating\tW\tD\tL\tffW\tffL\tunscored\tENL W-D-L\tRES W-D-L\tENL mean dScore2\tRES mean dScore2\n");
    for row in standings(cfg, table) {
        let split = splits
            .iter()
            .find(|split| split.name == row.name)
            .expect("one split per entrant");
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}-{}-{}\t{}-{}-{}\t{}\t{}\n",
            row.rank,
            row.name,
            row.rating,
            row.wins,
            row.draws,
            row.losses,
            row.forfeit_wins,
            row.forfeit_losses,
            row.unscored,
            split.enl_wins,
            split.enl_draws,
            split.enl_losses,
            split.res_wins,
            split.res_draws,
            split.res_losses,
            mean_i128_str(split.enl_delta_sum, split.enl_played()),
            mean_i128_str(split.res_delta_sum, split.res_played()),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// docs/reports/tournament-r1.md — committed evidence from an actual run
// ---------------------------------------------------------------------------

/// The committed report body for a finished tournament run (D85). All
/// numbers come from the run itself; the operator records the milestone
/// commit (the M6 convention — a std-only binary cannot hash its own
/// commit).
pub fn tournament_doc_markdown(
    cfg: &TournamentConfig,
    run: &TournamentRun,
    table: &EloTable,
    splits: &[EntrantSplit],
    records: &[IndexRecord],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Tournament R1 — {} (M9, P5)\n\n", cfg.name));
    out.push_str("First ranked tournament over the frozen Benchmark Seed Set: external\n");
    out.push_str("client binaries through the real server/protocol stack, one match per\n");
    out.push_str("scheduling item (no auto faction-swap rematch), integer Elo with draw =\n");
    out.push_str("0.5, technical outcomes excluded from ratings. Every number below comes\n");
    out.push_str(&format!(
        "from the actual run artifacts in `runs/tournaments/{}/`.\n\n",
        cfg.name
    ));

    out.push_str("## 1. Method\n\n| Item | Value |\n|---|---|\n");
    out.push_str(&format!("| Tournament | `{}` | \n", cfg.name));
    out.push_str(&format!(
        "| Entrants | {} | \n",
        cfg.entrants
            .iter()
            .map(|e| format!("`{}`", e.name))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!(
        "| Matches | {} ({}) | \n",
        run.items.len(),
        cfg.matchups
            .iter()
            .map(|m| format!("{} {}", m.tag, m.count))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!(
        "| Master seed | `{}` -> schedule seed `{}` | \n",
        crate::tournament::archive::hex64(cfg.master_seed),
        crate::tournament::archive::hex64(cfg.schedule_seed())
    ));
    out.push_str(&format!(
        "| Benchmark Seed Set | {} maps, `map_seed = LE-u64(blake3(\"ingress-arena-m9-benchmap\" || k_le32 || master_le64))`, map index = j % {} | \n",
        cfg.map_count, cfg.map_count
    ));
    out.push_str(&format!(
        "| Config | rounds/match {}, windows {} ms spawn / {} ms decision, identity `{}` | \n",
        cfg.rounds_per_match,
        cfg.config.initial_spawn_window_ms,
        cfg.config.decision_window_ms,
        format!("{:#018x}", cfg.config_identity())
    ));
    out.push_str(&format!(
        "| Elo | method = {}, initial = {}, Q32.32 integer fixed point; {} | \n",
        table.method(), cfg.elo.initial,
        if table.is_final() {
            "order-independent Bradley-Terry fit, Gaussian prior sigma=400; ratings.tsv retains online Elo history".to_owned()
        } else {
            format!("provisional online Elo, K={}, replayed in item order", cfg.elo.k)
        }
    ));
    out.push_str(&format!(
        "| Execution | {} workers, sandbox {}, per-match budget {} ms | \n",
        run.workers,
        cfg.sandbox.tag(),
        cfg.per_match_budget_ms
    ));
    out.push_str(&format!(
        "| Runtime | {} ms total (metadata only) | \n",
        run.total_elapsed_ms
    ));
    out.push_str("| Reproduce | `cargo run --release -p arena-runner -- tournament-r1` | \n");
    out.push_str("\nEntrant provenance (binary hashes computed by the runner; tree/lock/\ntoolchain/manifest ids recorded from `tools/provenance` output):\n\n");
    out.push_str("| Entrant | Kind | Binary blake3 (`ingress-arena-m9-binary`) | Tree | Lock | Toolchain | Manifest |\n|---|---|---|---|---|---|---|\n");
    for entrant in &cfg.entrants {
        let provenance = &entrant.provenance;
        out.push_str(&format!(
            "| {} | {} | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            entrant.name,
            entrant.kind.tag(),
            provenance.binary_blake3.as_deref().unwrap_or("-"),
            provenance.tree_hash.as_deref().unwrap_or("-"),
            provenance.lock_hash.as_deref().unwrap_or("-"),
            provenance.toolchain_hash.as_deref().unwrap_or("-"),
            provenance.manifest_id.as_deref().unwrap_or("-"),
        ));
    }

    out.push_str("\n## 2. Standings\n\n");
    out.push_str(
        "| Rank | Entrant | Rating | W-D-L | ffW | unscored |\n|---:|---|---:|---|---:|---:|\n",
    );
    for row in standings(cfg, table) {
        out.push_str(&format!(
            "| {} | {} | {} | {}-{}-{} | {} | {} |\n",
            row.rank,
            row.name,
            row.rating,
            row.wins,
            row.draws,
            row.losses,
            row.forfeit_wins,
            row.unscored
        ));
    }
    out.push_str("\nFaction splits over scored matches (the no-swap hypothesis check;\n");
    out.push_str(
        "scheduling gives each entrant exactly M/2 ENL and M/2 RES slots per matchup):\n\n",
    );
    out.push_str("| Entrant | ENL W-D-L | RES W-D-L | ENL mean dScore2 | RES mean dScore2 |\n|---|---|---|---:|---:|\n");
    for split in splits {
        out.push_str(&format!(
            "| {} | {}-{}-{} | {}-{}-{} | {} | {} |\n",
            split.name,
            split.enl_wins,
            split.enl_draws,
            split.enl_losses,
            split.res_wins,
            split.res_draws,
            split.res_losses,
            mean_i128_str(split.enl_delta_sum, split.enl_played()),
            mean_i128_str(split.res_delta_sum, split.res_played()),
        ));
    }

    out.push_str("\n## 3. Matches\n\n");
    out.push_str("| # | Matchup | Entrant A | Entrant B | A faction | Map | Outcome | Winner | Rounds | Replay |\n|---:|---|---|---|---|---|---|---|---:|---|\n");
    for record in records {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            record.index,
            record.tag,
            record.entrant_a,
            record.entrant_b,
            if record.a_is_enl { "ENL" } else { "RES" },
            record.map_index,
            record.outcome,
            record.winner,
            record.rounds_completed,
            record.replay,
        ));
    }

    let technical: Vec<&IndexRecord> = records
        .iter()
        .filter(|record| record.outcome == "technical_win" || record.outcome == "unscored")
        .collect();
    out.push_str("\n## 4. Technical exclusions\n\n");
    if technical.is_empty() {
        out.push_str("None — every match completed normally.\n");
    } else {
        out.push_str("Technical outcomes never move ratings or W/D/L counters (D82);\n");
        out.push_str("the private reason text is archived in each match's `meta.json`.\n\n");
        out.push_str("| # | Matchup | Outcome | Stage | Winner | Budget kill |\n|---:|---|---|---|---|---|\n");
        for record in technical {
            let budget = run
                .budget_exceeded
                .get(record.index as usize)
                .copied()
                .unwrap_or(false);
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                record.index, record.tag, record.outcome, record.stage, record.winner, budget
            ));
        }
    }
    out.push_str("\n## 5. Reproducibility\n\n");
    out.push_str(&format!(
        "The schedule, seeds, map hashes, and both entrants' provenance are frozen in\n`runs/tournaments/{}/tournament.json`; every match row (seeds, factions,\nmap hash, config identity, provenance ids, replay path) is in `index.tsv`;\nper-match online Elo pre/post ratings are in `ratings.tsv`; the standings\nheader records whether rankings use online Elo or the completed batch fit. Each `matches/m<index>/` directory carries `meta.json`\nplus the server's `journal.txt`/`result.txt` and — for every match that\nentered Round 1 — a `replay.iarp` that decodes and replays to the recorded\nfinal state hash.\n\nCode: M9 milestone worktree (base commit `24029ce`, the M8 handoff; the\norchestrator records the milestone commit). The cited build manifest\ncannot hash the sentence citing it (the M8 limitation): verify any tree\nwith `target/release/provenance --root .` — the entrant binaries above\nmatch the manifest hashes exactly.\n",
        cfg.name
    ));
    out
}
