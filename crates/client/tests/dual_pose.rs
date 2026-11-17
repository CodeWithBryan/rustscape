use rs09_client::{
    animation::{Base, Frame, Pose, animate_dual_hd},
    model::Model,
    surface, texture,
};
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
fn dual_action_movement_masks_match_original_gl_vertices_normals_colors_and_alpha() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/blend.json")).unwrap();
    for (name, hash) in fixture["inputs"].as_object().unwrap() {
        assert_eq!(
            format!("{:x}", Sha256::digest(input(name))),
            hash.as_str().unwrap()
        );
    }
    let materials = texture::materials(&input("materials.bin")).unwrap();
    for (i, c) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let a = &c["first"];
        let b = &c["second"];
        let parts: Vec<_> = a["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| Model::decode(&input(s.as_str().unwrap())).unwrap())
            .collect();
        let mut model = Model::combine(&parts);
        model.sort_by_priority = true;
        model.surfaces = surface::prepare(&model, &materials);
        let base = Base::decode(
            a["base_id"].as_u64().unwrap() as u16,
            &input(a["base"].as_str().unwrap()),
        )
        .unwrap();
        let first = Frame::decode(&input(a["first"].as_str().unwrap()), &base).unwrap();
        let first_next = Frame::decode(&input(a["next"].as_str().unwrap()), &base).unwrap();
        let second = Frame::decode(&input(b["first"].as_str().unwrap()), &base).unwrap();
        let second_next = Frame::decode(&input(b["next"].as_str().unwrap()), &base).unwrap();
        let mask = c["mask"]
            .as_array()
            .map(|m| m.iter().map(|b| b.as_bool().unwrap()).collect::<Vec<_>>());
        let pose = animate_dual_hd(
            &model,
            Pose {
                base: &base,
                frame: &first,
                next: Some(&first_next),
                delta: a["delta"].as_i64().unwrap() as i32,
                length: a["length"].as_i64().unwrap() as i32,
            },
            Some(Pose {
                base: &base,
                frame: &second,
                next: Some(&second_next),
                delta: b["delta"].as_i64().unwrap() as i32,
                length: b["length"].as_i64().unwrap() as i32,
            }),
            mask.as_deref(),
            c["normals"].as_bool().unwrap(),
        );
        let mut bytes = vec![];
        for vertex in &pose.vertices {
            for n in vertex {
                bytes.extend(n.to_be_bytes());
            }
        }
        for (face, s) in pose.surfaces.iter().enumerate() {
            if surface::visible(&pose, face, &materials) {
                for normal in s.normals {
                    for n in normal {
                        bytes.extend(n.to_be_bytes());
                    }
                }
            }
        }
        for face in 0..pose.faces.len() {
            if surface::visible(&pose, face, &materials) {
                bytes.extend(pose.colors[face].to_be_bytes());
                bytes.push(pose.alpha[face]);
            }
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            c["expected"].as_str().unwrap(),
            "dual pose{i}"
        );
    }
}
