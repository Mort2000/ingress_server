//! Link graph with the derived field cache (R-LINK-01, R-FIELD-01/02,
//! P1-FIELD). Owned by [`crate::state::GameState`]; mutations are
//! `pub(crate)` and driven only by the resolver path.
//!
//! Storage (D36): `links` is the authoritative edge map keyed by the
//! canonical [`LinkId`]; `adjacency` is a neighbor index maintained in
//! lockstep so `Neighbors(A) ∩ Neighbors(B)` costs O(deg); `fields` caches
//! every derived triangle with its `area2` computed ONCE at derivation, so
//! round scoring is O(F) u128 adds with zero per-round geometry.
//!
//! Derivation invariant (D40): every link incident to a portal carries that
//! portal's owner faction — neutralization cascades links away before a
//! recapture is possible (capture requires Neutral) — so the three vertices
//! of any cached field automatically share one faction and derivation needs
//! no owner check.

use std::collections::{BTreeMap, BTreeSet};

use crate::geometry::area2;
use crate::types::{Area2, Coord, Faction, FieldId, LinkId, PortalId};

/// An undirected link edge; endpoints live in the canonical [`LinkId`].
/// The faction is stable for the link's lifetime (D40 invariant).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link {
    pub faction: Faction,
}

/// A derived triangle field with its cached twice-area (R-FIELD-02).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Field {
    pub faction: Faction,
    pub area2: Area2,
}

/// What a portal neutralization removed: incident links and the fields that
/// died with them, both ascending (D40). A layered field spanning two
/// removed edges is listed exactly once.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct NeutralizeCascade {
    pub links: Vec<LinkId>,
    pub fields: Vec<FieldId>,
}

/// Link storage plus the derived field cache.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LinkGraph {
    links: BTreeMap<LinkId, Link>,
    adjacency: BTreeMap<PortalId, BTreeSet<PortalId>>,
    fields: BTreeMap<FieldId, Field>,
}

impl LinkGraph {
    /// All links, ascending `LinkId`.
    pub fn links(&self) -> &BTreeMap<LinkId, Link> {
        &self.links
    }

    /// One link by canonical id.
    pub fn link(&self, id: LinkId) -> Option<&Link> {
        self.links.get(&id)
    }

    /// All derived fields, ascending `FieldId`.
    pub fn fields(&self) -> &BTreeMap<FieldId, Field> {
        &self.fields
    }

    /// Neighbors of `p` in the link graph; empty for a portalless id.
    pub fn neighbors(&self, p: PortalId) -> BTreeSet<PortalId> {
        self.adjacency.get(&p).cloned().unwrap_or_default()
    }

    /// Inserts a resolver-validated link and derives every field it closes
    /// (R-FIELD-01/02, P1-FIELD): the common neighbors of the two endpoints
    /// BEFORE the edge exists. Every derived field is provably new — an
    /// existing field `(A,B,C)` implies the link `(A,B)` already exists,
    /// contradicting the resolver's duplicate check. Returns the derived
    /// field ids ascending. `position` maps a portal id to its frozen map
    /// coordinate; `area2` is computed exactly once per field here.
    pub(crate) fn insert_link<F: Fn(PortalId) -> Coord>(
        &mut self,
        id: LinkId,
        faction: Faction,
        position: F,
    ) -> Vec<FieldId> {
        let empty = BTreeSet::new();
        let neighbors_min = self.adjacency.get(&id.min).unwrap_or(&empty);
        let neighbors_max = self.adjacency.get(&id.max).unwrap_or(&empty);
        let common: Vec<PortalId> = neighbors_min.intersection(neighbors_max).copied().collect();

        self.adjacency.entry(id.min).or_default().insert(id.max);
        self.adjacency.entry(id.max).or_default().insert(id.min);
        self.links.insert(id, Link { faction });

        let mut derived = Vec::with_capacity(common.len());
        for c in common {
            let field_id =
                FieldId::new(id.min, id.max, c).expect("a neighbor is never an endpoint");
            let area2 = area2(
                position(field_id.a),
                position(field_id.b),
                position(field_id.c),
            );
            self.fields.insert(field_id, Field { faction, area2 });
            derived.push(field_id);
        }
        derived
    }

