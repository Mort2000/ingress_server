#![cfg(feature = "offline")]
use arena_client_sdk::{
    offline,
    strategy::{Decision, Strategy},
};
use arena_core::{observe::Observation, types::Faction};
struct Counter(u32);
impl Strategy for Counter {
    fn decide(
        &mut self,
        obs: &Observation,
        turn: &mut dyn Decision,
    ) -> Result<(), arena_client_sdk::SdkError> {
        self.0 += 1;
        assert_eq!(self.0, obs.round, "history must start at opening");
        for a in &obs.own_agents {
            turn.submit(a.id, vec![])?;
        }
        Ok(())
    }
}
#[test]
fn budget_document_requires_explicit_round_inputs() {
    let inputs = offline::Budgets::parse("arena-budgets-v1\n1\t20,10\n2\t\n").unwrap();
    assert_eq!(inputs.for_round(1).unwrap(), vec![20, 10]);
    assert_eq!(inputs.for_round(2).unwrap(), Vec::<u64>::new());
    assert!(inputs.for_round(3).is_err());
    assert!(offline::Budgets::parse("arena-budgets-v1\n1\t10\n1\t20\n").is_err());
    let _ = (Counter(0), Faction::Enl);
}

#[test]
fn stateful_history_starts_at_opening_and_reports_first_command_difference() {
    use arena_core::{
        config::GameConfig, resolve::Resolver, spawn::validate_initial_spawn_set, state::GameState,
        types::Coord,
    };
    let cfg = GameConfig {
        total_rounds: 2,
        ..GameConfig::default()
    };
    let map = arena_core::map::generate_map(9, &cfg).unwrap();
    let (a, b) = GameState::initial_roster(&cfg);
    let spawn = |ids: Vec<_>| {
        validate_initial_spawn_set(
            &ids,
            &ids.iter()
                .map(|id| (*id, Coord::new(0, 0)))
                .collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap()
    };
    let a = spawn(a);
    let b = spawn(b);
    let mut state = GameState::new(cfg, &map, &a, &b).unwrap();
    let mut writer = arena_replay::recorder::ReplayWriter::new(71, &cfg, &map, &a, &b);
    for _ in 0..2 {
        let report = Resolver::with_seed(71).run_round(&mut state, &[]);
        state.advance_round();
        writer.record_round(&report, &state).unwrap();
    }
    let log = arena_replay::format::decode(&writer.finish(&state).unwrap()).unwrap();
    let budgets = offline::Budgets::parse("arena-budgets-v1\n1\t\n2\t\n").unwrap();
    let result = offline::reconstruct(&log, Faction::Enl, &budgets, &mut Counter(0)).unwrap();
    assert_eq!(result.first_difference, Some(1));
    assert_eq!(result.rounds.len(), 2);
    assert_eq!(result.rounds[1].observation.own_agents.len(), 3);
    assert_eq!(result.rounds[1].submissions.len(), 3);
    assert_eq!(result.rounds[1].recorded.len(), 0);
}
