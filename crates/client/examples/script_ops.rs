//! Tallies every CS2 opcode in the pinned cache's script archive (12), so the
//! native VM's coverage can be compared with what the game's scripts use.
//!
//! `cargo run -p rs09-client --example script_ops [-- CACHE_DIR [OUTPUT.json]]`
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_client::ui::script::Script;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let cache = args.next().unwrap_or_else(|| "assets/cache".into());
    let output = args
        .next()
        .unwrap_or_else(|| "runs/visual/script-ops.json".into());
    let limits = Limits::default();
    let store = Store::open(std::path::Path::new(&cache), StorageMode::Resident, limits)?;
    let scripts = Archive::open(store, 12, None, limits)?;
    // opcode -> [scripts using it, total instructions]
    let mut usage = BTreeMap::<i32, [usize; 2]>::new();
    let (mut decoded, mut failed) = (0usize, vec![]);
    for group in &scripts.index.groups {
        let Some(files) = scripts.group(group.id, [0; 4])? else {
            failed.push(serde_json::json!({"script": group.id, "error": "missing group"}));
            continue;
        };
        let Some(bytes) = files.get(0) else {
            failed.push(serde_json::json!({"script": group.id, "error": "missing file 0"}));
            continue;
        };
        match Script::decode(&bytes) {
            Ok(script) => {
                decoded += 1;
                let mut seen = std::collections::BTreeSet::new();
                for &op in &script.opcodes {
                    let entry = usage.entry(op).or_default();
                    entry[1] += 1;
                    if seen.insert(op) {
                        entry[0] += 1;
                    }
                }
            }
            Err(error) => {
                failed.push(serde_json::json!({"script": group.id, "error": error.to_string()}))
            }
        }
    }
    let report = serde_json::json!({
        "cache": cache,
        "scripts": decoded,
        "failed": failed,
        "opcodes": usage
            .iter()
            .map(|(op, [scripts, uses])| serde_json::json!({"op": op, "scripts": scripts, "uses": uses}))
            .collect::<Vec<_>>(),
    });
    if let Some(parent) = std::path::Path::new(&output).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{decoded} scripts decoded, {} failed, {} distinct opcodes -> {output}",
        report["failed"].as_array().unwrap().len(),
        usage.len()
    );
    Ok(())
}
