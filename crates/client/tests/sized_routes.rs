use rs09_client::{interaction::Approach, walking};
use rs09_game_core::collision::CollisionMap;
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/ui/sized-routes.json")).unwrap()
}
fn base() -> CollisionMap {
    let mut map = CollisionMap::new(104, 104).unwrap();
    for x in 1..99 {
        for y in 1..99 {
            map.set(x, y, 0).unwrap();
        }
    }
    map
}
fn n(c: &Value, k: &str, default: i32) -> i32 {
    c[k].as_i64().map_or(default, |n| n as i32)
}
fn pair(c: &Value, k: &str, default: [i32; 2]) -> [i32; 2] {
    if c[k].is_null() {
        default
    } else {
        serde_json::from_value(c[k].clone()).unwrap()
    }
}
fn map(c: &Value, base: &CollisionMap) -> CollisionMap {
    let mut map = base.clone();
    for row in c["cells"].as_array().unwrap() {
        let [x, y, flags]: [i32; 3] = serde_json::from_value(row.clone()).unwrap();
        map.set(x, y, flags).unwrap();
    }
    map
}
fn approach(c: &Value) -> Approach {
    Approach {
        tile: pair(c, "destination", [0; 2]),
        size: pair(c, "target_size", [0; 2]),
        shape: n(c, "shape", 0),
        angle: n(c, "angle", 0),
        blocked_sides: n(c, "blocked", 0),
    }
}
#[test]
fn collision_window_keeps_bridge_levels_and_unloaded_tiles_at_a_region_seam() {
    use rs09_client::terrain::{Region, Tile};
    use walking::{Navigation, Placement};

    let region = |x| Region {
        x,
        z: 50,
        tiles: vec![Tile::default(); 4 * 64 * 64],
        environment: Default::default(),
        point_lights: vec![],
        chunk_environments: vec![],
        occlusion: vec![],
        underwater: Default::default(),
    };
    let west = region(49);
    let mut east = region(50);
    east.tiles[4096 + 13].flags = 3;
    let placement = Placement {
        position: [3200, 3213],
        plane: 1,
        shape: 10,
        rotation: 0,
        size: [2, 1],
        walk: 2,
        projectiles: true,
    };
    let nav = Navigation::new(&[west, east], &[placement], [3168, 3168]);
    let map = &nav.maps[0];
    assert_eq!(map.get(31, 45).unwrap() & 0x1000000, 0);
    assert_eq!(map.get(32, 45).unwrap() & 0x200000, 0x200000);
    assert_eq!(map.get(33, 45).unwrap() & 0x100, 0x100);
    assert_eq!(nav.maps[1].get(32, 45).unwrap() & 0x200100, 0);
    assert_ne!(map.get(32, 20).unwrap() & 0x1000000, 0);
}

#[test]
fn original_size_dispatch_corner_masks_and_interior_edge_scans_match() {
    let f = fixture();
    let base = base();
    for (i, c) in f["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, c)| c["kind"] == "step")
    {
        let map = map(c, &base);
        let source = pair(c, "source", [0; 2]);
        let destination = pair(c, "destination", [0; 2]);
        assert_eq!(
            walking::can_step_sized(
                &map,
                source,
                [destination[0] - source[0], destination[1] - source[1]],
                n(c, "size", 1)
            ),
            c["expected"].as_bool().unwrap(),
            "step{i}: {c}"
        );
    }
}
#[test]
fn original_sized_wall_decoration_and_rectangle_reach_match() {
    let f = fixture();
    let base = base();
    for (i, c) in f["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, c)| c["kind"] == "reach")
    {
        assert_eq!(
            approach(c).reached_sized(&map(c, &base), pair(c, "source", [0; 2]), n(c, "size", 1)),
            c["expected"].as_bool().unwrap(),
            "reach{i}: {c}"
        );
    }
}
#[test]
fn original_sized_bfs_approximation_turn_compression_and_move_bytes_match() {
    let f = fixture();
    let base = base();
    let mut clipped = 0;
    for (i, c) in f["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, c)| c["kind"] == "route" || c["kind"] == "ground")
    {
        let route = if c["kind"] == "ground" {
            rs09_client::ground_interaction::route_sized(
                &map(c, &base),
                pair(c, "source", [0; 2]),
                pair(c, "destination", [0; 2]),
                n(c, "size", 1),
            )
        } else {
            walking::find_path_to_sized(
                &map(c, &base),
                pair(c, "source", [0; 2]),
                &approach(c),
                c["approximate"].as_bool().unwrap_or(false),
                n(c, "size", 1),
            )
        };
        let result=route.map(|r|{clipped+=usize::from(r.turns.len()>25);let bytes=r.move_packet(pair(c,"origin",[3152,3168]),c["run"].as_bool().unwrap_or(false),n(c,"move_type",0)==2);json!({"turns":r.turns,"approximate":r.approximate,"packet":bytes.iter().map(|b|format!("{b:02x}")).collect::<String>()})});
        assert_eq!(json!(result), c["expected"], "route{i}: {c}");
    }
    assert!(
        clipped > 0,
        "Original25-turn packet clamp needs an exercised route"
    );
}
#[test]
fn invalid_actor_footprints_and_outside_map_queries_are_rejected() {
    let map = base();
    for size in [-1, 0, 105, i32::MAX] {
        assert!(walking::find_path_sized(&map, [50, 50], [51, 51], true, size).is_none());
    }
    assert!(walking::find_path_sized(&map, [103, 50], [51, 51], true, 2).is_none());
    assert!(!walking::can_step_sized(&map, [-1, 50], [1, 0], 2));
}
