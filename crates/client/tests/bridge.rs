use rs09_client::terrain::{self, Region, Tile};
use serde_json::Value;

fn number(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

#[test]
fn original_floor_render_levels_preserve_force_ground_before_bridge() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/bridge.json")).unwrap();
    for (index, c) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            terrain::render_level(
                number(&c["plane"]) as usize,
                number(&c["flags"]) as u8,
                number(&c["bridge"]) as u8
            ),
            number(&c["expected"]) as usize,
            "original floor{index}"
        );
    }
}

#[test]
fn prepared_region_actor_heights_match_original_scene_interpolation_across_seams() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/ground-geometry.json")).unwrap();
    for origin in [[3200, 3392], [-64, -64]] {
        let mut regions: Vec<_> = (0..2)
            .flat_map(|x| {
                (0..2).map(move |z| Region {
                    x: origin[0] / 64 + x,
                    z: origin[1] / 64 + z,
                    tiles: vec![Tile::default(); 4 * 4096],
                    environment: Default::default(),
                    point_lights: Vec::new(),
                    chunk_environments: Vec::new(),
                    occlusion: Vec::new(),
                    underwater: Default::default(),
                })
            })
            .collect();
        let mut count = 0;
        for (index, c) in fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, c)| c["kind"] == "height")
        {
            let fine = [number(&c["point"][0]), number(&c["point"][1])];
            let [x, z] = fine.map(|n| n >> 7);
            if (0..104).contains(&x) && (0..104).contains(&z) {
                for plane in 0..4 {
                    for dz in 0..2 {
                        for dx in 0..2 {
                            let wx = origin[0] + x + dx;
                            let wz = origin[1] + z + dz;
                            let r = regions
                                .iter_mut()
                                .find(|r| r.x == wx.div_euclid(64) && r.z == wz.div_euclid(64))
                                .unwrap();
                            let tile = &mut r.tiles[plane * 4096
                                + wx.rem_euclid(64) as usize * 64
                                + wz.rem_euclid(64) as usize];
                            tile.height = number(&c["heights"][plane][(dx + dz * 2) as usize]);
                            tile.flags = number(&c["flags"]) as u8;
                        }
                    }
                }
            }
            let global = std::array::from_fn(|i| fine[i].wrapping_add(origin[i].wrapping_mul(128)));
            assert_eq!(
                terrain::scene_height(&regions, origin, number(&c["plane"]), global),
                number(&c["expected"]),
                "original scene height{index} origin{origin:?}"
            );
            count += 1;
        }
        assert_eq!(count, 1024);
    }
}
