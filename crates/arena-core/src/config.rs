//! Game configuration (R-CONFIG-01). All values use canonical units:
//! centimetres for lengths, milliseconds for durations, rounds for periods.

/// Upper bound for map dimensions in cm (D10): keeps every authoritative i64
/// intermediate comfortably bounded after `GameConfig::validate`.
pub const MAX_MAP_DIMENSION_CM: i32 = 1 << 20;

/// Upper bound for every distance-like u32 parameter (D10): `(r as i64) * (r as i64)`
/// cannot overflow i64 and `dist2` against it stays exact.
pub const MAX_DISTANCE_PARAM_CM: u32 = i32::MAX as u32;

/// Upper bound for `portal_count` (D10); lower bound is 2.
pub const MAX_PORTAL_COUNT: u32 = 10_000;

/// Configuration validation error. Ordering of variants is not contractual.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    MapDimension,
    PortalCount,
    MinDistance,
    ClusterCount,
    Ratio,
    ZeroParameter(&'static str),
    SupplyBounds,
    HackYield,
    HpBounds,
    DistanceParamTooLarge,
}

/// Complete rule parameter set; `Default` equals the R-CONFIG-01 table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GameConfig {
    // Time
    pub total_rounds: u32,
    pub initial_spawn_window_ms: u32,
    pub decision_window_ms: u32,
    pub priority_bucket_ms: u32,
    pub epoch_length: u32,

    // Map
    pub map_width_cm: i32,
    pub map_height_cm: i32,
    pub portal_count: u32,
    pub portal_min_distance_cm: u32,
    pub clustered_portal_ratio_permille: u32,
    pub portal_cluster_count: u32,

    // Agents
    pub initial_agents_per_faction: u32,
    pub reinforcement_interval_rounds: u32,
    pub reinforcement_agents_per_faction: u32,

    // Movement and ranges
    pub move_budget_cm: u32,
    pub portal_operation_range_cm: u32,
    pub transfer_range_cm: u32,

    // Supply
    pub initial_supply: u32,
    pub max_supply: u32,

    // Portal combat
    pub portal_max_hp: u32,
    pub attack_damage: u32,
    pub recharge_amount: u32,
    pub recharge_cost_supply: u32,

    // Hack
    pub hack_supply_yield: u32,
    pub hack_key_yield_min: u32,
    pub hack_key_yield_max: u32,
    pub hack_cooldown_rounds: u32,
    pub hack_limit_per_epoch: u32,

    // Link
    pub max_link_distance_cm: u32,

    // Protocol guards
    pub max_commands_per_agent_round: u32,
    pub max_frame_payload_bytes: u32,
    /// Cumulative reinforcement limit per faction; zero disables reinforcements.
    /// `u32::MAX` preserves the unlimited legacy rule and binary encoding.
    pub max_reinforcement_agents_per_faction: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            total_rounds: 800,
            initial_spawn_window_ms: 5000,
            decision_window_ms: 1000,
            priority_bucket_ms: 5,
            epoch_length: 100,

            map_width_cm: 600_000,
            map_height_cm: 600_000,
            portal_count: 120,
            portal_min_distance_cm: 8_000,
            clustered_portal_ratio_permille: 700,
            portal_cluster_count: 6,

            initial_agents_per_faction: 3,
            reinforcement_interval_rounds: 100,
            reinforcement_agents_per_faction: 1,

            move_budget_cm: 20_000,
            portal_operation_range_cm: 5_000,
            transfer_range_cm: 5_000,

            initial_supply: 20,
            max_supply: 100,

            portal_max_hp: 100,
            attack_damage: 20,
            recharge_amount: 20,
            recharge_cost_supply: 1,

            hack_supply_yield: 1,
            hack_key_yield_min: 1,
            hack_key_yield_max: 3,
            hack_cooldown_rounds: 3,
            hack_limit_per_epoch: 5,

            max_link_distance_cm: 800_000,

            max_reinforcement_agents_per_faction: 4,
            max_commands_per_agent_round: 10,
            max_frame_payload_bytes: 65_536,
        }
    }
}

