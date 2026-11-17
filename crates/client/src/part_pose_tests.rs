use super::*;
use rs09_client::{actor_animation, surface, texture};
use serde_json::Value;
use sha2::{Digest, Sha256};
fn input(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/visual")
            .join(name),
    )
    .unwrap()
}
#[test]
fn actual_body_binding_preserves_original_slot_order_delay_basis_and_final_blending() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/parts.json")).unwrap();
    let materials = texture::materials(&input("materials.bin")).unwrap();
    let mut checked = 0;
    for (i, c) in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, c)| c["runtime"].as_bool() == Some(true))
    {
        let parts: Vec<_> = c["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().map(|s| Model::decode(&input(s)).unwrap()))
            .collect();
        let mut model = Model::combine_slots(&parts.iter().map(Option::as_ref).collect::<Vec<_>>());
        model.sort_by_priority = true;
        model.surfaces = surface::prepare(&model, &materials);
        model.part_matrices = vec![None; 12];
        let a = &c["first"];
        let b = &c["second"];
        let mut base = Base::decode(
            a["base_id"].as_u64().unwrap() as u16,
            &input(a["base"].as_str().unwrap()),
        )
        .unwrap();
        if let Some(parts) = c["parts"].as_array() {
            base.parts = parts.iter().map(|n| n.as_u64().unwrap() as u16).collect();
        }
        let frames: BTreeMap<_, _> = [
            a["first"].as_str().unwrap(),
            a["next"].as_str().unwrap(),
            b["first"].as_str().unwrap(),
            b["next"].as_str().unwrap(),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, name)| (10 + i as u32, Frame::decode(&input(name), &base).unwrap()))
        .collect();
        let normals = c["normals"].as_bool().unwrap();
        let seq = |first, length| Sequence {
            frames: vec![first, first + 1],
            delays: vec![length; 2],
            transform_normals: normals,
            ..Default::default()
        };
        let mut sequences = BTreeMap::from([
            (100, seq(12, b["length"].as_u64().unwrap() as u16)),
            (
                200,
                Sequence {
                    frame_group: c["mask"]
                        .as_array()
                        .map(|a| a.iter().map(|n| n.as_bool().unwrap()).collect()),
                    ..seq(10, a["length"].as_u64().unwrap() as u16)
                },
            ),
        ]);
        let mut state = actor_animation::State {
            action: actor_animation::Track {
                id: 200,
                frame: 0,
                next: 1,
                clock: a["delta"].as_i64().unwrap() as i32 + 1,
                ..Default::default()
            },
            ..Default::default()
        };
        for overlay in c["overlays"].as_array().unwrap() {
            let slot = overlay["slot"].as_u64().unwrap() as usize;
            sequences.insert(
                300 + slot as u32,
                seq(10, a["length"].as_u64().unwrap() as u16),
            );
            model.part_matrices[slot] = overlay["matrix"].as_array().map(|a| {
                a.iter()
                    .map(|n| n.as_i64().unwrap() as i32)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            });
            state.slots[slot] = Some(actor_animation::Track {
                id: 300 + slot as i32,
                frame: 0,
                next: 1,
                clock: a["delta"].as_i64().unwrap() as i32 + 1,
                delay: c["runtimeDelay"].as_i64().unwrap() as i32,
                ..Default::default()
            });
        }
        let bases = BTreeMap::from([(base.id, base)]);
        let animation = ActorPose {
            actor: (i % 2 == 0, i),
            sequence: 100,
            tick: 0,
            tween: true,
            cursor: Some((
                0,
                1,
                b["delta"].as_i64().unwrap() as i32,
                b["length"].as_i64().unwrap() as i32,
            )),
            state: Some(&state),
            idle: -1,
        };
        let (movement, action) = actor_poses(&sequences, &frames, &bases, animation);
        let slots = actor_slots(&sequences, &frames, &bases, animation);
        assert_eq!(
            slots.len(),
            c["overlays"].as_array().unwrap().len(),
            "delayed slot rendering body {i}"
        );
        let valid = std::cell::Cell::new(c["originValid"].as_bool().unwrap());
        let model = actor_model(&model, movement, action, 100, -1, &slots, &valid);
        let mut bytes = vec![];
        for v in &model.vertices {
            for n in v {
                bytes.extend(n.to_be_bytes());
            }
        }
        for (face, s) in model.surfaces.iter().enumerate() {
            if surface::visible(&model, face, &materials) {
                for normal in s.normals {
                    for n in normal {
                        bytes.extend(n.to_be_bytes());
                    }
                }
            }
        }
        for face in 0..model.faces.len() {
            if surface::visible(&model, face, &materials) {
                bytes.extend(model.colors[face].to_be_bytes());
                bytes.push(model.alpha[face]);
            }
        }
        bytes.push(u8::from(valid.get()));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            c["expected"]["poses"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .as_str()
                .unwrap(),
            "body binding {i}"
        );
        checked += 1;
    }
    assert_eq!(checked, 256);
}
