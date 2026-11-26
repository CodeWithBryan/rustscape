use rs09_client::{
    appearance::IdentityKit, model::Model, pack::Pack, player_body, ui::item::Catalog,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn canonical(model: &Model) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((model.vertices.len() as i32).to_be_bytes());
    out.extend((model.faces.len() as i32).to_be_bytes());
    for v in &model.vertices {
        for n in v {
            out.extend(n.to_be_bytes());
        }
    }
    for face in &model.faces {
        for n in face {
            out.extend((*n as i32).to_be_bytes());
        }
    }
    for n in &model.colors {
        out.extend(n.to_be_bytes());
    }
    out.extend(&model.alpha);
    out.extend((model.vertex_labels.len() as i32).to_be_bytes());
    out.extend(&model.vertex_labels);
    out.extend((model.face_labels.len() as i32).to_be_bytes());
    out.extend(&model.face_labels);
    for n in &model.textures {
        out.extend(n.to_be_bytes());
    }
    out
}
fn raw(case: &Value, pack: &Pack, name: &str) -> Vec<u8> {
    if let Some(s) = case["raw"].as_str() {
        s.as_bytes()
            .chunks_exact(2)
            .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
            .collect()
    } else {
        pack.get(name).unwrap().to_vec()
    }
}
#[path = "support/packs.rs"]
mod packs;
use packs::recorded;

#[test]
fn original_all_wearable_bodies_kit_arrays_bas_transforms_and_compositions_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/player-bodies.json")).unwrap();
    let bytes = std::fs::read(recorded("ui-players.pack")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        fixture["packSha256"]
    );
    let pack = Pack::decode(bytes).unwrap();
    let models: BTreeMap<u32, Model> = pack
        .manifest
        .files
        .keys()
        .filter_map(|n| {
            n.strip_prefix("model/").map(|id| {
                (
                    id.parse().unwrap(),
                    Model::decode(pack.get(n).unwrap()).unwrap(),
                )
            })
        })
        .collect();
    let mut items = Catalog::default();
    let mut bas = BTreeMap::new();
    for c in fixture["cases"].as_array().unwrap() {
        let id = c["id"].as_i64().unwrap_or(0) as i32;
        let m = match c["kind"].as_str().unwrap() {
            "bas" => {
                let t = player_body::bas_transforms(&raw(c, &pack, &format!("bas/{id}"))).unwrap();
                assert_eq!(
                    serde_json::to_value(t).unwrap(),
                    serde_json::from_str::<Value>(c["expected"].as_str().unwrap()).unwrap(),
                    "BAS{id}"
                );
                bas.insert(id, t.unwrap_or([None; 12]));
                continue;
            }
            "kit" => player_body::kit_body(
                &IdentityKit::decode(&raw(c, &pack, &format!("kit/{id}"))).unwrap(),
                &models,
            )
            .unwrap(),
            "item" => player_body::item_body(
                &items.get(&pack, id).unwrap(),
                &models,
                c["female"].as_bool().unwrap(),
            )
            .unwrap(),
            "player" => Some(
                player_body::human_body(
                    &pack,
                    &models,
                    &mut items,
                    serde_json::from_value(c["slots"].clone()).unwrap(),
                    serde_json::from_value(c["colors"].clone()).unwrap(),
                    c["female"].as_bool().unwrap(),
                    serde_json::from_value(c["overrides"].clone()).unwrap(),
                    bas.get(&(c["bas"].as_i64().unwrap() as i32))
                        .copied()
                        .unwrap_or([None; 12]),
                )
                .unwrap(),
            ),
            _ => unreachable!(),
        };
        let actual = m.map_or("null".into(), |m| {
            format!("{:x}", Sha256::digest(canonical(&m)))
        });
        assert_eq!(actual, c["expected"].as_str().unwrap(), "{c}");
    }
}

#[test]
fn specular_texture_alpha_is_shine_intensity_and_preserves_original_primary_opacity() {
    use rs09_client::texture;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let world = Pack::decode(
        std::fs::read(root.join("assets/visual/live/lumbridge-minimap.pack")).unwrap(),
    )
    .unwrap();
    let materials = texture::materials(world.get("materials").unwrap()).unwrap();
    let material = materials
        .iter()
        .flatten()
        .find(|m| m.material_type == 1)
        .unwrap();
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/material-alpha.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let vertex = case["vertex"].as_u64().unwrap();
        let texture = case["texture"].as_u64().unwrap();
        let preserve = texture::primary_alpha(Some(material));
        let alpha = if preserve {
            vertex
        } else {
            vertex * texture / 255
        };
        assert_eq!(
            serde_json::json!([if preserve { 4 } else { 0 }, alpha]),
            case["expected"],
            "{case}"
        );
    }
    assert!(!texture::primary_alpha(None));
}
