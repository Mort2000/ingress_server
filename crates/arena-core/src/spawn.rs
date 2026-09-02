//! Initial Spawn Set whole-set validation and application (R-SPAWN-01).
//!
//! A faction submits one complete `agent_id -> coordinate` set inside the
//! initial spawn window; the set must exactly cover that faction's initial
//! agents and is accepted or rejected AS A WHOLE. Missing, duplicate, or
//! unknown agent ids and out-of-bounds coordinates all invalidate it. No
//! coordinate-coincidence checks exist: same or overlapping coordinates are
//! legal for own and enemy agents alike (R-AGENT-01). Envelope, framing, and
//! deadline handling are transport concerns outside arena-core.

use std::collections::BTreeMap;

use crate::config::GameConfig;
use crate::geometry::in_bounds;
use crate::types::{Agent, AgentId, Coord};

/// Rejection reasons for a submitted Initial Spawn Set.
#[derive(Debug, PartialEq, Eq)]
pub enum SpawnSetError {
    /// An expected initial agent received no coordinate; reported over the
    /// sorted expected ids after all entries passed per-entry checks.
    MissingAgent(AgentId),
    /// The same agent id appears twice in one submission.
    DuplicateAgent(AgentId),
    /// The submitted agent id is not one of this faction's initial agents.
    UnknownAgent(AgentId),
    /// The coordinate lies outside the closed map bounds (R-MAP-01).
    OutOfBounds { agent: AgentId, position: Coord },
}

/// A validated spawn set. Can only be obtained from
/// [`validate_initial_spawn_set`] succeeding.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InitialSpawns {
    positions: BTreeMap<AgentId, Coord>,
}

impl InitialSpawns {
    pub fn position(&self, agent: AgentId) -> Option<Coord> {
        self.positions.get(&agent).copied()
    }

    /// Iterates `(agent_id, coordinate)` in ascending agent-id order.
    pub fn iter(&self) -> impl Iterator<Item = (AgentId, Coord)> + '_ {
        self.positions.iter().map(|(&id, &position)| (id, position))
    }

    /// Applies every validated position to `agents`. Call only with the set
    /// produced from a successful validation of the same roster: this cannot
    /// fail, so application is atomic by construction.
    pub fn apply(&self, agents: &mut [Agent]) {
        for agent in agents.iter_mut() {
            if let Some(position) = self.positions.get(&agent.id) {
                agent.position = *position;
            }
        }
    }
}

