//! Zero-copy observation views over the two SHM snapshots (ADR-0006).
//! `ObservationView<'round>` borrows the payload slices the seqlock read
//! lent, so it cannot outlive the [`crate::round::RoundContext`] that
//! produced it — not storable in bot state, not returnable from
//! `on_round`, not sendable to a detached thread (compile-fail tests
//! pin all three).
//!
//! Every accessor also re-checks the session's [`crate::phase::PhaseGate`]:
//! a view lent while the round was open becomes unreadable the moment the
//! participant loop sees ROUND_CLOSED (Settlement Phase, P3-RPC — "从
//! ROUND_CLOSED 到下一次 ROUND_START_SIGNAL，Client 读取任何 SHM 都属于
//! 作弊"; the official SDK refusing IS the v0.1 enforcement). Without this,
//! a view COPY held by a scoped worker thread could keep reading the
//! mapping while the main thread waits for the next ROUND_START_SIGNAL.

use arena_core::event::Event;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction, FieldId, LinkId, PortalId, Score2};

use arena_protocol::decode as proto_decode;
use arena_protocol::generated::snap_private as snap_priv;
use arena_protocol::generated::snap_public as snap_pub;

use crate::SdkError;
use crate::phase::PhaseGate;

/// Reads planus refs off the borrowed payload with error mapping.
fn planus<T>(result: Result<T, planus::Error>) -> Result<T, SdkError> {
    result.map_err(|error| SdkError::Protocol(format!("snapshot access failed: {error:?}")))
}

/// The zero-copy round-start observation: the public snapshot plus this
/// faction's private snapshot, as generated `*Ref` views over the mapped
/// buffers. Every accessor walks (and thus verifies) its field eagerly —
/// and refuses outside the licensed phase (Settlement Phase refusal).
#[derive(Clone, Copy)]
pub struct ObservationView<'round> {
    public: snap_pub::PublicSnapshotRef<'round>,
    private: snap_priv::PrivateSnapshotRef<'round>,
    /// The session's phase gate, borrowed with the same lifetime as the
    /// payload: every accessor asserts the read is still licensed.
    phase: &'round PhaseGate,
}

impl<'round> ObservationView<'round> {
    /// Builds the view over two seqlock-stable payload slices. The caller
    /// (the round context) has already verified file identifiers.
    pub(crate) fn new(
        public_bytes: &'round [u8],
        private_bytes: &'round [u8],
        phase: &'round PhaseGate,
    ) -> Result<Self, SdkError> {
        use planus::ReadAsRoot;
        let public = planus(snap_pub::PublicSnapshotRef::read_as_root(
            proto_decode::strip_file_identifier(public_bytes, proto_decode::PUBLIC_FILE_IDENTIFIER)
                .map_err(|e| SdkError::Protocol(e.to_string()))?,
        ))?;
        let private = planus(snap_priv::PrivateSnapshotRef::read_as_root(
            proto_decode::strip_file_identifier(
                private_bytes,
                proto_decode::PRIVATE_FILE_IDENTIFIER,
            )
            .map_err(|e| SdkError::Protocol(e.to_string()))?,
        ))?;
        Ok(ObservationView {
            public,
            private,
            phase,
        })
    }

    /// The snapshot generation this view reads.
    pub fn snapshot_version(&self) -> Result<u64, SdkError> {
        self.phase.assert_read_licensed()?;
        planus(self.public.snapshot_version())
    }

    /// Current round of the public snapshot.
    pub fn round(&self) -> Result<u32, SdkError> {
        self.phase.assert_read_licensed()?;
        planus(self.public.current_round())
    }

    /// Public portals (zero-copy walk into owned `PortalPublic` values).
    pub fn portals(&self) -> Result<Vec<arena_core::observe::PortalPublic>, SdkError> {
        self.phase.assert_read_licensed()?;
        let mut out = Vec::new();
        if let Some(vector) = planus(self.public.portals())? {
            out.reserve(vector.len());
            for entry in vector.iter() {
                let entry = planus(entry)?;
                out.push(arena_core::observe::PortalPublic {
                    id: PortalId(planus(entry.id())?),
                    position: Coord::new(planus(entry.x())?, planus(entry.y())?),
                    owner: proto_decode::owner_from_byte(planus(entry.owner())?)
                        .ok_or_else(|| SdkError::Protocol("unknown owner tag".into()))?,
                    hp: planus(entry.hp())?,
                });
            }
        }
        Ok(out)
    }

