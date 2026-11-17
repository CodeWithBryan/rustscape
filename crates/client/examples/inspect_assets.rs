use rs09_client::{model::Model, pack::Pack, texture};
use std::collections::{BTreeMap, BTreeSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = Pack::decode(std::fs::read("assets/visual/demo.pack")?)?;
    let scene: serde_json::Value = serde_json::from_slice(pack.get("scene.json")?)?;
    let mut usage = BTreeMap::<i32, usize>::new();
    let mut models = BTreeMap::new();
    for name in pack
        .manifest
        .files
        .keys()
        .filter(|n| n.starts_with("model/"))
    {
        let model = Model::decode(pack.get(name)?)?;
        for &id in &model.textures {
            if id >= 0 {
                *usage.entry(id).or_default() += 1;
            }
        }
        models.insert(name[6..].parse::<u32>()?, model);
    }
    let mut seen = BTreeMap::<[i32; 3], (u32, u8)>::new();
    let mut conflicts = BTreeSet::new();
    for id in scene["avatar_models"].as_array().unwrap() {
        let id = id.as_u64().unwrap() as u32;
        let m = &models[&id];
        for (i, &v) in m.vertices.iter().enumerate() {
            let label = m.vertex_labels.get(i).copied().unwrap_or(0);
            if let Some(&(other, bone)) = seen.get(&v) {
                if bone != label {
                    conflicts.insert((other, id, bone, label));
                }
            } else {
                seen.insert(v, (id, label));
            }
        }
    }
    let mut report = Vec::new();
    for (id, faces) in usage {
        let generated = texture::generate(&pack, id, 64);
        report.push(serde_json::json!({"id":id,"faces":faces,"generated":generated.is_ok(),"error":generated.err()}));
    }
    std::fs::create_dir_all("runs/visual")?;
    std::fs::write(
        "runs/visual/material-inventory.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "{} material IDs used; avatar shared-coordinate label conflicts: {:?}",
        report.len(),
        conflicts
    );
    Ok(())
}
