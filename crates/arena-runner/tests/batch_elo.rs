use arena_core::types::MatchResult;
use arena_runner::tournament::{
    EloConfig, Stage, TournamentItem, TournamentOutcome, elo::EloTable,
};

fn game(index: u64, a: u32, b: u32, winner: Option<u32>) -> (TournamentItem, TournamentOutcome) {
    let a_is_enl = index % 2 == 0;
    let result = match winner {
        None => MatchResult::Draw,
        Some(id) if (id == a) == a_is_enl => MatchResult::EnlWin,
        Some(_) => MatchResult::ResWin,
    };
    (
        TournamentItem {
            index,
            tag: format!("{a}-{b}"),
            j: index as u32,
            a,
            b,
            a_is_enl,
            map_index: 0,
            map_seed: 0,
            map_hash: String::new(),
            game_seed: 0,
            client_seed: 0,
            schedule_seed: 0,
            config_hash: 0,
        },
        TournamentOutcome::Normal {
            result,
            score_enl: u128::from(result == MatchResult::EnlWin),
            score_res: u128::from(result == MatchResult::ResWin),
            rounds_completed: 1,
            final_state_hash: [0; 32],
        },
    )
}

fn standings(
    entrants: usize,
    games: &[(TournamentItem, TournamentOutcome)],
    total: usize,
) -> EloTable {
    EloTable::for_standings(
        &EloConfig::default(),
        entrants,
        total,
        games.iter().map(|(i, o)| (i, o)),
    )
    .unwrap()
}

#[test]
fn completed_round_robin_is_order_independent_and_glm_outscores_adaptive() {
    // The 15 matchup totals from all_bots_260916, in registered entrant order:
    // kimi, adaptive, astra, astra_v2, deepseek, glm.
    let wins = [47, 3, 0, 0, 34, 0, 0, 3, 20, 4, 37, 48, 50, 50, 49];
    let mut games = Vec::new();
    let mut pair = 0;
    for a in 0..6 {
        for b in a + 1..6 {
            for j in 0..50 {
                games.push(game(
                    games.len() as u64,
                    a,
                    b,
                    Some(if j < wins[pair] { a } else { b }),
                ));
            }
            pair += 1;
        }
    }
    let table = standings(6, &games, 750);
    assert!(table.is_final());
    // Independent floating-point gradient ascent of the penalized likelihood.
    assert_eq!(table.ratings(), &[1242, 961, 1804, 2268, 1649, 1076]);
    assert!(table.rating(5) > table.rating(1), "{:?}", table.ratings());
    assert_eq!((table.wins(5), table.losses(5)), (49, 201));
    assert_eq!((table.wins(1), table.losses(1)), (26, 224));
    games.reverse();
    assert_eq!(standings(6, &games, 750).ratings(), table.ratings());
    games.sort_by_key(|(item, _)| (item.j % 50, item.a, item.b));
    assert_eq!(standings(6, &games, 750).ratings(), table.ratings());
    // Entrant renumbering also preserves the rounded optimum.
    for (item, _) in &mut games {
        item.a = 5 - item.a;
        item.b = 5 - item.b;
    }
    let reversed = standings(6, &games, 750);
    assert_eq!(
        reversed.ratings().iter().rev().copied().collect::<Vec<_>>(),
        table.ratings()
    );
}

#[test]
fn batch_prior_bounds_undefeated_disconnected_and_unplayed_entrants() {
    let games: Vec<_> = (0..100)
        .map(|i| game(i, 0, 1, Some(0)))
        .chain((100..120).map(|i| game(i, 2, 3, None)))
        .collect();
    let table = standings(5, &games, games.len());
    assert!(table.rating(0) > 1500 && table.rating(0) < 2300);
    assert_eq!(table.rating(0) + table.rating(1), 3000);
    assert_eq!(&table.ratings()[2..], &[1500, 1500, 1500]);
    // Independently check the penalized likelihood's stationarity using f64.
    let offset = (table.rating(0) - 1500) as f64;
    let p = 1.0 / (1.0 + 10f64.powf(-2.0 * offset / 400.0));
    let residual = 100.0 * (1.0 - p) - offset / (400.0 * 10f64.ln());
    assert!(residual.abs() < 0.01, "residual={residual}");
}

#[test]
fn completion_requires_all_terminal_results_and_keeps_online_history() {
    let mut games = vec![game(0, 0, 1, Some(0))];
    let partial = standings(2, &games, 2);
    assert!(!partial.is_final());
    assert_eq!(partial.ratings(), &[1516, 1484]);
    games.push(game(1, 0, 1, None));
    for stage in [Stage::RunnerFault, Stage::SetupError] {
        games[1].1 = TournamentOutcome::Unscored {
            stage,
            reason: "pending or fault".into(),
        };
        let pending = standings(2, &games, 2);
        assert!(!pending.is_final());
        assert_eq!(pending.ratings(), partial.ratings());
    }
    games[1].1 = TournamentOutcome::TechnicalWin {
        winner: 1,
        stage: Stage::ClientForfeit,
        reason: "timeout".into(),
    };
    let final_table = standings(2, &games, 2);
    assert!(final_table.is_final());
    assert_eq!(final_table.forfeit_wins(1), 1);
    assert_eq!(final_table.wins(0), 1);
    assert_eq!(final_table.losses(1), 1);
    assert_eq!(
        final_table.ratings(),
        standings(2, &games[..1], 1).ratings()
    );
    assert!(!standings(2, &[], 0).is_final());
}

#[test]
fn batch_ignores_online_k_and_translates_with_initial_rating() {
    let games = vec![game(0, 0, 1, Some(0))];
    let baseline = standings(2, &games, 1);
    let shifted = EloTable::for_standings(
        &EloConfig {
            k: 1,
            initial: 2000,
        },
        2,
        1,
        games.iter().map(|(i, o)| (i, o)),
    )
    .unwrap();
    assert_eq!(
        shifted.ratings(),
        baseline
            .ratings()
            .iter()
            .map(|r| r + 500)
            .collect::<Vec<_>>()
    );
}
