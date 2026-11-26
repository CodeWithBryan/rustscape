use rs09_client::{
    animation::{Base, Frame, animate_hd},
    model::Model,
    npc::Definition,
    pack::Pack,
    player_body, player_form, surface, texture,
    walking::Movement,
};
use rs09_protocol::game::players::RawAppearance;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/ui/player-forms.json")).unwrap()
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn definitions(f: &Value) -> BTreeMap<u32, Definition> {
    f["types"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, raw)| {
            (
                id.parse().unwrap(),
                Definition::decode(&hex(raw.as_str().unwrap())).unwrap(),
            )
        })
        .collect()
}
fn raw(flags: i32, npc: i32, bas: i32) -> RawAppearance {
    let mut bytes = vec![flags as u8, 255, 255];
    if npc != -1 {
        bytes.extend([255, 255]);
        bytes.extend((npc as u16).to_be_bytes());
        bytes.push(0);
    } else {
        bytes.extend([0; 12]);
    }
    bytes.extend([0; 5]);
    bytes.extend((bas as u16).to_be_bytes());
    bytes.extend(1i64.to_be_bytes());
    bytes.push(3);
    if flags & 4 != 0 {
        bytes.extend([0; 2]);
    } else {
        bytes.extend([3, 255]);
    }
    bytes.push(0);
    RawAppearance::decode(&bytes).unwrap()
}
#[test]
fn original_player_size_bas_shadow_morph_and_decode_position_order_match() {
    let f = fixture();
    let types = definitions(&f);
    for (index, c) in f["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, c)| c["kind"] == "form")
    {
        let a: Vec<i32> = serde_json::from_value(c["input"].clone()).unwrap();
        let previous = (a[1] != -2).then(|| raw(a[2], a[1], 0));
        let raw = raw(a[0], a[7], a[8]);
        let mut motion = Movement::new([0; 2]);
        motion.fine = [a[3], a[4]];
        motion.head = [a[5], a[6]];
        player_form::appearance_position(&mut motion, previous.as_ref(), &raw, &types).unwrap();
        let form = player_form::resolve(&raw, &types, |bit, _| Ok(if bit { a[9] } else { a[10] }))
            .unwrap();
        assert_eq!(
            json!([
                motion.fine[0],
                motion.fine[1],
                form.size,
                raw.bas,
                form.shadow,
                form.render_id,
                form.resize[0],
                form.resize[1]
            ]),
            c["expected"],
            "form{index}"
        );
        assert_eq!(form.visible(), raw.npc_id == -1 || form.render_id.is_some());
    }
}
fn canonical(m: &Model) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend((m.vertices.len() as i32).to_be_bytes());
    b.extend((m.faces.len() as i32).to_be_bytes());
    for v in &m.vertices {
        for n in v {
            b.extend(n.to_be_bytes());
        }
    }
    for f in &m.faces {
        for n in f {
            b.extend((*n as i32).to_be_bytes());
        }
    }
    for n in &m.colors {
        b.extend(n.to_be_bytes());
    }
    b.extend(&m.alpha);
    b.extend((m.vertex_labels.len() as i32).to_be_bytes());
    b.extend(&m.vertex_labels);
    b.extend((m.face_labels.len() as i32).to_be_bytes());
    b.extend(&m.face_labels);
    for n in &m.textures {
        b.extend(n.to_be_bytes());
    }
    b
}
#[test]
fn original_npc_assembly_and_transformed_player_gl_poses_match() {
    check_body_poses(false);
}
#[test]
fn original_actor_gl_terrain_after_animation_resize_and_shadow_bounds_match() {
    check_body_poses(true);
}
fn check_body_poses(terrain: bool) {
    let f = fixture();
    let types = definitions(&f);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Needs local assets that are not in the repository.
    let missing: Vec<_> = f["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| root.join(p["path"].as_str().unwrap()))
        .filter(|path| !path.exists())
        .collect();
    if !missing.is_empty() {
        eprintln!("skipped: needs {missing:?}, nothing was checked");
        return;
    }
    let packs: Vec<_> = f["packs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let b = std::fs::read(root.join(p["path"].as_str().unwrap())).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&b)), p["sha256"]);
            Pack::decode(b).unwrap()
        })
        .collect();
    let bytes =
        |name: &str| -> &[u8] { packs.iter().rev().find_map(|p| p.get(name).ok()).unwrap() };
    let materials_bytes = std::fs::read(root.join("fixtures/visual/materials.bin")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&materials_bytes)),
        f["materialsSha256"]
    );
    let materials = texture::materials(&materials_bytes).unwrap();
    let mut models = BTreeMap::new();
    for d in types.values() {
        for &id in &d.models {
            if id != u32::MAX {
                models
                    .entry(id)
                    .or_insert_with(|| Model::decode(bytes(&format!("model/{id}"))).unwrap());
            }
        }
    }
    let mut bodies = BTreeMap::new();
    for (index, c) in f[if terrain { "terrainCases" } else { "cases" }]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, c)| c["kind"] != "form")
    {
        let id = c["id"].as_u64().unwrap() as u32;
        let d = &types[&id];
        let m = bodies.entry(id).or_insert_with(|| {
            let transforms = if d.bas == -1 {
                [None; 12]
            } else {
                let key = d.bas.to_string();
                let b = f["bas"].get(&key).map(|s| hex(s.as_str().unwrap()));
                player_body::bas_transforms(
                    b.as_deref()
                        .unwrap_or_else(|| bytes(&format!("bas/{}", d.bas))),
                )
                .unwrap()
                .unwrap_or([None; 12])
            };
            let model = d.assemble_body(&models, transforms, &[0; 256]).unwrap();
            // Normal NPC assembly must use the same original BAS part order.
            // Resolve the explicit zero-palette fixture into its direct colours;
            // mutable script palettes remain outside the normal-NPC path.
            let mut normal = d.clone();
            for (i, _) in normal.palette.iter().enumerate() {
                if let Some(pair) = normal.recolor.get_mut(i) {
                    pair[1] = 0;
                }
            }
            normal.palette.clear();
            let bas = if d.bas == -1 {
                rs09_client::walking::PlayerBas::default()
            } else {
                let raw = f["bas"]
                    .get(d.bas.to_string())
                    .map(|s| hex(s.as_str().unwrap()));
                rs09_client::walking::PlayerBas::decode(
                    raw.as_deref()
                        .unwrap_or_else(|| bytes(&format!("bas/{}", d.bas))),
                )
                .unwrap()
            };
            let normal_model = normal.assemble_with_bas(&models, &bas).unwrap();
            assert_eq!(
                canonical(&normal_model),
                canonical(&model),
                "normal NPC{id}"
            );
            model
        });
        let output = if c["kind"] == "body" {
            canonical(m)
        } else {
            let mut m = m.clone();
            m.surfaces = surface::prepare(&m, &materials);
            let base = Base::decode(
                c["base_id"].as_u64().unwrap() as u16,
                bytes(c["base"].as_str().unwrap()),
            )
            .unwrap();
            let frame = Frame::decode(bytes(c["first"].as_str().unwrap()), &base).unwrap();
            let next = Frame::decode(bytes(c["second"].as_str().unwrap()), &base).unwrap();
            let mut pose = animate_hd(
                &m,
                &base,
                &frame,
                Some(&next),
                c["delta"].as_i64().unwrap() as i32,
                c["length"].as_i64().unwrap() as i32,
                c["normals"].as_bool().unwrap(),
            );
            player_form::Form {
                resize: d.resize,
                ..Default::default()
            }
            .resize_model(&mut pose);
            if terrain {
                let before_y = pose.vertices.iter().map(|v| v[1]).fold(32767, i32::min) as i16;
                let bas = rs09_client::walking::PlayerBas::decode(&hex(f["bas"]
                    [c["bas_id"].as_i64().unwrap().to_string()]
                .as_str()
                .unwrap()))
                .unwrap();
                let mut calls = Vec::new();
                let alignment = rs09_client::spot::Alignment::sample(
                    bas.terrain,
                    c["yaw"].as_i64().unwrap() as i32,
                    serde_json::from_value(c["fine"].clone()).unwrap(),
                    c["tile_height"].as_i64().unwrap() as i32,
                    |point| {
                        let value = c["heights"][calls.len()].as_i64().unwrap() as i32;
                        calls.push([0, point[0], point[1]]);
                        value
                    },
                );
                alignment.apply(&mut pose);
                let after_y = pose.vertices.iter().map(|v| v[1]).fold(32767, i32::min) as i16;
                assert_eq!(
                    json!(before_y),
                    c["expected"]["beforeY"],
                    "before case{index}"
                );
                assert_eq!(json!(after_y), c["expected"]["afterY"], "after case{index}");
                assert_eq!(json!(calls), c["expected"]["calls"], "samples case{index}");
                assert_eq!(
                    json!([alignment.pitch, alignment.roll, alignment.offset]),
                    c["expected"]["terrain"],
                    "angles case{index}"
                );
            }
            let mut b = Vec::new();
            for v in &pose.vertices {
                for n in v {
                    b.extend(n.to_be_bytes());
                }
            }
            for (face, s) in pose.surfaces.iter().enumerate() {
                if surface::visible(&pose, face, &materials) {
                    for normal in s.normals {
                        for n in normal {
                            b.extend(n.to_be_bytes());
                        }
                    }
                }
            }
            b
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(output)),
            if terrain {
                &c["expected"]["sha256"]
            } else {
                &c["expected"]
            }
            .as_str()
            .unwrap(),
            "{} {id} case{index}",
            c["kind"]
        );
    }
}
#[test]
fn missing_definition_variable_service_and_morph_cycles_are_explicit_errors() {
    let mut types = definitions(&fixture());
    assert!(player_form::resolve(&raw(0, 12345, 0), &types, |_, _| Ok(0)).is_err());
    assert!(
        player_form::resolve(&raw(0, 60001, 0), &types, |_, _| Err(
            "Varbit unavailable".into()
        ))
        .unwrap_err()
        .contains("Varbit unavailable")
    );
    types.get_mut(&60000).unwrap().children = Some(vec![60000, 60000]);
    assert!(
        player_form::resolve(&raw(0, 60000, 0), &types, |_, _| Ok(0))
            .unwrap_err()
            .contains("Cyclic")
    );
}