impl GameConfig {
    /// Validates that this configuration is usable: dimensions and counts are
    /// in range, no required parameter is zero, and every distance-like value
    /// is small enough that authoritative checked arithmetic cannot overflow.
    /// Must be called before a match uses the config; ADR-0007 treats any
    /// post-validation overflow as a server bug.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Time parameters must be positive.
        for (name, value) in [
            ("total_rounds", self.total_rounds),
            ("initial_spawn_window_ms", self.initial_spawn_window_ms),
            ("decision_window_ms", self.decision_window_ms),
            ("priority_bucket_ms", self.priority_bucket_ms),
            ("epoch_length", self.epoch_length),
            (
                "initial_agents_per_faction",
                self.initial_agents_per_faction,
            ),
            (
                "reinforcement_interval_rounds",
                self.reinforcement_interval_rounds,
            ),
            (
                "reinforcement_agents_per_faction",
                self.reinforcement_agents_per_faction,
            ),
            ("hack_cooldown_rounds", self.hack_cooldown_rounds),
            ("hack_limit_per_epoch", self.hack_limit_per_epoch),
            (
                "max_commands_per_agent_round",
                self.max_commands_per_agent_round,
            ),
            ("max_frame_payload_bytes", self.max_frame_payload_bytes),
            ("recharge_cost_supply", self.recharge_cost_supply),
            ("hack_supply_yield", self.hack_supply_yield),
        ] {
            if value == 0 {
                return Err(ConfigError::ZeroParameter(name));
            }
        }

        // Map dimensions: 1..=MAX cm per axis (D10).
        if self.map_width_cm < 1
            || self.map_width_cm > MAX_MAP_DIMENSION_CM
            || self.map_height_cm < 1
            || self.map_height_cm > MAX_MAP_DIMENSION_CM
        {
            return Err(ConfigError::MapDimension);
        }

        // Portal population: at least 2, at most MAX_PORTAL_COUNT (D10).
        if self.portal_count < 2 || self.portal_count > MAX_PORTAL_COUNT {
            return Err(ConfigError::PortalCount);
        }

        // Minimum spacing: >= 1 cm guarantees distinct portal coordinates,
        // bounded so the generator's spread arithmetic stays exact (D10).
        if self.portal_min_distance_cm == 0 || self.portal_min_distance_cm > MAX_DISTANCE_PARAM_CM {
            return Err(ConfigError::MinDistance);
        }

        // Cluster ratio is stored permille: 0..=1000 (D4).
        if self.clustered_portal_ratio_permille > 1000 {
            return Err(ConfigError::Ratio);
        }

        // Cluster count: 1..=portal_count (D5).
        if self.portal_cluster_count == 0 || self.portal_cluster_count > self.portal_count {
            return Err(ConfigError::ClusterCount);
        }

        // Supply bounds.
        if self.max_supply == 0 || self.initial_supply > self.max_supply {
            return Err(ConfigError::SupplyBounds);
        }

        // Portal HP and damage.
        if self.portal_max_hp == 0 || self.attack_damage == 0 {
            return Err(ConfigError::HpBounds);
        }

        // Key yield interval must be a sane DiscreteUniform range (D3).
        if self.hack_key_yield_min == 0 || self.hack_key_yield_min > self.hack_key_yield_max {
            return Err(ConfigError::HackYield);
        }

