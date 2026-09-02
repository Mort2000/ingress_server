//! Broadcast events for the Public Event History (R-EVENT-01).
//!
//! `MOVE`, `HACK`, `RECHARGE`, and `TRANSFER` never emit events. `seq` is a
//! global, 0-based, strictly increasing counter owned by
//! [`crate::state::GameState`]; the log is append-only per round and never
//! reordered by execution results.

use crate::types::{AgentId, Faction, PortalId};

/// One entry of the Public Event History.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Event {
    /// Global 0-based sequence number, strictly increasing across rounds.
    pub seq: u64,
    /// Round in which the event was generated (1-based).
    pub round: u32,
    pub kind: EventKind,
}

/// Broadcast event payloads. Every event carries the acting faction, agent,
/// and the affected portal (R-EVENT-01 minimum fields).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// A portal changed owner via CAPTURE (R-CAPTURE-01).
    PortalCaptured {
        faction: Faction,
        agent_id: AgentId,
        portal: PortalId,
    },
    /// A portal took fixed damage from ATTACK (R-ATTACK-01); `hp_after` is
    /// the post-damage HP (0 when the attack neutralized the portal).
    PortalAttacked {
        faction: Faction,
        agent_id: AgentId,
        portal: PortalId,
        damage: u32,
        hp_after: u32,
    },
    /// A portal's HP reached 0 and it was neutralized (R-ATTACK-02).
    PortalNeutralized {
        faction: Faction,
        agent_id: AgentId,
        portal: PortalId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_carries_seq_round_and_payload() {
        let event = Event {
            seq: 7,
            round: 3,
            kind: EventKind::PortalCaptured {
                faction: Faction::Res,
                agent_id: AgentId(4),
                portal: PortalId(17),
            },
        };
        assert_eq!(event.seq, 7);
        assert_eq!(event.round, 3);
        assert_eq!(
            event.kind,
            EventKind::PortalCaptured {
                faction: Faction::Res,
                agent_id: AgentId(4),
                portal: PortalId(17)
            }
        );
    }

    #[test]
    fn attacked_event_exposes_damage_and_hp_after() {
        let kind = EventKind::PortalAttacked {
            faction: Faction::Enl,
            agent_id: AgentId(0),
            portal: PortalId(2),
            damage: 20,
            hp_after: 80,
        };
        match kind {
            EventKind::PortalAttacked {
                faction,
                agent_id,
                portal,
                damage,
                hp_after,
            } => {
                assert_eq!(faction, Faction::Enl);
                assert_eq!(agent_id, AgentId(0));
                assert_eq!(portal, PortalId(2));
                assert_eq!(damage, 20);
                assert_eq!(hp_after, 80);
            }
            _ => panic!("unexpected event kind"),
        }
    }
}
