//! Native replacement for `tools/prepare_ui.py` and `tools/prepare_visual.py`:
//! builds asset packs from the pinned cache without Python.
//!
//! `cargo run --release -p rs09-client --example build_packs -- ui [OUTPUT] [--cache DIR]`
//!
//! `... -- world OUTPUT [--cache DIR] [--keys xteas.json] [--doors door_configs.json]
//! [--spawns npc_spawns.json] [--center X,Y] [--label NAME] [--login-window]
//! [--region-npcs] [--lenient]` builds a starting-scene pack. The three JSON
//! files are the pinned server's; each is optional, but without a region's
//! key its locations cannot be read (`--lenient` then leaves them out).
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let cache = PathBuf::from(option("--cache").unwrap_or_else(|| "assets/cache".into()));
    let lock: serde_json::Value = serde_json::from_slice(&std::fs::read("upstream.lock.json")?)?;
    let commit = lock["server"]["commit"]
        .as_str()
        .ok_or("upstream.lock.json has no server commit")?;
    let kind = args.first().map(String::as_str).unwrap_or("ui");
    let (bytes, default_output) = match kind {
        "ui" => (
            rs09_client::cache_pack::ui_pack(&cache, commit)?,
            "assets/visual/ui.pack",
        ),
        "world" => {
            use rs09_client::world_pack::{Inputs, Options, world_pack};
            let read = |key: &str| option(key).map(std::fs::read).transpose();
            let mut inputs = Inputs::default();
            if let Some(bytes) = read("--keys")? {
                inputs.keys = Inputs::parse_keys(&bytes)?;
            }
            if let Some(bytes) = read("--doors")? {
                inputs.doors = Inputs::parse_doors(&bytes)?;
            }
            if let Some(bytes) = read("--spawns")? {
                inputs.spawns = Inputs::parse_spawns(&bytes)?;
            }
            let mut options = Options {
                login_window: args.iter().any(|a| a == "--login-window"),
                region_npcs: args.iter().any(|a| a == "--region-npcs"),
                lenient: args.iter().any(|a| a == "--lenient"),
                ..Options::default()
            };
            if let Some(center) = option("--center") {
                let (x, y) = center.split_once(',').ok_or("--center takes X,Y")?;
                options.center = [x.parse()?, y.parse()?];
            }
            if let Some(label) = option("--label") {
                options.label = label;
            }
            let built = world_pack(&cache, &inputs, &options, commit)?;
            for note in &built.notes {
                eprintln!("left out: {note}");
            }
            (built.bytes, "runs/visual/world.pack")
        }
        other => return Err(format!("unknown pack kind {other}; expected ui or world").into()),
    };
    let output = PathBuf::from(
        args.get(1)
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| default_output.into()),
    );
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, &bytes)?;
    println!(
        "{} bytes, sha256 {:x} -> {}",
        bytes.len(),
        Sha256::digest(&bytes),
        output.display()
    );
    Ok(())
}
