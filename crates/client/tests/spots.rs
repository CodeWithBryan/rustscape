use rs09_client::{
    animation::{Base, Frame, Pose, Sequence},
    model::Model,
    spot::{Alignment, Definition},
    surface, texture,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn folder() -> PathBuf {
    root().join("fixtures/players/spots")
}
fn fixture() -> Value {
    serde_json::from_slice(&std::fs::read(folder().join("oracle.json")).unwrap()).unwrap()
}
fn input(name: &str) -> Vec<u8> {
    std::fs::read(folder().join(name)).unwrap()
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn number(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn spot_original_sources_and_cache_inputs_remain_pinned() {
    let f = fixture();
    for (name, expected) in f["sources"].as_object().unwrap() {
        let b = std::fs::read(
            root()
                .join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap();
        let b = String::from_utf8(b).unwrap().replace("\r\n", "\n");
        assert_eq!(sha(b.as_bytes()), expected.as_str().unwrap(), "{name}");
    }
    for (name, expected) in f["inputs"].as_object().unwrap() {
        assert_eq!(sha(&input(name)), expected.as_str().unwrap(), "{name}");
    }
}
#[test]
fn all_cached_spot_definitions_and_original_decode_quirks_match() {
    let f = fixture();
    assert_eq!(f["decoded"].as_array().unwrap().len(), 2249);
    for c in f["decoded"].as_array().unwrap() {
        let (d, position) = Definition::decode(&hex(c["raw"].as_str().unwrap())).unwrap();
        assert_eq!(
            json!({"definition":d,"position":position}),
            c["expected"],
            "spot {}",
            c["id"]
        );
    }
}
#[test]
fn original_cached_spot_sequence_sound_cues_match() {
    let f = fixture();
    for c in f["cues"].as_array().unwrap() {
        let s = Sequence::decode(&input(c["file"].as_str().unwrap())).unwrap();
        let cues: Vec<_> = s
            .sounds
            .iter()
            .enumerate()
            .filter(|(_, sounds)| !sounds.is_empty())
            .map(|(frame, sounds)| json!([frame, sounds]))
            .collect();
        assert_eq!(json!(cues), c["expected"], "spot {}", c["spot"]);
    }
}
#[test]
fn original_actor_terrain_samples_angles_offsets_and_call_order_match() {
    let f = fixture();
    assert_eq!(f["alignments"].as_array().unwrap().len(), 512);
    for (i, c) in f["alignments"].as_array().unwrap().iter().enumerate() {
        let mut calls = vec![];
        let mut index = 0;
        let a = Alignment::sample(
            [number(&c["dimensions"][0]), number(&c["dimensions"][1])],
            number(&c["yaw"]),
            [number(&c["fine"][0]), number(&c["fine"][1])],
            number(&c["tile_height"]),
            |fine| {
                calls.push([2, fine[0], fine[1]]);
                let h = number(&c["heights"][index]);
                index += 1;
                h
            },
        );
        assert_eq!(
            json!({"alignment":a,"calls":calls}),
            c["expected"],
            "terrain {i}"
        );
    }
}
#[test]
fn original_spot_default_resize_guard_and_wrapping_extreme_transforms_match() {
    let f = fixture();
    assert_eq!(f["primitives"].as_array().unwrap().len(), 36);
    for c in f["primitives"].as_array().unwrap() {
        let (d, _) = Definition::decode(&hex(c["raw"].as_str().unwrap())).unwrap();
        let vertices: Vec<[i32; 3]> = serde_json::from_value(c["vertices"].clone()).unwrap();
        let model = Model {
            vertices: vertices.clone(),
            ..Model::default()
        };
        assert_eq!(json!(d.pose(&model, None).vertices), c["expected"]);
        assert_eq!(model.vertices, vertices);
    }
}
#[test]
fn original_spot_gl_poses_scaling_turns_normals_alpha_and_actor_attachments_match() {
    let f = fixture();
    let materials = texture::materials(&input("materials.bin")).unwrap();
    let mut models = BTreeMap::new();
    for name in f["inputs"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|n| n.starts_with("model_"))
    {
        let id = name
            .strip_prefix("model_")
            .unwrap()
            .strip_suffix(".bin")
            .unwrap()
            .parse()
            .unwrap();
        models.insert(id, Model::decode(&input(name)).unwrap());
    }
    let mut bodies = BTreeMap::new();
    for c in f["cases"].as_array().unwrap() {
        let file = c["file"].as_str().unwrap();
        if !bodies.contains_key(file) {
            let (d, _) = Definition::decode(&input(file)).unwrap();
            let mut m = d.assemble(&models).unwrap();
            m.surfaces = surface::prepare(&m, &materials);
            m.draw_order = surface::face_order(&m, &materials);
            bodies.insert(file.to_owned(), (d, m));
        }
        let (d, template) = &bodies[file];
        let seq = Sequence::decode(&input(c["seq"].as_str().unwrap())).unwrap();
        let frame = number(&c["frame"]);
        let mut pose = if frame < 0 {
            d.pose(template, None)
        } else {
            let first_id = number(&c["first_base"]) as u16;
            let base = Base::decode(first_id, &input(&format!("base_{first_id}.bin"))).unwrap();
            let first = Frame::decode(&input(c["first"].as_str().unwrap()), &base).unwrap();
            let next_base = if number(&c["second_base"]) >= 0 {
                let id = number(&c["second_base"]) as u16;
                Some(Base::decode(id, &input(&format!("base_{id}.bin"))).unwrap())
            } else {
                None
            };
            let next = next_base
                .as_ref()
                .map(|b| Frame::decode(&input(c["second"].as_str().unwrap()), b).unwrap());
            d.pose(
                template,
                Some((
                    &seq,
                    Pose {
                        base: &base,
                        frame: &first,
                        next: if c["tween"].as_bool().unwrap() || seq.tween {
                            next.as_ref()
                        } else {
                            None
                        },
                        delta: number(&c["delta"]),
                        length: i32::from(seq.delays[frame as usize]),
                    },
                )),
            )
        };
        d.attach(
            &mut pose,
            number(&c["height"]),
            Alignment {
                pitch: number(&c["terrain"][0]),
                roll: number(&c["terrain"][1]),
                offset: number(&c["terrain"][2]),
            },
        );
        let mut bytes = vec![];
        bytes.extend((pose.vertices.len() as i32).to_be_bytes());
        for v in &pose.vertices {
            for n in v {
                bytes.extend(n.to_be_bytes());
            }
        }
        for (i, s) in pose.surfaces.iter().enumerate() {
            if surface::visible(&pose, i, &materials) {
                for normal in s.normals {
                    for n in normal {
                        bytes.extend(n.to_be_bytes());
                    }
                }
                bytes.extend(pose.colors[i].to_be_bytes());
                bytes.push(pose.alpha[i]);
            }
        }
        assert_eq!(
            sha(&bytes),
            c["expected"].as_str().unwrap(),
            "spot {}",
            c["id"]
        );
        assert_eq!(template.lighting, [d.ambient + 64, d.contrast + 850]);
    }
}
