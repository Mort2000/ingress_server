//! Core domain types (P0-MODEL). Terminology follows `CONTEXT.md`.

use std::collections::BTreeMap;

/// Fixed: one coordinate unit equals one centimetre (R-CONFIG-01 `COORDINATE_UNIT`).
pub const COORDINATE_UNIT_CM: i32 = 1;

/// Twice the triangle area in squared coordinate units (ADR-0007).
pub type Area2 = u64;

/// Accumulated faction score in twice-squared coordinate units (ADR-0007).
pub type Score2 = u128;

/// Dense portal identifier. Map definitions assign `0..n-1` in placement order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PortalId(pub u32);

/// Match-unique agent identifier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AgentId(pub u32);

/// One of the two opposing factions; v0.1 is fixed to ENL and RES.
/// `Ord` is declaration order (`Enl < Res`), used for score maps only.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Faction {
    Enl,
    Res,
}

impl Faction {
    /// The opposing faction; v0.1 has exactly two.
    pub fn opposite(self) -> Faction {
        match self {
            Faction::Enl => Faction::Res,
            Faction::Res => Faction::Enl,
        }
    }
}

/// Portal ownership; neutral portals can be captured, owned portals attacked.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Owner {
    Neutral,
    Enl,
    Res,
}

/// A map node with fixed position, ownership, and hit points.
///
/// Generated portals start `Owner::Neutral` with `hp == 0`; HP only changes on
/// capture (`PORTAL_MAX_HP`) or neutralization (0) per the portal rules.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Portal {
    pub id: PortalId,
    pub position: Coord,
    pub owner: Owner,
    pub hp: u32,
}

/// A point-like match unit belonging to a faction.
///
/// Agents have no collision volume: same or overlapping coordinates are always
/// legal (R-AGENT-01). Starts with `INITIAL_SUPPLY` and an empty key inventory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Agent {
    pub id: AgentId,
    pub faction: Faction,
    pub position: Coord,
    pub supply: u32,
    pub keys: BTreeMap<PortalId, u32>,
}

/// Undirected simple edge identity between two distinct portals (R-LINK-01):
/// the canonical (min, max) pair makes `LINK(A,B)` and `LINK(B,A)` the same edge.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LinkId {
    pub min: PortalId,
    pub max: PortalId,
}

impl LinkId {
    /// Canonical identity for the undirected pair; `None` on a self-loop
    /// (a link requires two distinct portals).
    pub fn new(a: PortalId, b: PortalId) -> Option<LinkId> {
        if a == b {
            None
        } else if a < b {
            Some(LinkId { min: a, max: b })
        } else {
            Some(LinkId { min: b, max: a })
        }
    }

    /// Whether portal `p` is an endpoint of this link.
    pub fn contains(&self, p: PortalId) -> bool {
        p == self.min || p == self.max
    }
}

/// Integer two-dimensional coordinate in centimetres, origin at the map's
/// lower-left corner (R-MAP-01).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl Coord {
    pub const fn new(x: i32, y: i32) -> Self {
        Coord { x, y }
    }
}

/// Command payload submitted inside an Agent Command Stream (R-ACTION-01).
///
/// M2 carries MOVE, ATTACK, CAPTURE, and RECHARGE; M3 adds HACK and TRANSFER
/// (both auxiliary). `Clone` but not `Copy` since D30 — TRANSFER owns a key
/// map, and every construction site moves or borrows commands anyway.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// Absolute destination coordinate; cost is billed per segment from the
    /// agent's latest position (R-MOVE-01). Auxiliary action.
    Move { destination: Coord },
    /// Damages every enemy portal in operation range (R-ATTACK-01). Major action.
    Attack,
    /// Takes ownership of a neutral portal (R-CAPTURE-01). Major action.
    Capture { portal: PortalId },
    /// Pushes supply into an owned portal (R-RECHARGE-01). Major action.
    Recharge { portal: PortalId },
    /// Farms `HACK_KEY_YIELD` keys and `HACK_SUPPLY_YIELD` supply from a
    /// portal in operation range (R-HACK-01); the portal may be neutral,
    /// own, or enemy. Auxiliary action.
    Hack { portal: PortalId },
    /// One-way transfer of supply and keys to a same-faction agent within
    /// `TRANSFER_RANGE` (R-TRANSFER-01). The payload names only the
    /// receiver: the sender is always the Command Stream's own agent, so a
    /// command can never spend another agent's resources. `keys` is a
    /// `BTreeMap` for canonical ascending order with free dedup; zero-count
    /// entries are inert. Auxiliary action.
    Transfer {
        target_agent: AgentId,
        supply: u32,
        keys: BTreeMap<PortalId, u32>,
    },
}