    /// Public links.
    pub fn links(&self) -> Result<Vec<arena_core::observe::LinkPublic>, SdkError> {
        self.phase.assert_read_licensed()?;
        let mut out = Vec::new();
        if let Some(vector) = planus(self.public.links())? {
            out.reserve(vector.len());
            for entry in vector.iter() {
                let entry = planus(entry)?;
                let a = PortalId(planus(entry.a())?);
                let b = PortalId(planus(entry.b())?);
                let faction = faction_of(planus(entry.faction())?)?;
                out.push(arena_core::observe::LinkPublic {
                    id: LinkId {
                        min: if a < b { a } else { b },
                        max: if a < b { b } else { a },
                    },
                    faction,
                });
            }
        }
        Ok(out)
    }

    /// Public fields (exact integer `area2`).
    pub fn fields(&self) -> Result<Vec<arena_core::observe::FieldPublic>, SdkError> {
        self.phase.assert_read_licensed()?;
        let mut out = Vec::new();
        if let Some(vector) = planus(self.public.fields())? {
            out.reserve(vector.len());
            for entry in vector.iter() {
                let entry = planus(entry)?;
                let (a, b, c) = (
                    PortalId(planus(entry.a())?),
                    PortalId(planus(entry.b())?),
                    PortalId(planus(entry.c())?),
                );
                out.push(arena_core::observe::FieldPublic {
                    id: FieldId::new(a, b, c)
                        .ok_or_else(|| SdkError::Protocol("degenerate field triple".into()))?,
                    faction: faction_of(planus(entry.faction())?)?,
                    area2: planus(entry.area2())?,
                });
            }
        }
        Ok(out)
    }

    /// Full public event history.
    pub fn events(&self) -> Result<Vec<Event>, SdkError> {
        self.phase.assert_read_licensed()?;
        let mut out = Vec::new();
        if let Some(vector) = planus(self.public.events())? {
            out.reserve(vector.len());
            for entry in vector.iter() {
                let entry = planus(entry)?;
                let seq = planus(entry.seq())?;
                let round = planus(entry.round())?;
                let body = planus(entry.body())?
                    .ok_or_else(|| SdkError::Protocol("event entry without a body".into()))?;
                let kind = convert_event_body(body)?;
                out.push(Event { seq, round, kind });
            }
        }
        Ok(out)
    }

    /// Both factions' scores (exact u128).
    pub fn scores(&self) -> Result<[(Faction, Score2); 2], SdkError> {
        self.phase.assert_read_licensed()?;
        let enl = proto_decode::join_u128(
            planus(self.public.score_enl_hi())?,
            planus(self.public.score_enl_lo())?,
        );
        let res = proto_decode::join_u128(
            planus(self.public.score_res_hi())?,
            planus(self.public.score_res_lo())?,
        );
        Ok([(Faction::Enl, enl), (Faction::Res, res)])
    }

    /// This faction's own agents (the private snapshot).
    pub fn own_agents(&self) -> Result<Vec<arena_core::observe::AgentPrivate>, SdkError> {
        self.phase.assert_read_licensed()?;
        let mut out = Vec::new();
        if let Some(vector) = planus(self.private.agents())? {
            out.reserve(vector.len());
            for entry in vector.iter() {
                let entry = planus(entry)?;
                let id = AgentId(planus(entry.id())?);
                let position = Coord::new(planus(entry.x())?, planus(entry.y())?);
                let supply = planus(entry.supply())?;
                let mut keys = std::collections::BTreeMap::new();
                if let Some(key_vector) = planus(entry.keys())? {
                    for key in key_vector.iter() {
                        let key = planus(key)?;
                        keys.insert(PortalId(planus(key.portal())?), planus(key.count())?);
                    }
                }
                out.push(arena_core::observe::AgentPrivate {
                    id,
                    position,
                    supply,
                    keys,
                });
            }
        }
        Ok(out)
    }

    /// Full deep conversion for wrapping arena-bots strategies (the
    /// external-client path reuses the frozen M6 algorithms verbatim).
    /// Walks every field — also the strongest verification of the buffer.
    pub fn to_observation(&self) -> Result<Observation, SdkError> {
        let [(_, enl), (_, res)] = self.scores()?;
        let mut scores = std::collections::BTreeMap::new();
        scores.insert(Faction::Enl, enl);
        scores.insert(Faction::Res, res);
        Ok(Observation {
            round: self.round()?,
            portals: self.portals()?,
            links: self.links()?,
            fields: self.fields()?,
            events: self.events()?,
            scores,
            own_agents: self.own_agents()?,
        })
    }
}

fn faction_of(tag: u8) -> Result<Faction, SdkError> {
    proto_decode::faction_from_byte(tag)
        .ok_or_else(|| SdkError::Protocol("unknown faction tag".into()))
}