    /// Removes every link incident to `p` together with the fields that die
    /// (R-FIELD-01 cache maintenance, D40). Dead fields are collected into a
    /// set BEFORE any removal — a layered field spanning two removed edges is
    /// counted once — then the links (map and both adjacency ends) and the
    /// dead fields are dropped. Returns the removed ids ascending.
    pub(crate) fn remove_links_incident(&mut self, p: PortalId) -> NeutralizeCascade {
        let incident: Vec<LinkId> = self
            .links
            .keys()
            .copied()
            .filter(|link| link.contains(p))
            .collect();

        let mut dead: BTreeSet<FieldId> = BTreeSet::new();
        let empty = BTreeSet::new();
        for link in &incident {
            let neighbors_min = self.adjacency.get(&link.min).unwrap_or(&empty);
            let neighbors_max = self.adjacency.get(&link.max).unwrap_or(&empty);
            for c in neighbors_min.intersection(neighbors_max) {
                if let Some(field_id) = FieldId::new(link.min, link.max, *c) {
                    dead.insert(field_id);
                }
            }
        }

        for link in &incident {
            self.links.remove(link);
            if let Some(set) = self.adjacency.get_mut(&link.min) {
                set.remove(&link.max);
            }
            if let Some(set) = self.adjacency.get_mut(&link.max) {
                set.remove(&link.min);
            }
        }
        for field_id in &dead {
            self.fields.remove(field_id);
        }

        NeutralizeCascade {
            links: incident,
            fields: dead.into_iter().collect(),
        }
    }

