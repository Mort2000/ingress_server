use arena_core::{config::GameConfig, types::Coord};
use arena_runner::tournament::maps::{Layout, MapAsset};

#[test]
fn manual_map_roundtrip_and_structure_are_public_and_exact() {
    let cfg = GameConfig {
        portal_count: 3,
        portal_cluster_count: 1,
        portal_min_distance_cm: 1,
        max_link_distance_cm: 100,
        ..GameConfig::default()
    };
    let asset = MapAsset::manual(
        cfg,
        vec![Coord::new(0, 0), Coord::new(30, 0), Coord::new(0, 40)],
    )
    .unwrap();
    let decoded = MapAsset::from_json(&asset.to_json()).unwrap();
    assert_eq!(decoded.map, asset.map);
    assert_eq!(decoded.config, cfg);
    let summary = decoded.summary();
    assert_eq!(summary.edges, 3);
    assert_eq!(summary.triangles, 1);
    assert_eq!(summary.components, 1);
    assert!(summary.warnings.is_empty());
    assert!(
        MapAsset::manual(cfg, vec![Coord::new(0, 0); 3])
            .unwrap_err()
            .to_string()
            .contains("distance")
    );
}

#[test]
fn all_six_layouts_are_frozen_valid_and_symmetric_is_exact() {
    let cfg = GameConfig {
        portal_count: 12,
        ..GameConfig::default()
    };
    for layout in [
        Layout::Uniform,
        Layout::DenseClusters,
        Layout::SparseClusters,
        Layout::Corridor,
        Layout::EdgeAnchors,
        Layout::Symmetric,
    ] {
        let asset = MapAsset::generate(cfg, layout, 17).unwrap();
        asset.validate().unwrap();
        assert_eq!(
            asset.to_json(),
            MapAsset::generate(cfg, layout, 17).unwrap().to_json()
        );
        assert_eq!(
            MapAsset::from_json(&asset.to_json()).unwrap().map,
            asset.map
        );
        if matches!(layout, Layout::Symmetric) {
            for p in asset.map.portals() {
                assert!(
                    asset
                        .map
                        .portals()
                        .iter()
                        .any(|q| q.position.x == cfg.map_width_cm - p.position.x
                            && q.position.y == cfg.map_height_cm - p.position.y)
                );
            }
        }
    }
}