impl Command {
    /// Whether this command belongs to the Major Action category
    /// (R-ACTION-01): ATTACK, RECHARGE, and CAPTURE — at most one per agent
    /// per round. MOVE, HACK, and TRANSFER (LINK lands later) are auxiliary.
    pub fn is_major(&self) -> bool {
        matches!(
            self,
            Command::Attack | Command::Capture { .. } | Command::Recharge { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linkid_self_loop_rejected() {
        assert!(LinkId::new(PortalId(3), PortalId(3)).is_none());
        assert!(LinkId::new(PortalId(0), PortalId(0)).is_none());
    }

    #[test]
    fn linkid_sorted_both_directions_equal() {
        let a = PortalId(9);
        let b = PortalId(4);
        let forward = LinkId::new(a, b).expect("distinct portals yield a link");
        let backward = LinkId::new(b, a).expect("order does not matter");
        assert_eq!(forward, backward);
        assert_eq!(forward.min, PortalId(4));
        assert_eq!(forward.max, PortalId(9));
        assert!(forward.min < forward.max);
    }

    #[test]
    fn linkid_contains_endpoints() {
        let link = LinkId::new(PortalId(2), PortalId(7)).expect("distinct portals");
        assert!(link.contains(PortalId(2)));
        assert!(link.contains(PortalId(7)));
        assert!(!link.contains(PortalId(3)));
        assert!(!link.contains(PortalId(0)));
    }

    #[test]
    fn link_through_third_portal_geometry() {
        use crate::geometry::point_on_segment;
        // A link (0,0)-(100,0) passing through a portal at (50,0).
        assert!(point_on_segment(
            Coord::new(50, 0),
            Coord::new(0, 0),
            Coord::new(100, 0)
        ));
        // A portal one unit off the line is not on the segment.
        assert!(!point_on_segment(
            Coord::new(50, 1),
            Coord::new(0, 0),
            Coord::new(100, 0)
        ));
    }

    #[test]
    fn command_move_holds_absolute_destination() {
        let command = Command::Move {
            destination: Coord::new(351_230, 178_452),
        };
        match command {
            Command::Move { destination } => {
                assert_eq!(destination, Coord::new(351_230, 178_452));
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn command_major_classification() {
        // R-ACTION-01: ATTACK, RECHARGE, CAPTURE are Major Actions; MOVE,
        // HACK, and TRANSFER are auxiliary.
        assert!(
            !Command::Move {
                destination: Coord::new(0, 0)
            }
            .is_major()
        );
        assert!(Command::Attack.is_major());
        assert!(
            Command::Capture {
                portal: PortalId(0)
            }
            .is_major()
        );
        assert!(
            Command::Recharge {
                portal: PortalId(0)
            }
            .is_major()
        );
        assert!(
            !Command::Hack {
                portal: PortalId(0)
            }
            .is_major()
        );
        assert!(
            !Command::Transfer {
                target_agent: AgentId(1),
                supply: 0,
                keys: BTreeMap::new(),
            }
            .is_major()
        );
    }

    // Checklist 3 (M3): HACK and TRANSFER payload shapes. HACK mirrors the
    // CAPTURE naming (a single `portal` field); TRANSFER names only the
    // receiver — the sender is the stream owner by construction
    // (R-TRANSFER-01) and cannot be set by the payload.
    #[test]
    fn command_hack_and_transfer_payload_shapes() {
        match (Command::Hack {
            portal: PortalId(4),
        }) {
            Command::Hack { portal } => assert_eq!(portal, PortalId(4)),
            _ => panic!("unexpected command variant"),
        }
        let mut keys = BTreeMap::new();
        keys.insert(PortalId(2), 3u32);
        keys.insert(PortalId(7), 1u32);
        let transfer = Command::Transfer {
            target_agent: AgentId(5),
            supply: 12,
            keys,
        };
        match &transfer {
            Command::Transfer {
                target_agent,
                supply,
                keys,
            } => {
                assert_eq!(*target_agent, AgentId(5));
                assert_eq!(*supply, 12);
                // Canonical ascending PortalId order with free dedup.
                let ids: Vec<PortalId> = keys.keys().copied().collect();
                assert_eq!(ids, vec![PortalId(2), PortalId(7)]);
                assert_eq!(keys[&PortalId(2)], 3);
            }
            _ => panic!("unexpected command variant"),
        }
        // A command clones deeply (Clone replaces the removed Copy, D30).
        let cloned = transfer.clone();
        assert_eq!(cloned, transfer);
    }

    #[test]
    fn faction_opposite_swaps() {
        assert_eq!(Faction::Enl.opposite(), Faction::Res);
        assert_eq!(Faction::Res.opposite(), Faction::Enl);
        // Round trip is the identity for both factions.
        for faction in [Faction::Enl, Faction::Res] {
            assert_eq!(faction.opposite().opposite(), faction);
        }
    }

    #[test]
    fn coordinate_unit_is_one_cm() {
        assert_eq!(COORDINATE_UNIT_CM, 1);
    }
}
