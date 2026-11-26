//! The native builders of the layered interface packs (`ui_packs.rs`)
//! against the packs `tools/prepare_ui_models.py` and
//! `tools/prepare_player_models.py` wrote from the same pinned cache, and the
//! default packs against all four native builders.
//!
//! Both tests need `assets/cache`, the packs under `assets/visual` and the
//! reference checkout (for the pinned server's files), and report being
//! skipped without them.
use rs09_client::{
    cache_pack::{Source, ui_pack},
    ui_packs::{facial_sequences, ui_models_pack, ui_players_pack},
    world_pack::{Inputs, Options, world_pack},
};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};

#[path = "support/packs.rs"]
mod packs;
use packs::recorded;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn server_commit() -> String {
    let lock: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root().join("upstream.lock.json")).unwrap()).unwrap();
    lock["server"]["commit"].as_str().unwrap().to_owned()
}
fn server_file(path: &str) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root().join("reference/server"))
        .arg("show")
        .arg(format!("{}:Server/{path}", server_commit()))
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
const FACIAL: &str = "src/main/core/game/dialogue/FacialExpression.java";

#[test]
fn native_layered_packs_reproduce_the_recorded_python_packs() {
    let read = |path: std::path::PathBuf| std::fs::read(path).ok();
    let (Some(ui), Some(world), Some(models), Some(players), Some(facial)) = (
        read(recorded("ui.pack")),
        read(packs::visual().join("live/lumbridge-minimap.pack")),
        read(recorded("ui-models.pack")),
        read(recorded("ui-players.pack")),
        server_file(FACIAL),
    ) else {
        eprintln!("skipped: the recorded packs or the reference checkout are not present");
        return;
    };
    if !root().join("assets/cache").is_dir() {
        eprintln!("skipped: no cache");
        return;
    }
    let source = Source::open(&root().join("assets/cache")).unwrap();

    // The model pack, from the inputs its provenance names.
    let record = json(&recorded("ui-models.provenance.json"));
    assert!(record["builder"].is_null(), "not the script's pack");
    assert_eq!(record["sha256"], sha(&models));
    assert_eq!(record["baseUiSha256"], sha(&ui));
    assert_eq!(record["worldSha256"], sha(&world));
    assert_eq!(record["facialExpressionSource"], sha(&facial));
    let built = ui_models_pack(&source, &ui, &world, &facial_sequences(&facial)).unwrap();
    for key in [
        "nativeDependencyDiscovery",
        "identityKits",
        "widgetSequences",
        "frameGroups",
        "bases",
        "modelsRequested",
        "missingModels",
    ] {
        assert_eq!(built.report[key], record[key], "{key}");
    }
    assert!(built.bytes == models, "ui-models.pack is not reproduced");

    // The player pack, from that pack.
    let record = json(&recorded("ui-players.provenance.json"));
    assert!(record["builder"].is_null(), "not the script's pack");
    assert_eq!(record["sha256"], sha(&players));
    assert_eq!(record["baseUiSha256"], sha(&models));
    let built = ui_players_pack(&source, &models).unwrap();
    for key in [
        "nativeDependencyDiscovery",
        "additionalModels",
        "missingModels",
        "basDefinitions",
        "movementSequences",
        "frameGroups",
        "bases",
    ] {
        assert_eq!(built.report[key], record[key], "{key}");
    }
    assert!(built.bytes == players, "ui-players.pack is not reproduced");
}

/// The four packs the client and the browser page load by default are what
/// `build_packs -- all` writes from the pinned cache and the pinned server's
/// files, each from the one before, and what their provenance files record.
#[test]
fn the_default_packs_are_what_the_builders_write() {
    if !packs::rebuilt() || !root().join("assets/cache").is_dir() {
        eprintln!("skipped: the default packs are not rebuilt ones, or there is no cache");
        return;
    }
    let (Some(keys), Some(doors), Some(spawns), Some(facial)) = (
        server_file("data/configs/xteas.json"),
        server_file("data/configs/door_configs.json"),
        server_file("data/configs/npc_spawns.json"),
        server_file(FACIAL),
    ) else {
        eprintln!("skipped: the pinned server files are not available from git");
        return;
    };
    let cache = root().join("assets/cache");
    let on_disk = |name: &str, built: &[u8]| {
        let bytes = std::fs::read(packs::visual().join(name)).unwrap();
        assert!(bytes == built, "{name} is not what its builder writes");
        let record = json(&packs::visual().join(name).with_extension("provenance.json"));
        assert_eq!(record["sha256"], sha(built), "{name}: provenance");
        assert_eq!(record["server"]["commit"], server_commit(), "{name}");
        record
    };

    let inputs = Inputs {
        keys: Inputs::parse_keys(&keys).unwrap(),
        doors: Inputs::parse_doors(&doors).unwrap(),
        spawns: Inputs::parse_spawns(&spawns).unwrap(),
    };
    let world = world_pack(&cache, &inputs, &Options::default(), &server_commit()).unwrap();
    assert!(world.notes.is_empty(), "{:?}", world.notes);
    let record = on_disk("demo.pack", &world.bytes);
    assert_eq!(record["region_key_source_sha256"], sha(&keys));
    assert_eq!(record["door_source_sha256"], sha(&doors));
    assert_eq!(record["npc_spawn_source_sha256"], sha(&spawns));

    let ui = ui_pack(&cache, &server_commit()).unwrap();
    on_disk("ui.pack", &ui);

    let source = Source::open(&cache).unwrap();
    let models = ui_models_pack(&source, &ui, &world.bytes, &facial_sequences(&facial)).unwrap();
    let record = on_disk("ui-models.pack", &models.bytes);
    assert_eq!(record["baseUiSha256"], sha(&ui));
    assert_eq!(record["worldSha256"], sha(&world.bytes));
    assert_eq!(record["missingModels"], serde_json::json!([]));

    let players = ui_players_pack(&source, &models.bytes).unwrap();
    let record = on_disk("ui-players.pack", &players.bytes);
    assert_eq!(record["baseUiSha256"], sha(&models.bytes));
    assert_eq!(record["missingModels"], serde_json::json!([]));

    // What the rebuild was for: entries the scripts' packs did not have.
    let pack = rs09_client::pack::Pack::decode(world.bytes).unwrap();
    for name in [
        "minimap/assets",
        "underwater-complete",
        "npc-catalog-complete",
        "mapscene/0",
    ] {
        assert!(pack.get(name).is_ok(), "demo.pack lacks {name}");
    }
    let has = |prefix: &str| {
        pack.manifest
            .files
            .keys()
            .any(|name| name.starts_with(prefix))
    };
    // (The pinned cache has no light definitions in config group 31.)
    assert!(has("underwater/terrain/") && has("worldmap/"));
    let pack = rs09_client::pack::Pack::decode(players.bytes).unwrap();
    for name in ["param-catalog-complete", "sprite/scrollbar", "title/assets"] {
        assert!(pack.get(name).is_ok(), "ui-players.pack lacks {name}");
    }
    let named: serde_json::Value =
        serde_json::from_slice(pack.get("overhead/assets").unwrap()).unwrap();
    assert!(named["p12_full"].is_u64());
}
