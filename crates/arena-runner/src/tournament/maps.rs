//! Portable versioned map assets, outside the authoritative runtime RNG.
use super::{
    archive::{Json, parse_json},
    document::*,
    schedule::map_codec_hash,
};
use arena_core::{
    config::GameConfig,
    geometry::{cross, dist2, point_on_segment},
    map::MapDefinition,
    types::{Coord, Owner, Portal, PortalId},
};
use std::io;

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    Uniform,
    DenseClusters,
    SparseClusters,
    Corridor,
    EdgeAnchors,
    Symmetric,
}
impl Layout {
    pub fn parse(s: &str) -> io::Result<Self> {
        match s {
            "uniform" => Ok(Self::Uniform),
            "dense-clusters" => Ok(Self::DenseClusters),
            "sparse-clusters" => Ok(Self::SparseClusters),
            "corridor" => Ok(Self::Corridor),
            "edge-anchors" => Ok(Self::EdgeAnchors),
            "symmetric" => Ok(Self::Symmetric),
            _ => Err(invalid("unknown layout")),
        }
    }
    pub fn tag(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::DenseClusters => "dense-clusters",
            Self::SparseClusters => "sparse-clusters",
            Self::Corridor => "corridor",
            Self::EdgeAnchors => "edge-anchors",
            Self::Symmetric => "symmetric",
        }
    }
}
#[derive(Clone, Debug)]
pub struct MapAsset {
    pub config: GameConfig,
    pub map: MapDefinition,
    pub source: Json,
    pub tags: Vec<String>,
}
#[derive(Debug)]
pub struct StructureSummary {
    pub edges: u64,
    pub triangles: u64,
    pub components: u32,
    pub warnings: Vec<String>,
}
impl MapAsset {
    pub fn generate(config: GameConfig, layout: Layout, seed: u64) -> io::Result<Self> {
        config
            .validate()
            .map_err(|e| invalid(format!("rules: {e:?}")))?;
        let mut h = blake3::Hasher::new();
        h.update(b"arena-layout-v1");
        h.update(layout.tag().as_bytes());
        h.update(&seed.to_le_bytes());
        let mut rng = h.finalize_xof();
        let mut draw = |max: i32| {
            let mut b = [0; 8];
            rng.fill(&mut b);
            (u64::from_le_bytes(b) % (max as u64 + 1)) as i32
        };
        let w = config.map_width_cm;
        let h = config.map_height_cm;
        let mut points = Vec::new();
        if matches!(layout, Layout::Symmetric) && config.portal_count % 2 == 1 {
            if w % 2 != 0 || h % 2 != 0 {
                return Err(invalid("odd symmetric count needs even dimensions"));
            }
            points.push(Coord::new(w / 2, h / 2));
        }
        for attempt in 0..100_000 {
            if points.len() == config.portal_count as usize {
                break;
            }
            let x = draw(w);
            let y = draw(h);
            let p = match layout {
                Layout::Uniform | Layout::Symmetric => Coord::new(x, y),
                Layout::Corridor => Coord::new(x, h / 2 + (y - h / 2) / 10),
                Layout::EdgeAnchors => match attempt % 4 {
                    0 => Coord::new(x, y / 10),
                    1 => Coord::new(x, h - y / 10),
                    2 => Coord::new(x / 10, y),
                    _ => Coord::new(w - x / 10, y),
                },
                Layout::DenseClusters | Layout::SparseClusters => {
                    let c = (points.len() % 3) as i32;
                    let scale = if matches!(layout, Layout::DenseClusters) {
                        10
                    } else {
                        4
                    };
                    Coord::new(
                        w * (c + 1) / 4 + (x - w / 2) / scale,
                        h * (if c == 1 { 3 } else { 1 }) / 4 + (y - h / 2) / scale,
                    )
                }
            };
            let mut candidates = vec![p];
            if matches!(layout, Layout::Symmetric) {
                candidates.push(Coord::new(w - p.x, h - p.y));
            }
            let min2 = (config.portal_min_distance_cm as i64).pow(2);
            if candidates
                .iter()
                .any(|p| points.iter().any(|&q| dist2(*p, q) < min2))
                || (candidates.len() == 2 && dist2(candidates[0], candidates[1]) < min2)
            {
                continue;
            }
            points.extend(candidates);
        }
        if points.len() != config.portal_count as usize {
            return Err(invalid(
                "layout infeasible within 100000 placement attempts; reduce count/distance or enlarge map",
            ));
        }
        let mut asset = Self::manual(config, points)?;
        asset.source = object(vec![
            ("kind", string("generated")),
            ("generator", string("arena-layout-v1")),
            ("layout", string(layout.tag())),
            ("seed", string(seed)),
        ]);
        asset.tags = vec![layout.tag().into()];
        Ok(asset)
    }
    pub fn manual(config: GameConfig, positions: Vec<Coord>) -> io::Result<Self> {
        let asset = Self {
            config,
            map: MapDefinition::new(
                positions
                    .into_iter()
                    .enumerate()
                    .map(|(i, position)| Portal {
                        id: PortalId(i as u32),
                        position,
                        owner: Owner::Neutral,
                        hp: 0,
                    })
                    .collect(),
            ),
            source: object(vec![("kind", string("manual"))]),
            tags: vec!["manual".into()],
        };
        asset.validate()?;
        Ok(asset)
    }
    pub fn validate(&self) -> io::Result<()> {
        self.config
            .validate()
            .map_err(|e| invalid(format!("rules: {e:?}")))?;
        if self.map.len() != self.config.portal_count as usize {
            return Err(invalid("portal_count does not match portals"));
        }
        for (i, p) in self.map.portals().iter().enumerate() {
            if p.id.0 != i as u32 || p.owner != Owner::Neutral || p.hp != 0 {
                return Err(invalid(format!(
                    "portal {i}: expected dense ID, neutral owner and zero hp"
                )));
            }
            if p.position.x < 0
                || p.position.y < 0
                || p.position.x > self.config.map_width_cm
                || p.position.y > self.config.map_height_cm
            {
                return Err(invalid(format!("portal {i}: out of bounds")));
            }
            for q in &self.map.portals()[..i] {
                if dist2(p.position, q.position)
                    < (self.config.portal_min_distance_cm as i64).pow(2)
                {
                    return Err(invalid(format!(
                        "portal {i} and {} violate minimum distance",
                        q.id.0
                    )));
                }
            }
        }
        Ok(())
    }
    pub fn from_json(s: &str) -> io::Result<Self> {
        let j = parse_json(s).map_err(invalid)?;
        keys(
            &j,
            &["schema", "config", "portals", "hash", "source", "tags"],
        )?;
        if text(&j, "schema")? != "arena-map-v1" {
            return Err(invalid("unsupported map schema"));
        }
        let cfg = config(j.get("config").ok_or_else(|| invalid("missing config"))?)?;
        let mut positions = Vec::new();
        for p in array(&j, "portals")? {
            let a = p
                .as_array()
                .filter(|a| a.len() == 2)
                .ok_or_else(|| invalid("portal must be [x,y]"))?;
            let coord = |i: usize| {
                a[i].as_int()
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or_else(|| invalid("coordinate must be i32"))
            };
            positions.push(Coord::new(coord(0)?, coord(1)?));
        }
        let mut asset = Self::manual(cfg, positions)?;
        if let Some(hash) = j.get("hash") {
            if hash.as_str() != Some(&map_codec_hash(&asset.map)) {
                return Err(invalid("map hash mismatch"));
            }
        }
        if let Some(source) = j.get("source") {
            asset.source = source.clone();
        }
        if let Some(tags) = j.get("tags") {
            asset.tags = tags
                .as_array()
                .ok_or_else(|| invalid("tags must be array"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("tag must be string"))
                })
                .collect::<io::Result<_>>()?;
        }
        Ok(asset)
    }
    pub fn to_json(&self) -> String {
        encode(&object(vec![
            ("schema", string("arena-map-v1")),
            ("config", config_json(&self.config)),
            ("hash", string(map_codec_hash(&self.map))),
            ("source", self.source.clone()),
            ("tags", Json::Array(self.tags.iter().map(string).collect())),
            (
                "portals",
                Json::Array(
                    self.map
                        .portals()
                        .iter()
                        .map(|p| {
                            Json::Array(vec![
                                Json::Int(p.position.x as i64),
                                Json::Int(p.position.y as i64),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }
    /// Potential straight Links: within range and not passing through another
    /// Portal. Triangles additionally require nonzero area. Ownership/keys and
    /// intersections with dynamically created Links are intentionally excluded.
    pub fn summary(&self) -> StructureSummary {
        let n = self.map.len();
        let ps = self.map.portals();
        let mut neighbors = vec![Vec::new(); n];
        let mut edges = 0;
        for i in 0..n {
            for j in i + 1..n {
                let a = ps[i].position;
                let b = ps[j].position;
                if dist2(a, b) > (self.config.max_link_distance_cm as i64).pow(2) {
                    continue;
                }
                if ps
                    .iter()
                    .enumerate()
                    .any(|(k, p)| k != i && k != j && point_on_segment(p.position, a, b))
                {
                    continue;
                }
                neighbors[i].push(j);
                neighbors[j].push(i);
                edges += 1;
            }
        }
        let mut triangles = 0;
        for i in 0..n {
            for &j in neighbors[i].iter().filter(|&&j| j > i) {
                for &k in neighbors[j].iter().filter(|&&k| k > j) {
                    if neighbors[i].binary_search(&k).is_ok()
                        && cross(ps[i].position, ps[j].position, ps[k].position) != 0
                    {
                        triangles += 1;
                    }
                }
            }
        }
        let mut seen = vec![false; n];
        let mut components = 0;
        for i in 0..n {
            if seen[i] {
                continue;
            }
            components += 1;
            let mut stack = vec![i];
            seen[i] = true;
            while let Some(j) = stack.pop() {
                for &k in &neighbors[j] {
                    if !seen[k] {
                        seen[k] = true;
                        stack.push(k);
                    }
                }
            }
        }
        let mut warnings = Vec::new();
        if triangles == 0 {
            warnings.push("no feasible triangles".into());
        }
        if components > 1 {
            warnings.push("disconnected potential-link graph".into());
        }
        StructureSummary {
            edges,
            triangles,
            components,
            warnings,
        }
    }
}
