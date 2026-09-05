//! Same-round combat/auxiliary composition. Preserve the chosen work's
//! route and resource prerequisites; never assume an unknown future Hack Key.

use crate::{
    common,
    r2::{build::BuilderCore, tasks::Task},
};
use arena_core::{
    observe::{AgentPrivate, Observation},
    types::Command,
};

pub(super) fn is_combat(task: Task) -> bool {
    matches!(task, Task::Attack { .. } | Task::Recharge { .. })
}

pub(super) fn pair(
    core: &BuilderCore,
    obs: &Observation,
    me: &AgentPrivate,
    first: Task,
    second: Task,
) -> Option<Vec<Command>> {
    let (combat, work) = match (is_combat(first), is_combat(second)) {
        (true, false) => (first, second),
        (false, true) => (second, first),
        _ => return None,
    };
    // Transfer belongs to a separate stateful planner and is not altered here.
    if matches!(work, Task::Transfer { .. }) {
        return None;
    }
    let combat_commands = core.commands_for(obs, me, combat).build();
    let work_commands = core.commands_for(obs, me, work).build();
    if combat_commands.is_empty()
        || work_commands.is_empty()
        || work_commands.iter().any(Command::is_major)
    {
        return None;
    }
    let destination = |commands: &[Command]| {
        commands.iter().find_map(|c| match c {
            Command::Move { destination } => Some(*destination),
            _ => None,
        })
    };
    let combat_move = destination(&combat_commands);
    let work_move = destination(&work_commands);
    let mut result = Vec::new();
    match (combat_move, work_move) {
        (Some(a), Some(b)) if a != b => return None,
        (Some(destination), Some(_)) => {
            result.push(Command::Move { destination });
            result.extend(
                combat_commands
                    .into_iter()
                    .filter(|c| !matches!(c, Command::Move { .. })),
            );
            result.extend(
                work_commands
                    .into_iter()
                    .filter(|c| !matches!(c, Command::Move { .. })),
            );
        }
        (Some(_), None) => {
            // Work at the starting position, then keep the combat approach.
            result.extend(work_commands);
            result.extend(combat_commands);
        }
        (None, destination) => {
            if let Some(destination) = destination {
                let portal = combat.target_portal()?;
                let remote = matches!(combat, Task::Recharge { .. })
                    && me.keys.get(&portal).copied().unwrap_or(0) > 0;
                if !remote
                    && !common::in_range(
                        destination,
                        common::portal(obs, portal).position,
                        core.cfg().portal_operation_range_cm,
                    )
                {
                    return None;
                }
            }
            // Charge BEFORE a Link can consume the last remote Key or the
            // construction route can leave operation range.
            result.extend(combat_commands);
            result.extend(work_commands);
        }
    }
    (result.len() <= core.cfg().max_commands_per_agent_round as usize).then_some(result)
}

/// One optional Hack at a visited position. Its cooldown is booked only
/// on actual emission, never during allocation previews. Existing work is
/// unchanged; a major action may restore a slot for Supply below the cap.
pub(super) fn add_hack(
    core: &BuilderCore,
    obs: &Observation,
    me: &AgentPrivate,
    commands: &mut Vec<Command>,
) {
    if commands.len() >= core.cfg().max_commands_per_agent_round as usize
        || commands.iter().any(|c| matches!(c, Command::Hack { .. }))
    {
        return;
    }
    let spends_supply = commands
        .iter()
        .any(|c| matches!(c, Command::Recharge { .. }));
    if me.supply >= core.cfg().max_supply && !spends_supply {
        return;
    }
    // Prefer the destination so Recharge can consume Supply before Hack
    // refills it. Otherwise harvest at the starting position before moving.
    let destination = commands
        .iter()
        .rev()
        .find_map(|c| match c {
            Command::Move { destination } => Some(*destination),
            _ => None,
        })
        .unwrap_or(me.position);
    for (position, at_end) in [(destination, true), (me.position, false)] {
        if !at_end && me.supply >= core.cfg().max_supply {
            continue;
        }
        let portal = obs
            .portals
            .iter()
            .filter(|p| {
                common::in_range(position, p.position, core.cfg().portal_operation_range_cm)
                    && core.hack_available(obs.round, me.id, p.id)
            })
            .min_by_key(|p| (arena_core::geometry::dist2(position, p.position), p.id));
        if let Some(portal) = portal {
            let command = Command::Hack { portal: portal.id };
            if at_end {
                commands.push(command);
            } else {
                commands.insert(0, command);
            }
            return;
        }
    }
}

/// Low-stock safety net, independent of obtaining a combat grant. Existing
/// work keeps its relative order; movement is added only to an idle stream or a
/// keyed remote Recharge with no route already assigned, and only when a Hack
/// can complete this round. Local defenders,
/// attackers and the stateful Transfer planner keep their stations/routes.
pub(super) fn replenish(
    core: &BuilderCore,
    obs: &Observation,
    me: &AgentPrivate,
    idle: bool,
    commands: &mut Vec<Command>,
) {
    let cfg = core.cfg();
    let reserve = core.params().supply_reserve_recharges;
    if reserve == 0
        || me.supply > reserve.saturating_mul(cfg.recharge_cost_supply)
        || me.supply >= cfg.max_supply
        || commands
            .iter()
            .any(|c| matches!(c, Command::Transfer { .. }))
    {
        return;
    }
    add_hack(core, obs, me, commands);
    if commands.len() >= cfg.max_commands_per_agent_round as usize
        || commands
            .iter()
            .any(|c| matches!(c, Command::Move { .. } | Command::Hack { .. }))
    {
        return;
    }
    let remote_charge = commands.iter().any(|c| match c {
        Command::Recharge { portal } => me.keys.get(portal).copied().unwrap_or(0) > 0,
        _ => false,
    });
    if !(idle || remote_charge)
        || cfg.move_budget_cm == 0
        || commands.len() + 2 > cfg.max_commands_per_agent_round as usize
    {
        return;
    }
    let Some(portal) = obs
        .portals
        .iter()
        .filter(|p| {
            core.hack_available(obs.round, me.id, p.id)
                && common::in_range(
                    common::step_toward(me.position, p.position, cfg.move_budget_cm),
                    p.position,
                    cfg.portal_operation_range_cm,
                )
        })
        .min_by_key(|p| (arena_core::geometry::dist2(me.position, p.position), p.id))
    else {
        return;
    };
    let destination = common::step_toward(me.position, portal.position, cfg.move_budget_cm);
    if destination == me.position {
        return;
    }
    commands.push(Command::Move { destination });
    // Do not begin a multi-round tour that can reverse on the next cooldown
    // reset or lengthen a future build route before harvesting anything.
    add_hack(core, obs, me, commands);
}
