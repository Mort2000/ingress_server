use arena_client_sdk::strategy::{Decision, OfflineRound, Strategy};
use arena_core::{
    config::GameConfig,
    observe::Observation,
    types::{AgentId, Command, Coord, Faction},
};

struct Stateful(u32);
impl Strategy for Stateful {
    fn decide(
        &mut self,
        obs: &Observation,
        turn: &mut dyn Decision,
    ) -> Result<(), arena_client_sdk::SdkError> {
        self.0 += 1;
        turn.submit(
            obs.own_agents[0].id,
            vec![Command::Move {
                destination: Coord::new(self.0 as i32, 0),
            }],
        )?;
        // Submission remains incremental: the next budget observation is
        // made after the first write, not before a returned batch flush.
        if turn.remaining_ms()? >= 10 {
            turn.submit(obs.own_agents[1].id, vec![])?;
        }
        turn.commit()
    }
}

#[test]
fn injected_budgets_preserve_stateful_early_submission_and_commit() {
    let cfg = GameConfig::default();
    let map = arena_core::map::generate_map(9, &cfg).unwrap();
    let (a, b) = arena_core::state::GameState::initial_roster(&cfg);
    let spawns = |ids: &[AgentId]| {
        arena_core::spawn::validate_initial_spawn_set(
            ids,
            &ids.iter()
                .map(|id| (*id, Coord::new(0, 0)))
                .collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap()
    };
    let state = arena_core::state::GameState::new(cfg, &map, &spawns(&a), &spawns(&b)).unwrap();
    let obs = Observation::for_faction(&state, Faction::Enl);
    let mut bot = Stateful(0);
    let mut first = OfflineRound::new(&obs, vec![5], 0);
    bot.decide(&obs, &mut first).unwrap();
    assert_eq!(first.submissions().len(), 1);
    assert_eq!(
        first.submissions()[0].commands,
        vec![Command::Move {
            destination: Coord::new(1, 0)
        }]
    );
    assert!(first.is_committed());
    assert!(first.submit(a[1], vec![]).is_err());
    let mut second = OfflineRound::new(&obs, vec![20], 0);
    bot.decide(&obs, &mut second).unwrap();
    assert_eq!(second.submissions().len(), 2);
    assert_eq!(
        second.submissions()[0].commands,
        vec![Command::Move {
            destination: Coord::new(2, 0)
        }]
    );
    assert_eq!(second.budget_reads(), &[20]);
    assert!(
        OfflineRound::new(&obs, vec![], 0).remaining_ms().is_err(),
        "missing clock input must not silently become real time"
    );
}