        // Distance-like parameters: bounded so (r as i64)^2 cannot overflow
        // and comparisons against dist2 stay exact (D10).
        for value in [
            self.move_budget_cm,
            self.portal_operation_range_cm,
            self.transfer_range_cm,
            self.max_link_distance_cm,
        ] {
            if value > MAX_DISTANCE_PARAM_CM {
                return Err(ConfigError::DistanceParamTooLarge);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_r_config_01() {
        let cfg = GameConfig::default();
        assert_eq!(cfg.total_rounds, 800);
        assert_eq!(cfg.initial_spawn_window_ms, 5000);
        assert_eq!(cfg.decision_window_ms, 1000);
        assert_eq!(cfg.priority_bucket_ms, 5);
        assert_eq!(cfg.epoch_length, 100);
        assert_eq!(cfg.map_width_cm, 600_000);
        assert_eq!(cfg.map_height_cm, 600_000);
        assert_eq!(cfg.portal_count, 120);
        assert_eq!(cfg.portal_min_distance_cm, 8_000);
        assert_eq!(cfg.clustered_portal_ratio_permille, 700);
        assert_eq!(cfg.portal_cluster_count, 6);
        assert_eq!(cfg.initial_agents_per_faction, 3);
        assert_eq!(cfg.reinforcement_interval_rounds, 100);
        assert_eq!(cfg.reinforcement_agents_per_faction, 1);
        assert_eq!(cfg.max_reinforcement_agents_per_faction, 4);
        assert_eq!(cfg.move_budget_cm, 20_000);
        assert_eq!(cfg.portal_operation_range_cm, 5_000);
        assert_eq!(cfg.transfer_range_cm, 5_000);
        assert_eq!(cfg.initial_supply, 20);
        assert_eq!(cfg.max_supply, 100);
        assert_eq!(cfg.portal_max_hp, 100);
        assert_eq!(cfg.attack_damage, 20);
        assert_eq!(cfg.recharge_amount, 20);
        assert_eq!(cfg.recharge_cost_supply, 1);
        assert_eq!(cfg.hack_supply_yield, 1);
        assert_eq!(cfg.hack_key_yield_min, 1);
        assert_eq!(cfg.hack_key_yield_max, 3);
        assert_eq!(cfg.hack_cooldown_rounds, 3);
        assert_eq!(cfg.hack_limit_per_epoch, 5);
        assert_eq!(cfg.max_link_distance_cm, 800_000);
        assert_eq!(cfg.max_commands_per_agent_round, 10);
        assert_eq!(cfg.max_frame_payload_bytes, 65_536);
    }

    #[test]
    fn default_validates() {
        assert_eq!(GameConfig::default().validate(), Ok(()));
    }

    #[test]
    fn rejects_map_dimension_over_cap() {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = MAX_MAP_DIMENSION_CM + 1;
        assert_eq!(cfg.validate(), Err(ConfigError::MapDimension));
        cfg = GameConfig::default();
        cfg.map_height_cm = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::MapDimension));
        cfg = GameConfig::default();
        cfg.map_width_cm = -1;
        assert_eq!(cfg.validate(), Err(ConfigError::MapDimension));
        // The cap itself is accepted.
        cfg = GameConfig::default();
        cfg.map_height_cm = MAX_MAP_DIMENSION_CM;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_zero_required_parameters() {
        let cases: [(&'static str, fn(&mut GameConfig)); 13] = [
            ("total_rounds", |c| c.total_rounds = 0),
            ("initial_spawn_window_ms", |c| c.initial_spawn_window_ms = 0),
            ("decision_window_ms", |c| c.decision_window_ms = 0),
            ("priority_bucket_ms", |c| c.priority_bucket_ms = 0),
            ("epoch_length", |c| c.epoch_length = 0),
            ("initial_agents_per_faction", |c| {
                c.initial_agents_per_faction = 0
            }),
            ("reinforcement_interval_rounds", |c| {
                c.reinforcement_interval_rounds = 0
            }),
            ("reinforcement_agents_per_faction", |c| {
                c.reinforcement_agents_per_faction = 0
            }),
            ("hack_cooldown_rounds", |c| c.hack_cooldown_rounds = 0),
            ("hack_limit_per_epoch", |c| c.hack_limit_per_epoch = 0),
            ("recharge_cost_supply", |c| c.recharge_cost_supply = 0),
            ("hack_supply_yield", |c| c.hack_supply_yield = 0),
            ("max_commands_per_agent_round", |c| {
                c.max_commands_per_agent_round = 0
            }),
        ];
        for (name, setter) in cases {
            let mut cfg = GameConfig::default();
            setter(&mut cfg);
            assert_eq!(
                cfg.validate(),
                Err(ConfigError::ZeroParameter(name)),
                "zeroing {name} must be rejected"
            );
        }
        let mut cfg = GameConfig::default();
        cfg.max_frame_payload_bytes = 0;
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::ZeroParameter("max_frame_payload_bytes"))
        );
    }