    /// Diagnostic full recompute of every triangle in the graph — for each
    /// link `(a, b)` with `a < b`, every common neighbor `c > b`, so each
    /// triple is emitted exactly once (by its two smallest vertices' edge).
    /// NEVER used by settlement; it exists so P1-FIELD can cross-check the
    /// incrementally maintained cache against a from-scratch derivation.
    pub fn fields_full_recompute<F: Fn(PortalId) -> Coord>(
        &self,
        position: F,
    ) -> BTreeMap<FieldId, Field> {
        let mut out = BTreeMap::new();
        let empty = BTreeSet::new();
        for (&link, &Link { faction }) in self.links.iter() {
            let neighbors_min = self.adjacency.get(&link.min).unwrap_or(&empty);
            let neighbors_max = self.adjacency.get(&link.max).unwrap_or(&empty);
            for &c in neighbors_min.intersection(neighbors_max) {
                if c > link.max {
                    let field_id = FieldId::new(link.min, link.max, c)
                        .expect("a neighbor is never an endpoint");
                    let area2 = area2(
                        position(field_id.a),
                        position(field_id.b),
                        position(field_id.c),
                    );
                    out.insert(field_id, Field { faction, area2 });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Portal coordinates for the fixtures: a hand-placed grid dense enough
    /// for layered fields. Keyed by dense portal id.
    fn pos(p: PortalId) -> Coord {
        match p.0 {
            0 => Coord::new(4_000, 4_000),
            1 => Coord::new(20_000, 4_000),
            2 => Coord::new(4_000, 20_000),
            3 => Coord::new(20_000, 20_000),
            4 => Coord::new(12_000, 12_000),
            5 => Coord::new(36_000, 4_000),
            _ => panic!("unknown fixture portal {p:?}"),
        }
    }

    fn pid(n: u32) -> PortalId {
        PortalId(n)
    }

    fn link(a: u32, b: u32) -> LinkId {
        LinkId::new(pid(a), pid(b)).expect("distinct endpoints")
    }

    fn field(a: u32, b: u32, c: u32) -> FieldId {
        FieldId::new(pid(a), pid(b), pid(c)).expect("distinct vertices")
    }

    // Checklist 4: fresh invariants.
    #[test]
    fn empty_graph_has_no_links_fields_or_neighbors() {
        let graph = LinkGraph::default();
        assert!(graph.links().is_empty());
        assert!(graph.fields().is_empty());
        assert!(graph.neighbors(pid(0)).is_empty());
        assert!(graph.link(link(0, 1)).is_none());
        assert!(graph.fields_full_recompute(pos).is_empty());
    }

    // Checklist 5: insertion keeps the edge map and BOTH adjacency ends in
    // lockstep, symmetric at both endpoints.
    #[test]
    fn insert_link_updates_links_and_both_adjacency_ends() {
        let mut graph = LinkGraph::default();
        assert_eq!(graph.insert_link(link(0, 1), Faction::Enl, pos), Vec::new());
        assert_eq!(
            graph.link(link(0, 1)).map(|l| l.faction),
            Some(Faction::Enl)
        );
        assert_eq!(graph.neighbors(pid(0)), BTreeSet::from([pid(1)]));
        assert_eq!(graph.neighbors(pid(1)), BTreeSet::from([pid(0)]));
        assert_eq!(graph.neighbors(pid(2)).len(), 0);
        assert!(graph.fields().is_empty(), "one edge closes no triangle");

        graph.insert_link(link(0, 3), Faction::Enl, pos);
        assert_eq!(graph.neighbors(pid(0)), BTreeSet::from([pid(1), pid(3)]));
        assert_eq!(graph.neighbors(pid(3)), BTreeSet::from([pid(0)]));
    }

    // Checklist 6: three edges close exactly one field with the exact
    // hand-computed area2 (right triangle with legs 16_000: 2 * (16_000²/2)).
    #[test]
    fn three_links_derive_one_field_with_exact_area2() {
        let mut graph = LinkGraph::default();
        assert_eq!(
            graph.insert_link(link(0, 1), Faction::Enl, pos),
            Vec::new(),
            "edge 0-1 closes nothing"
        );
        assert_eq!(
            graph.insert_link(link(0, 2), Faction::Enl, pos),
            Vec::new(),
            "edge 0-2 closes nothing"
        );
        let derived = graph.insert_link(link(1, 2), Faction::Enl, pos);
        assert_eq!(derived, vec![field(0, 1, 2)]);
        let cached = graph.fields().get(&field(0, 1, 2)).expect("field cached");
        assert_eq!(cached.faction, Faction::Enl);
        // cross((1)-(0), (2)-(0)) = (-16000)*(16000) - 0 = -2.56e9, abs:
        assert_eq!(cached.area2, 256_000_000u64);
        assert_eq!(
            cached.area2,
            area2(pos(pid(0)), pos(pid(1)), pos(pid(2))),
            "cached area2 equals the geometry recomputation"
        );
    }

    // Checklist 7: one link can close several layered fields at once
    // (R-FIELD-02): A-B with common neighbors C and D yields two fields.
    #[test]
    fn one_link_completes_multiple_layered_fields() {
        let mut graph = LinkGraph::default();
        // Star around portal 0: neighbors 1, 3, and 4 all link to 0.
        graph.insert_link(link(0, 1), Faction::Res, pos);
        graph.insert_link(link(0, 3), Faction::Res, pos);
        graph.insert_link(link(0, 4), Faction::Res, pos);
        assert!(graph.fields().is_empty());
        // Closing 1-3 completes fields (0,1,3) and — 4 is NOT adjacent to 1
        // or 3 yet, so exactly one field closes here.
        let derived = graph.insert_link(link(1, 3), Faction::Res, pos);
        assert_eq!(derived, vec![field(0, 1, 3)]);
        // Now 4-1 completes (0,1,4); the new edge also has common neighbor 3
        // with 1? No: adj(4)={0,1}, adj(1)={0,3,4}: common {0} -> one field.
        let derived = graph.insert_link(link(4, 1), Faction::Res, pos);
        assert_eq!(derived, vec![field(0, 1, 4)]);
        // Finally 3-4: common neighbors of 3 and 4 = {0, 1} -> TWO layered
        // fields close at once (0,3,4) and (1,3,4).
        let derived = graph.insert_link(link(3, 4), Faction::Res, pos);
        assert_eq!(derived, vec![field(0, 3, 4), field(1, 3, 4)]);
        assert_eq!(graph.fields().len(), 4);
        assert_eq!(
            graph.fields().keys().copied().collect::<Vec<_>>(),
            vec![
                field(0, 1, 3),
                field(0, 1, 4),
                field(0, 3, 4),
                field(1, 3, 4)
            ]
        );
    }

    // Checklist 8: field identity is independent of edge insertion order.
    #[test]
    fn field_identity_independent_of_link_creation_order() {
        let orders: [[(u32, u32); 3]; 6] = [
            [(0, 1), (1, 2), (2, 0)],
            [(0, 1), (2, 0), (1, 2)],
            [(1, 2), (0, 1), (2, 0)],
            [(1, 2), (2, 0), (0, 1)],
            [(2, 0), (0, 1), (1, 2)],
            [(2, 0), (1, 2), (0, 1)],
        ];
        for order in orders {
            let mut graph = LinkGraph::default();
            for &(a, b) in &order {
                graph.insert_link(link(a, b), Faction::Enl, pos);
            }
            assert_eq!(
                graph.fields().keys().copied().collect::<Vec<_>>(),
                vec![field(0, 1, 2)],
                "order {:?}",
                order
            );
            assert_eq!(graph.fields().len(), 1, "exactly one field, no duplicates");
        }
    }

    // Checklist 9: neutralizing one portal kills exactly the incident links
    // and the fields on them; disjoint structures survive.
    #[test]
    fn remove_links_incident_kills_only_affected_fields() {
        let mut graph = LinkGraph::default();
        for &(a, b) in &[(0u32, 1u32), (0, 3), (1, 3), (3, 4), (0, 4), (0, 2), (2, 5)] {
            graph.insert_link(link(a, b), Faction::Enl, pos);
        }
        // Closed triangles: (0,1,3) and (0,3,4). The pairs (0,2) and (2,5)
        // close nothing (2 has no adjacent pair), and no edge 1-4 exists, so
        // (0,1,4) is NOT a field.
        let before: Vec<FieldId> = graph.fields().keys().copied().collect();
        assert_eq!(before, vec![field(0, 1, 3), field(0, 3, 4)]);

        let cascade = graph.remove_links_incident(pid(3));
        // Incident links ascending; both cached fields had 3 as a vertex and
        // die with them.
        assert_eq!(cascade.links, vec![link(0, 3), link(1, 3), link(3, 4)]);
        assert_eq!(cascade.fields, vec![field(0, 1, 3), field(0, 3, 4)]);
        assert!(graph.fields().is_empty());
        // The untouched edges survive with intact adjacency.
        for &(a, b) in &[(0u32, 1u32), (0, 4), (0, 2), (2, 5)] {
            assert!(graph.link(link(a, b)).is_some(), "edge {a}-{b} survives");
        }
        assert_eq!(
            graph.neighbors(pid(0)),
            BTreeSet::from([pid(1), pid(2), pid(4)])
        );
        assert_eq!(graph.neighbors(pid(3)), BTreeSet::new());
        assert!(
            graph.fields_full_recompute(pos).is_empty(),
            "no triangle remains"
        );

        // A portal with no links cascades nothing.
        let cascade = graph.remove_links_incident(pid(9));
        assert!(cascade.links.is_empty());
        assert!(cascade.fields.is_empty());
        assert_eq!(graph.links().len(), 4, "unrelated cascade removes nothing");
    }

    // Checklist 10 (P1-FIELD): the incrementally maintained cache equals the
    // full recompute after every step of a mixed create/neutralize script.
    #[test]
    fn incremental_cache_matches_full_recompute_after_mixed_ops() {
        let mut graph = LinkGraph::default();
        let mut rng_state: u64 = 0x5DEE_CE66; // deterministic script driver
        let mut next = move || {
            // xorshift on a u64; generator-internal wrapping is fine here.
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            rng_state
        };
        let mut linked: Vec<LinkId> = Vec::new();
        for _ in 0..50 {
            let choice = next() % 10;
            if choice < 7 || linked.len() < 3 {
                // Insert a random edge over portals 0..=5 (K6 space). One
                // faction throughout: the D40 invariant guarantees a
                // triangle's three links share the endpoints' owner, so a
                // mixed-faction edge set is not a reachable game state.
                let a = (next() % 6) as u32;
                let b = (next() % 6) as u32;
                if let Some(id) = LinkId::new(pid(a), pid(b)) {
                    if graph.link(id).is_none() {
                        graph.insert_link(id, Faction::Enl, pos);
                        linked.push(id);
                    }
                }
            } else {
                // Neutralize a random portal.
                let p = pid((next() % 6) as u32);
                let cascade = graph.remove_links_incident(p);
                linked.retain(|l| !cascade.links.contains(l));
            }
            assert_eq!(
                *graph.fields(),
                graph.fields_full_recompute(pos),
                "cache must equal recompute after every step"
            );
        }
        assert!(!graph.links().is_empty(), "script exercised insertions");
    }

    // Checklist 11: the recompute emits every triple exactly once — a K4
    // clique has C(4,3) = 4 triangles regardless of insertion order.
    #[test]
    fn recompute_counts_each_triple_once() {
        for order in [
            vec![(0u32, 1u32), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)],
            vec![(2, 3), (1, 3), (1, 2), (0, 3), (0, 2), (0, 1)],
            vec![(0, 1), (2, 3), (0, 2), (1, 3), (0, 3), (1, 2)],
        ] {
            let mut graph = LinkGraph::default();
            for &(a, b) in &order {
                graph.insert_link(link(a, b), Faction::Enl, pos);
            }
            let cached: Vec<FieldId> = graph.fields().keys().copied().collect();
            assert_eq!(
                cached,
                vec![
                    field(0, 1, 2),
                    field(0, 1, 3),
                    field(0, 2, 3),
                    field(1, 2, 3)
                ],
                "order {order:?}"
            );
            let recomputed = graph.fields_full_recompute(pos);
            assert_eq!(recomputed.len(), 4, "K4 has exactly 4 triangles");
            assert_eq!(*graph.fields(), recomputed);
            for (id, f) in recomputed.iter() {
                assert_eq!(f.area2, area2(pos(id.a), pos(id.b), pos(id.c)));
            }
        }
    }
}
