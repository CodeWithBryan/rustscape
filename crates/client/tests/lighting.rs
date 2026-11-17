use rs09_client::{model::Model, shadow, texture};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn input(name: &str) -> Vec<u8> {
    std::fs::read(root().join("fixtures/visual").join(name)).unwrap()
}
fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
#[test]
fn projected_scenery_shadow_masks_match_original_gl_models() {
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let materials = texture::materials(&input("materials.bin")).unwrap();
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "shadow")
    {
        let model = Model::decode(&input(c["file"].as_str().unwrap())).unwrap();
        let light = std::array::from_fn(|i| c["light"][i].as_i64().unwrap() as i32);
        let mut bytes = Vec::new();
        if let Some(sprite) = shadow::project(&model, &materials, light) {
            for value in [
                sprite.offset[0],
                sprite.offset[1],
                sprite.width as i32,
                sprite.height as i32,
            ] {
                bytes.extend(value.to_be_bytes());
            }
            bytes.extend(sprite.pixels);
        }
        assert_eq!(hash(&bytes), c["expected"].as_str().unwrap(), "{}", c["id"]);
    }
}
#[test]
fn shadow_texture_alpha_and_edges_match_original() {
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "shadow_filter")
    {
        let mut map = shadow::Map::new(2, 2);
        map.pixels = input(c["file"].as_str().unwrap());
        let bytes: Vec<_> = map
            .updates()
            .into_iter()
            .flat_map(|(_, rgba)| rgba.into_iter().skip(3).step_by(4))
            .collect();
        assert_eq!(
            hash(&bytes),
            c["expected"].as_str().unwrap(),
            "{}",
            c["file"]
        );
        assert!(map.updates().is_empty());
    }
}

#[test]
fn character_shadow_geometry_and_animation_match_original() {
    use rs09_client::animation::{Base, Frame, animate_hd};
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let materials = texture::materials(&input("materials.bin")).unwrap();
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "character")
    {
        count += 1;
        let parts: Vec<_> = c["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| Model::decode(&input(f.as_str().unwrap())).unwrap())
            .collect();
        let mut model = Model::combine(&parts);
        model.sort_by_priority = true;
        model.surfaces = rs09_client::surface::prepare(&model, &materials);
        let base = Base::decode(
            c["base_id"].as_u64().unwrap() as u16,
            &input(c["base"].as_str().unwrap()),
        )
        .unwrap();
        let first = Frame::decode(&input(c["first"].as_str().unwrap()), &base).unwrap();
        let next = Frame::decode(&input(c["next"].as_str().unwrap()), &base).unwrap();
        let pose = animate_hd(
            &model,
            &base,
            &first,
            Some(&next),
            c["delta"].as_i64().unwrap() as i32,
            c["length"].as_i64().unwrap() as i32,
            true,
        );
        let shadow = shadow::character(
            &pose,
            std::array::from_fn(|i| c["fine"][i].as_i64().unwrap() as i32),
            c["angle"].as_i64().unwrap() as i32,
            c["stretched"].as_bool().unwrap(),
            Some((&base, &first)),
        );
        let mut bytes = Vec::new();
        for v in &shadow.vertices {
            for n in v {
                bytes.extend(n.to_be_bytes());
            }
        }
        for (color, alpha) in shadow.colors.iter().zip(shadow.alpha) {
            bytes.extend(color.to_be_bytes());
            bytes.push(alpha);
        }
        assert_eq!(hash(&bytes), c["expected"].as_str().unwrap(), "{}", c["id"]);
    }
    assert_eq!(count, 594);
}

#[test]
fn point_light_decoding_flicker_and_floor_colors_match_original() {
    use rs09_client::lighting::{self, Flicker, Light};
    let noise = lighting::noise();
    let bytes: Vec<_> = noise.iter().flat_map(|v| v.to_be_bytes()).collect();
    assert_eq!(bytes, input("light-noise.bin"));
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let palette = rs09_client::surface::palette(0.7);
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "light")
    {
        count += 1;
        let light = Light::decode(&input(c["file"].as_str().unwrap()), &palette, |_| {
            Ok(Flicker {
                kind: 5,
                speed: 3000,
                min: 4096,
                max: -1000,
            })
        })
        .unwrap();
        let mut bytes = Vec::new();
        for value in [
            light.level as i32,
            i32::from(light.extends_down),
            i32::from(light.extends_up),
            light.coordinates[0],
            light.coordinates[1],
            light.coordinates[2],
            light.radius,
            light.color as i32,
            light.preset,
            light.phase,
            light.flicker.kind,
            light.flicker.speed,
            light.flicker.min,
            light.flicker.max,
        ] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(light.quadratic_attenuation.to_bits().to_be_bytes());
        for span in &light.spans {
            bytes.extend(span.to_be_bytes());
        }
        for disable in [false, true] {
            for tick in [
                0,
                1,
                2,
                3,
                7,
                15,
                31,
                50,
                100,
                199,
                200,
                511,
                1023,
                2048,
                -1,
                -50,
                i32::MAX,
                i32::MIN,
            ] {
                let sample = light.sample(tick, disable, noise);
                for value in [
                    sample.alpha,
                    sample.diffuse[0],
                    sample.diffuse[1],
                    sample.diffuse[2],
                    0.0,
                ] {
                    bytes.extend(value.to_bits().to_be_bytes());
                }
            }
        }
        for i in 0..32 {
            let mut position = [
                light.coordinates[0] + (i % 8 - 3) * 70,
                light.coordinates[1] + (i % 5 - 2) * 50,
                light.coordinates[2] + (i % 3 - 1) * 140,
            ];
            if i == 31 {
                position = light.coordinates;
            }
            bytes.extend(light.floor_color(position, [0.0, -1.0, 0.0]));
        }
        assert_eq!(
            hash(&bytes),
            c["expected"].as_str().unwrap(),
            "{}",
            c["file"]
        );
    }
    assert_eq!(count, 334);
}