    #[test]
    fn rejects_initial_supply_above_max_supply() {
        let mut cfg = GameConfig::default();
        cfg.initial_supply = 101;
        assert_eq!(cfg.validate(), Err(ConfigError::SupplyBounds));
        // max_supply == 0 is also rejected even when initial fits.
        let mut cfg = GameConfig::default();
        cfg.max_supply = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::SupplyBounds));
        // initial == max is the exact boundary and passes.
        let mut cfg = GameConfig::default();
        cfg.initial_supply = cfg.max_supply;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_portal_count_out_of_range() {
        let mut cfg = GameConfig::default();
        cfg.portal_count = 1;
        assert_eq!(cfg.validate(), Err(ConfigError::PortalCount));
        cfg = GameConfig::default();
        cfg.portal_count = MAX_PORTAL_COUNT + 1;
        assert_eq!(cfg.validate(), Err(ConfigError::PortalCount));
        // Boundaries pass (cluster count must fit under the reduced count).
        cfg = GameConfig::default();
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        assert_eq!(cfg.validate(), Ok(()));
        cfg = GameConfig::default();
        cfg.portal_count = MAX_PORTAL_COUNT;
        cfg.portal_cluster_count = MAX_PORTAL_COUNT;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_zero_portal_min_distance() {
        let mut cfg = GameConfig::default();
        cfg.portal_min_distance_cm = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::MinDistance));
        // Oversized min distance is also a MinDistance failure.
        cfg.portal_min_distance_cm = u32::MAX;
        assert_eq!(cfg.validate(), Err(ConfigError::MinDistance));
    }

    #[test]
    fn rejects_ratio_permille_above_1000() {
        let mut cfg = GameConfig::default();
        cfg.clustered_portal_ratio_permille = 1001;
        assert_eq!(cfg.validate(), Err(ConfigError::Ratio));
        // Extremes 0 and 1000 pass.
        cfg = GameConfig::default();
        cfg.clustered_portal_ratio_permille = 0;
        assert_eq!(cfg.validate(), Ok(()));
        cfg.clustered_portal_ratio_permille = 1000;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_cluster_count_zero_or_above_portal_count() {
        let mut cfg = GameConfig::default();
        cfg.portal_cluster_count = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::ClusterCount));
        cfg = GameConfig::default();
        cfg.portal_cluster_count = cfg.portal_count + 1;
        assert_eq!(cfg.validate(), Err(ConfigError::ClusterCount));
        // cluster_count == portal_count is the exact boundary and passes.
        cfg = GameConfig::default();
        cfg.portal_cluster_count = cfg.portal_count;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_hack_yield_range() {
        let mut cfg = GameConfig::default();
        cfg.hack_key_yield_min = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::HackYield));
        cfg = GameConfig::default();
        cfg.hack_key_yield_min = 4;
        cfg.hack_key_yield_max = 3;
        assert_eq!(cfg.validate(), Err(ConfigError::HackYield));
        // min == max is a degenerate but valid DiscreteUniform.
        cfg = GameConfig::default();
        cfg.hack_key_yield_min = 2;
        cfg.hack_key_yield_max = 2;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_distance_param_square_overflow() {
        for setter in [
            |c: &mut GameConfig| c.move_budget_cm = u32::MAX,
            |c: &mut GameConfig| c.portal_operation_range_cm = u32::MAX,
            |c: &mut GameConfig| c.transfer_range_cm = u32::MAX,
            |c: &mut GameConfig| c.max_link_distance_cm = u32::MAX,
        ] {
            let mut cfg = GameConfig::default();
            setter(&mut cfg);
            assert_eq!(
                cfg.validate(),
                Err(ConfigError::DistanceParamTooLarge),
                "oversized distance parameter must be rejected"
            );
        }
        // The exact cap passes for every distance-like field.
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = MAX_DISTANCE_PARAM_CM;
        cfg.portal_operation_range_cm = MAX_DISTANCE_PARAM_CM;
        cfg.transfer_range_cm = MAX_DISTANCE_PARAM_CM;
        cfg.max_link_distance_cm = MAX_DISTANCE_PARAM_CM;
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_zero_hp_or_damage() {
        let mut cfg = GameConfig::default();
        cfg.portal_max_hp = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::HpBounds));
        let mut cfg = GameConfig::default();
        cfg.attack_damage = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::HpBounds));
    }
}