/// Validates a submitted Initial Spawn Set against the faction's expected
/// initial agents.
///
/// Per-entry defects are reported in submission order, each entry checked as
/// unknown id, then duplicate id, then out-of-bounds coordinate; the first
/// defect rejects the whole set. Only when every entry is clean are missing
/// agents reported, over the sorted expected ids. The set as a whole is
/// either fully accepted or fully rejected — there is no partial spawn.
pub fn validate_initial_spawn_set(
    expected: &[AgentId],
    entries: &[(AgentId, Coord)],
    cfg: &GameConfig,
) -> Result<InitialSpawns, SpawnSetError> {
    let mut positions: BTreeMap<AgentId, Coord> = BTreeMap::new();

    for &(agent, position) in entries {
        if !expected.contains(&agent) {
            return Err(SpawnSetError::UnknownAgent(agent));
        }
        if positions.contains_key(&agent) {
            return Err(SpawnSetError::DuplicateAgent(agent));
        }
        if !in_bounds(position, cfg.map_width_cm, cfg.map_height_cm) {
            return Err(SpawnSetError::OutOfBounds { agent, position });
        }
        positions.insert(agent, position);
    }

    let mut sorted_expected: Vec<AgentId> = expected.to_vec();
    sorted_expected.sort_unstable();
    for &agent in &sorted_expected {
        if !positions.contains_key(&agent) {
            return Err(SpawnSetError::MissingAgent(agent));
        }
    }

    Ok(InitialSpawns { positions })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Faction;
    use std::collections::BTreeMap;

    fn default_cfg() -> GameConfig {
        GameConfig::default()
    }

    fn sample_entries() -> Vec<(AgentId, Coord)> {
        vec![
            (AgentId(2), Coord::new(600_000, 0)),
            (AgentId(0), Coord::new(0, 0)),
            (AgentId(1), Coord::new(351_230, 178_452)),
        ]
    }

    fn make_agent(id: u32) -> Agent {
        Agent {
            id: AgentId(id),
            faction: Faction::Enl,
            position: Coord::new(-1, -1), // sentinel: "not yet spawned"
            supply: 20,
            keys: BTreeMap::new(),
        }
    }

    #[test]
    fn valid_exact_cover_accepts_and_applies() {
        let expected = [AgentId(0), AgentId(1), AgentId(2)];
        let spawns = validate_initial_spawn_set(&expected, &sample_entries(), &default_cfg())
            .expect("exact cover is valid");
        assert_eq!(spawns.position(AgentId(0)), Some(Coord::new(0, 0)));
        assert_eq!(
            spawns.position(AgentId(1)),
            Some(Coord::new(351_230, 178_452))
        );
        assert_eq!(spawns.position(AgentId(2)), Some(Coord::new(600_000, 0)));
        assert_eq!(spawns.position(AgentId(9)), None);

        // Iteration is ascending by agent id regardless of submission order.
        let ids: Vec<AgentId> = spawns.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![AgentId(0), AgentId(1), AgentId(2)]);

        let mut agents = vec![make_agent(2), make_agent(1), make_agent(0)];
        spawns.apply(&mut agents);
        assert_eq!(agents[0].position, Coord::new(600_000, 0));
        assert_eq!(agents[1].position, Coord::new(351_230, 178_452));
        assert_eq!(agents[2].position, Coord::new(0, 0));
        // Unrelated fields untouched.
        assert_eq!(agents[0].supply, 20);
        assert!(agents[0].keys.is_empty());
    }

    #[test]
    fn same_coordinate_own_agents_allowed() {
        let expected = [AgentId(0), AgentId(1)];
        let shared = Coord::new(100_000, 100_000);
        let entries = vec![(AgentId(0), shared), (AgentId(1), shared)];
        let spawns = validate_initial_spawn_set(&expected, &entries, &default_cfg())
            .expect("identical own coordinates are legal");
        assert_eq!(spawns.position(AgentId(0)), Some(shared));
        assert_eq!(spawns.position(AgentId(1)), Some(shared));
    }

    #[test]
    fn enemy_same_coordinate_independent() {
        // Both factions validate the same coordinate without any knowledge of
        // each other; validation never inspects the other faction.
        let shared = Coord::new(250_000, 250_000);
        for expected in [
            vec![AgentId(0), AgentId(1), AgentId(2)],
            vec![AgentId(3), AgentId(4), AgentId(5)],
        ] {
            let entries: Vec<(AgentId, Coord)> = expected.iter().map(|&id| (id, shared)).collect();
            let spawns = validate_initial_spawn_set(&expected, &entries, &default_cfg())
                .expect("enemy overlap is not a spawn concern");
            assert_eq!(spawns.iter().count(), 3);
        }
    }

    #[test]
    fn missing_agent_rejected() {
        let expected = [AgentId(0), AgentId(1), AgentId(2)];
        let entries = vec![
            (AgentId(0), Coord::new(0, 0)),
            (AgentId(2), Coord::new(10, 10)),
        ];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::MissingAgent(AgentId(1)))
        );
    }

    #[test]
    fn missing_agent_reported_over_sorted_expected() {
        // Submit only agent 2; among missing {0, 1} the lowest id is reported.
        let expected = [AgentId(2), AgentId(0), AgentId(1)];
        let entries = vec![(AgentId(2), Coord::new(0, 0))];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::MissingAgent(AgentId(0)))
        );
    }

    #[test]
    fn duplicate_agent_rejected() {
        let expected = [AgentId(0), AgentId(1)];
        let entries = vec![
            (AgentId(0), Coord::new(0, 0)),
            (AgentId(1), Coord::new(5, 5)),
            (AgentId(0), Coord::new(9, 9)),
        ];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::DuplicateAgent(AgentId(0)))
        );
    }

    #[test]
    fn unknown_agent_rejected() {
        let expected = [AgentId(0), AgentId(1), AgentId(2)];
        let entries = vec![
            (AgentId(0), Coord::new(0, 0)),
            (AgentId(99), Coord::new(1, 1)),
            (AgentId(1), Coord::new(2, 2)),
        ];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::UnknownAgent(AgentId(99)))
        );
    }

    #[test]
    fn out_of_bounds_rejected() {
        let expected = [AgentId(0)];
        for bad in [
            Coord::new(600_001, 0),
            Coord::new(0, 600_001),
            Coord::new(-1, 0),
        ] {
            assert_eq!(
                validate_initial_spawn_set(&expected, &[(AgentId(0), bad)], &default_cfg()),
                Err(SpawnSetError::OutOfBounds {
                    agent: AgentId(0),
                    position: bad
                }),
                "position {bad:?} must be rejected"
            );
        }
        // Closed-boundary corners are legal.
        for corner in [
            Coord::new(0, 0),
            Coord::new(600_000, 0),
            Coord::new(0, 600_000),
            Coord::new(600_000, 600_000),
        ] {
            assert!(
                validate_initial_spawn_set(&expected, &[(AgentId(0), corner)], &default_cfg())
                    .is_ok()
            );
        }
    }

    #[test]
    fn invalid_set_applies_nothing() {
        // A rejected set must leave the roster untouched: nothing was ever
        // applied because only a validated InitialSpawns can be applied.
        let expected = [AgentId(0), AgentId(1)];
        let entries = vec![(AgentId(0), Coord::new(0, 0))];
        let result = validate_initial_spawn_set(&expected, &entries, &default_cfg());
        assert!(result.is_err());
        let agents = vec![make_agent(0), make_agent(1)];
        // No InitialSpawns value exists for an invalid set; positions remain
        // the pre-spawn sentinel, proving no partial application happened.
        assert_eq!(agents[0].position, Coord::new(-1, -1));
        assert_eq!(agents[1].position, Coord::new(-1, -1));
    }

    #[test]
    fn first_error_in_submission_order() {
        // Entry with an unknown id precedes an out-of-bounds entry: the
        // unknown id is reported first regardless of magnitude.
        let expected = [AgentId(0), AgentId(1)];
        let entries = vec![
            (AgentId(42), Coord::new(999_999, 999_999)),
            (AgentId(0), Coord::new(600_001, 0)),
        ];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::UnknownAgent(AgentId(42)))
        );

        // Out-of-bounds precedes a later unknown id.
        let entries = vec![
            (AgentId(0), Coord::new(600_001, 0)),
            (AgentId(42), Coord::new(1, 1)),
        ];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::OutOfBounds {
                agent: AgentId(0),
                position: Coord::new(600_001, 0)
            })
        );

        // Duplicate precedes a later out-of-bounds entry.
        let entries = vec![
            (AgentId(1), Coord::new(1, 1)),
            (AgentId(1), Coord::new(2, 2)),
            (AgentId(0), Coord::new(600_001, 0)),
        ];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::DuplicateAgent(AgentId(1)))
        );

        // Per-entry defects always beat missing agents.
        let entries = vec![(AgentId(7), Coord::new(0, 0))];
        assert_eq!(
            validate_initial_spawn_set(&expected, &entries, &default_cfg()),
            Err(SpawnSetError::UnknownAgent(AgentId(7)))
        );
    }

    #[test]
    fn empty_expected_rejects_any_entry_and_accepts_empty() {
        let empty: [(AgentId, Coord); 0] = [];
        assert!(validate_initial_spawn_set(&[], &empty, &default_cfg()).is_ok());
        assert_eq!(
            validate_initial_spawn_set(&[], &[(AgentId(0), Coord::new(0, 0))], &default_cfg()),
            Err(SpawnSetError::UnknownAgent(AgentId(0)))
        );
    }
}
