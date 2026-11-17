//! Discover raw model dependencies using the already-ported cache decoders.
use rs09_client::{
    npc,
    pack::Pack,
    ui::{Component, item::ItemType},
};
use std::collections::BTreeSet;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let ui = Pack::decode(std::fs::read(&args[1]).unwrap()).unwrap();
    let world = Pack::decode(std::fs::read(&args[2]).unwrap()).unwrap();
    let mut models = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    let (mut components, mut items, mut npcs) = (0, 0, 0);
    for name in ui.manifest.files.keys() {
        if let Some(id) = name.strip_prefix("item/") {
            let (d, _) = ItemType::decode(id.parse().unwrap(), ui.get(name).unwrap()).unwrap();
            items += 1;
            for id in [d.manhead, d.manhead2, d.womanhead, d.womanhead2] {
                if id >= 0 {
                    models.insert(id);
                }
            }
        } else if let Some(id) = name.strip_prefix("interface/") {
            let (g, c) = id.split_once('/').unwrap();
            let id = (g.parse::<i32>().unwrap() << 16) | c.parse::<i32>().unwrap();
            let (d, _) = Component::decode(id, ui.get(name).unwrap()).unwrap();
            components += 1;
            if d.kind == 6 {
                for (kind, id) in [
                    (d.model_type, d.model_id),
                    (d.active_model_type, d.active_model_id),
                ] {
                    if kind == 1 && id >= 0 {
                        models.insert(id);
                    }
                }
                for id in [d.model_seq_id, d.active_model_seq_id] {
                    if id >= 0 {
                        sequences.insert(id);
                    }
                }
            }
        }
    }
    for name in world
        .manifest
        .files
        .keys()
        .filter(|s| s.starts_with("npc/"))
    {
        let d = npc::Definition::decode(world.get(name).unwrap()).unwrap();
        npcs += 1;
        if let Some(heads) = d.head_models {
            models.extend(heads.into_iter().map(|id| id as i32));
        }
    }
    println!(
        "{}",
        serde_json::json!({"models":models,"sequences":sequences,"components":components,"items":items,"npcs":npcs})
    );
}
