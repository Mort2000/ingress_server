//! Rust twins of the viewer's view predicates (checklist item 7,
//! P6-GATE(c)). `viewer/filter.js` is the single source of the predicate
//! inside the page; the functions below implement the IDENTICAL rules over
//! the parsed embedded payload so the machine tests can pin them:
//!
//! * God View = public state + every EMBEDDED faction's agents;
//! * Faction F View = public state + `agents[F]` (and nothing else);
//! * Public View = public state only — no agents at all.
//!
//! Enemy agents never appear in a faction view (R-VIS-02: positions are
//! private), in `--dev 0` exports because the enemy array is not embedded
//! at all (D90 construction guarantee), and in `--dev 1` exports because
//! the predicate selects only the view's own faction.

use crate::json::Json;

/// The three view kinds behind the four view labels (the two faction
/// labels share `Enl`/`Res`, `Public`, `God`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Public,
    Enl,
    Res,
    God,
}

/// The faction keys a view may read agents from, in embedding order.
/// Mirrors `Filter.visibleFactions` in `viewer/filter.js` exactly.
pub fn visible_factions(view: View, embedded: &[&str]) -> Vec<&'static str> {
    let allows = |key: &str| embedded.contains(&key);
    match view {
        View::Public => Vec::new(),
        View::God => ["enl", "res"].into_iter().filter(|f| allows(f)).collect(),
        View::Enl => {
            if allows("enl") {
                vec!["enl"]
            } else {
                Vec::new()
            }
        }
        View::Res => {
            if allows("res") {
                vec!["res"]
            } else {
                Vec::new()
            }
        }
    }
}

/// One visible agent at a round: `(faction key, id, x, y, supply)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VisibleAgent {
    pub faction: &'static str,
    pub id: i64,
    pub x: i64,
    pub y: i64,
    pub supply: i64,
}

/// The agents `view` shows at `round` (1-based; `round == 0` derives the
/// pre-match spawn dots from `header.spawns`). Mirrors
/// `Filter.visibleAgents(view, i)` in `viewer/filter.js` exactly.
pub fn visible_agents(payload: &Json, view: View, round: usize) -> Vec<VisibleAgent> {
    let embedded: Vec<&str> = payload
        .get("header")
        .and_then(|h| h.get("factions_embedded"))
        .and_then(Json::as_array)
        .map(|keys| keys.iter().filter_map(Json::as_str).collect::<Vec<&str>>())
        .unwrap_or_default();
    let mut out = Vec::new();
    for faction in visible_factions(view, &embedded) {
        if round == 0 {
            let rows = payload
                .get("header")
                .and_then(|h| h.get("spawns"))
                .and_then(|s| s.get(faction))
                .and_then(Json::as_array)
                .unwrap_or(&[]);
            for row in rows {
                let Some(entries) = row.as_array() else {
                    continue;
                };
                if entries.len() == 3 {
                    out.push(VisibleAgent {
                        faction,
                        id: entries[0].as_int().unwrap_or(-1),
                        x: entries[1].as_int().unwrap_or(-1),
                        y: entries[2].as_int().unwrap_or(-1),
                        supply: -1, // pre-match: no supply exists yet
                    });
                }
            }
        } else if let Some(rounds) = payload
            .get("agents")
            .and_then(|a| a.get(faction))
            .and_then(Json::as_array)
        {
            if let Some(rows) = rounds.get(round - 1).and_then(Json::as_array) {
                for row in rows {
                    let Some(entries) = row.as_array() else {
                        continue;
                    };
                    if entries.len() == 4 {
                        out.push(VisibleAgent {
                            faction,
                            id: entries[0].as_int().unwrap_or(-1),
                            x: entries[1].as_int().unwrap_or(-1),
                            y: entries[2].as_int().unwrap_or(-1),
                            supply: entries[3].as_int().unwrap_or(-1),
                        });
                    }
                }
            }
        }
    }
    out
}

/// The event-log predicate (mirrors `Filter.eventVisible` in
/// `viewer/filter.js`): an event is visible when its kind is checked AND
/// its faction (when it has one — `REINFORCEMENT` is both-faction) is
/// checked.
pub fn event_visible(kind: i64, faction: Option<i64>, kinds: &[i64], factions: &[i64]) -> bool {
    kinds.contains(&kind) && faction.is_none_or(|f| factions.contains(&f))
}
