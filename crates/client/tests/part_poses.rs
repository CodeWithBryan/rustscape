use rs09_client::{
    animation::{Base, Frame, Pose, animate_dual_hd_with_state, animate_masked_hd},
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
fn raw(model: &Model) -> String {
    let mut bytes = vec![];
    for count in [
        model.vertices.len(),
        model.faces.len(),
        model.texture_faces.len(),
    ] {
        bytes.extend((count as i32).to_be_bytes());
    }
    for (i, v) in model.vertices.iter().enumerate() {
        for n in v {
            bytes.extend(n.to_be_bytes());
        }
        bytes.extend(i32::from(model.vertex_labels[i]).to_be_bytes());
        bytes.extend(model.vertex_sources[i].to_be_bytes());
    }
    for (i, f) in model.faces.iter().enumerate() {
        for n in f {
            bytes.extend((*n as i32).to_be_bytes());
        }
        bytes.extend(model.face_sources[i].to_be_bytes());
    }
    for f in &model.texture_faces {
        for n in f {
            bytes.extend((*n as i32).to_be_bytes());
        }
    }
    format!("{:x}", Sha256::digest(bytes))
}
fn pose_hash(model: &Model, materials: &[Option<texture::Material>], valid: bool) -> String {
    let mut bytes = vec![];
    for v in &model.vertices {
        for n in v {
            bytes.extend(n.to_be_bytes());
        }
    }
    for (face, s) in model.surfaces.iter().enumerate() {
        if surface::visible(model, face, materials) {
            for normal in s.normals {
                for n in normal {
                    bytes.extend(n.to_be_bytes());
                }
            }
        }
    }
    for face in 0..model.faces.len() {
        if surface::visible(model, face, materials) {
            bytes.extend(model.colors[face].to_be_bytes());
            bytes.push(model.alpha[face]);
        }
    }
    bytes.push(u8::from(valid));
    format!("{:x}", Sha256::digest(bytes))
}
#[test]
fn original_sparse_source_bits_and_sequential_masked_then_dual_poses_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/parts.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in fixture["sources"].as_object().unwrap() {
        let raw = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(raw)),
            hash.as_str().unwrap(),
            "source {name}"
        );
    }
    for (name, hash) in fixture["inputs"].as_object().unwrap() {
        assert_eq!(
            format!("{:x}", Sha256::digest(input(name))),
            hash.as_str().unwrap()
        );
    }
    let materials = texture::materials(&input("materials.bin")).unwrap();
    for (i, c) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let parts: Vec<_> = c["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().map(|s| Model::decode(&input(s)).unwrap()))
            .collect();
        let mut model = Model::combine_slots(&parts.iter().map(Option::as_ref).collect::<Vec<_>>());
        assert_eq!(
            raw(&model),
            c["expected"]["raw"].as_str().unwrap(),
            "sparse body {i}"
        );
        model.sort_by_priority = true;
        model.surfaces = surface::prepare(&model, &materials);
        let a = &c["first"];
        let b = &c["second"];
        let mut base = Base::decode(
            a["base_id"].as_u64().unwrap() as u16,
            &input(a["base"].as_str().unwrap()),
        )
        .unwrap();
        if let Some(parts) = c["parts"].as_array() {
            base.parts = parts.iter().map(|v| v.as_u64().unwrap() as u16).collect();
        }
        let first = Frame::decode(&input(a["first"].as_str().unwrap()), &base).unwrap();
        let next = Frame::decode(&input(a["next"].as_str().unwrap()), &base).unwrap();
        let second = Frame::decode(&input(b["first"].as_str().unwrap()), &base).unwrap();
        let second_next = Frame::decode(&input(b["next"].as_str().unwrap()), &base).unwrap();
        let primary = Pose {
            base: &base,
            frame: &first,
            next: Some(&next),
            delta: a["delta"].as_i64().unwrap() as i32,
            length: a["length"].as_i64().unwrap() as i32,
        };
        let secondary = Pose {
            base: &base,
            frame: &second,
            next: Some(&second_next),
            delta: b["delta"].as_i64().unwrap() as i32,
            length: b["length"].as_i64().unwrap() as i32,
        };
        let normals = c["normals"].as_bool().unwrap();
        let mut valid = c["originValid"].as_bool().unwrap();
        let overlays = c["overlays"].as_array().unwrap();
        for (j, o) in overlays.iter().enumerate() {
            let matrix: Option<[i32; 15]> = o["matrix"].as_array().map(|a| {
                a.iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            });
            let mut p = primary;
            if !o["tween"].as_bool().unwrap() {
                p.next = None;
            }
            model = animate_masked_hd(
                &model,
                p,
                o["mask"].as_u64().unwrap() as u16,
                matrix.as_ref(),
                normals,
                &mut valid,
            );
            assert_eq!(
                pose_hash(&model, &materials, valid),
                c["expected"]["poses"][j].as_str().unwrap(),
                "body {i} overlay {j}"
            );
        }
        let mask = c["mask"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_bool().unwrap()).collect::<Vec<_>>());
        model = animate_dual_hd_with_state(
            &model,
            primary,
            Some(secondary),
            mask.as_deref(),
            normals,
            &mut valid,
        );
        assert_eq!(
            pose_hash(&model, &materials, valid),
            c["expected"]["poses"][overlays.len()].as_str().unwrap(),
            "body {i} full/dual"
        );
    }
}