fn convert_event_body(
    body: snap_pub::EventBodyRef<'_>,
) -> Result<arena_core::event::EventKind, SdkError> {
    use arena_core::event::EventKind;
    use snap_pub::EventBodyRef as Body;
    Ok(match body {
        Body::PortalCaptured(inner) => EventKind::PortalCaptured {
            faction: faction_of(planus(inner.faction())?)?,
            agent_id: AgentId(planus(inner.agent_id())?),
            portal: PortalId(planus(inner.portal())?),
        },
        Body::PortalAttacked(inner) => EventKind::PortalAttacked {
            faction: faction_of(planus(inner.faction())?)?,
            agent_id: AgentId(planus(inner.agent_id())?),
            portal: PortalId(planus(inner.portal())?),
            damage: planus(inner.damage())?,
            hp_after: planus(inner.hp_after())?,
        },
        Body::PortalNeutralized(inner) => EventKind::PortalNeutralized {
            faction: faction_of(planus(inner.faction())?)?,
            agent_id: AgentId(planus(inner.agent_id())?),
            portal: PortalId(planus(inner.portal())?),
        },
        Body::LinkCreated(inner) | Body::LinkDestroyed(inner) => {
            let a = PortalId(planus(inner.a())?);
            let b = PortalId(planus(inner.b())?);
            let link = LinkId {
                min: if a < b { a } else { b },
                max: if a < b { b } else { a },
            };
            let faction = faction_of(planus(inner.faction())?)?;
            let agent_id = AgentId(planus(inner.agent_id())?);
            match body {
                Body::LinkCreated(_) => EventKind::LinkCreated {
                    faction,
                    agent_id,
                    link,
                },
                _ => EventKind::LinkDestroyed {
                    faction,
                    agent_id,
                    link,
                },
            }
        }
        Body::FieldCreated(inner) | Body::FieldDestroyed(inner) => {
            let (a, b, c) = (
                PortalId(planus(inner.a())?),
                PortalId(planus(inner.b())?),
                PortalId(planus(inner.c())?),
            );
            let field = FieldId::new(a, b, c)
                .ok_or_else(|| SdkError::Protocol("degenerate field triple".into()))?;
            let faction = faction_of(planus(inner.faction())?)?;
            let agent_id = AgentId(planus(inner.agent_id())?);
            match body {
                Body::FieldCreated(_) => EventKind::FieldCreated {
                    faction,
                    agent_id,
                    field,
                },
                _ => EventKind::FieldDestroyed {
                    faction,
                    agent_id,
                    field,
                },
            }
        }
        Body::Reinforcement(inner) => EventKind::Reinforcement {
            index: planus(inner.index())?,
            position: Coord::new(planus(inner.x())?, planus(inner.y())?),
            count_per_faction: planus(inner.count())?,
            first_agent_id: AgentId(planus(inner.first_agent_id())?),
        },
    })
}

/// A pre-match map view (generation 0 of the public region). Like
/// [`ObservationView`], every accessor re-checks the session's phase gate
/// — the map view is licensed only while reads are licensed.
pub struct MapView<'spawn> {
    public: snap_pub::PublicSnapshotRef<'spawn>,
    phase: &'spawn PhaseGate,
}

impl<'spawn> MapView<'spawn> {
    pub(crate) fn new(
        public_bytes: &'spawn [u8],
        phase: &'spawn PhaseGate,
    ) -> Result<Self, SdkError> {
        use planus::ReadAsRoot;
        let public = planus(snap_pub::PublicSnapshotRef::read_as_root(
            proto_decode::strip_file_identifier(public_bytes, proto_decode::PUBLIC_FILE_IDENTIFIER)
                .map_err(|e| SdkError::Protocol(e.to_string()))?,
        ))?;
        Ok(MapView { public, phase })
    }

    /// The frozen map portals (owner/hp are the pre-match values).
    pub fn portals(&self) -> Result<Vec<arena_core::types::Portal>, SdkError> {
        self.phase.assert_read_licensed()?;
        let mut out = Vec::new();
        if let Some(vector) = planus(self.public.portals())? {
            out.reserve(vector.len());
            for entry in vector.iter() {
                let entry = planus(entry)?;
                let owner = proto_decode::owner_from_byte(planus(entry.owner())?)
                    .ok_or_else(|| SdkError::Protocol("unknown owner tag".into()))?;
                out.push(arena_core::types::Portal {
                    id: PortalId(planus(entry.id())?),
                    position: Coord::new(planus(entry.x())?, planus(entry.y())?),
                    owner,
                    hp: planus(entry.hp())?,
                });
            }
        }
        Ok(out)
    }

    /// Deep conversion to a `MapDefinition` for arena-bots strategies.
    pub fn to_map_definition(&self) -> Result<arena_core::map::MapDefinition, SdkError> {
        self.phase.assert_read_licensed()?;
        Ok(arena_core::map::MapDefinition::new(self.portals()?))
    }

    /// Generation of this map view (0 for the pre-match publication).
    pub fn snapshot_version(&self) -> Result<u64, SdkError> {
        self.phase.assert_read_licensed()?;
        planus(self.public.snapshot_version())
    }
}