#[test]
fn light_grid_spans_plane_flags_and_four_slot_limit_match_original() {
    use rs09_client::lighting::{self, Flicker, Light};
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let palette = rs09_client::surface::palette(0.7);
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "lightgrid")
    {
        count += 1;
        let pattern = c["pattern"].as_i64().unwrap();
        let lights: Vec<_> = c["files"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let mut light = Light::decode(&input(file.as_str().unwrap()), &palette, |_| {
                    Ok(Flicker::default())
                })
                .unwrap();
                let center = match pattern {
                    1 => 0,
                    2 => 103,
                    4 if i < 255 => 1,
                    _ => 50,
                };
                light.coordinates[0] = center * 128 + (i % 3) as i32 * 32;
                light.coordinates[1] = center * 128 + (i % 5) as i32 * 40;
                light
            })
            .collect();
        let grid = lighting::grid(104, 104, &lights);
        let mut bytes = Vec::new();
        for plane in 0..4 {
            for x in 0..104 {
                for y in 0..104 {
                    bytes.extend(
                        u32::from_le_bytes(grid[(plane * 104 + y) * 104 + x]).to_be_bytes(),
                    );
                }
            }
        }
        assert_eq!(hash(&bytes), c["expected"].as_str().unwrap(), "{}", c["id"]);
    }
    assert_eq!(count, 5);
}

#[test]
fn shadow_stamp_clipping_overlap_wrapping_and_removal_match_original() {
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let sprite = shadow::Sprite {
        width: 17,
        height: 23,
        offset: [-7, -12],
        pixels: (0..17 * 23).map(|i| u8::from(i % 3 != 0)).collect(),
    };
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "shadow_stamp")
    {
        count += 1;
        let pattern = c["pattern"].as_u64().unwrap() as usize;
        let mut map = shadow::Map::new(2, 2);
        map.pixels = input(c["file"].as_str().unwrap());
        let position = [
            [1, 1],
            [257, 257],
            [0, 130],
            [100, 0],
            [128, 128],
            [250, 250],
            [-10, 90],
            [50, 256],
        ][pattern]
            .map(|n| n - 1);
        for _ in 0..260 {
            map.stamp(&sprite, position, false);
        }
        for _ in 0..3 {
            map.stamp(&sprite, position, true);
        }
        assert_eq!(
            hash(&map.pixels),
            c["expected"].as_str().unwrap(),
            "pattern {pattern}"
        );
    }
    assert_eq!(count, 8);
}

#[test]
fn pinned_map_light_coordinate_overwrite_matches_original_loader() {
    use rs09_client::lighting::{self, Flicker, Light};
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let palette = rs09_client::surface::palette(0.7);
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "lightplace")
    {
        count += 1;
        let light = Light::decode(&input(c["file"].as_str().unwrap()), &palette, |_| {
            Ok(Flicker::default())
        })
        .unwrap();
        let region = std::array::from_fn(|i| c["region"][i].as_i64().unwrap() as i32 * 64);
        let placed = lighting::pinned_position(
            light.coordinates,
            region,
            [3168; 2],
            c["ground"].as_i64().unwrap() as i32,
        );
        let mut bytes = i32::from(placed.is_some()).to_be_bytes().to_vec();
        if let Some(position) = placed {
            for v in position {
                bytes.extend(v.to_be_bytes());
            }
        }
        assert_eq!(
            hash(&bytes),
            c["expected"].as_str().unwrap(),
            "{} {}",
            c["file"],
            c["ground"]
        );
    }
    assert_eq!(count, 234);
}

#[test]
fn shadow_depth_bias_matches_original_projection_coefficients() {
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "depthbias")
    {
        count += 1;
        let coefficients = shadow::depth_bias(
            c["near"].as_f64().unwrap() as f32,
            c["far"].as_f64().unwrap() as f32,
            c["distance"].as_f64().unwrap() as f32,
            c["bias"].as_f64().unwrap() as f32,
        );
        let bytes: Vec<_> = coefficients
            .into_iter()
            .flat_map(|v| v.to_bits().to_be_bytes())
            .collect();
        assert_eq!(hash(&bytes), c["expected"].as_str().unwrap(), "{}", c["id"]);
    }
    assert_eq!(count, 24);
}
