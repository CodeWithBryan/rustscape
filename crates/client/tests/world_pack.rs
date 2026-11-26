//! The native starting-scene pack against the recorded `demo.pack`
//! (`support/packs.rs`), which an earlier `tools/prepare_visual.py` wrote
//! from the same pinned cache.
//!
//! The script read three files from the pinned server commit; the test takes
//! them from the reference checkout with `git show`. It needs `assets/cache`,
//! the recorded pack and that checkout, and reports being skipped without
//! them. The recorded pack predates later additions to the script (every raw
//! NPC definition, lights, map scenes, the minimap sprite IDs and morph
//! children), so byte identity is not expected: every entry it has must be
//! present and identical, in the same order, and `scene.json` must describe
//! the same scene.
use rs09_client::{
    pack::Pack,
    world_pack::{Inputs, Options, world_pack},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, process::Command};

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
fn server_file(name: &str) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root().join("reference/server"))
        .arg("show")
        .arg(format!("{}:Server/data/configs/{name}", server_commit()))
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}
/// Names in the order their payloads were written.
fn order(pack: &Pack) -> Vec<&str> {
    let mut names: Vec<(&str, [usize; 2])> = pack
        .manifest
        .files
        .iter()
        .map(|(name, span)| (name.as_str(), *span))
        .collect();
    names.sort_by_key(|(_, span)| *span);
    names.into_iter().map(|(name, _)| name).collect()
}

#[test]
fn native_world_pack_reproduces_the_recorded_python_pack() {
    let (Ok(recorded_bytes), true) = (
        std::fs::read(recorded("demo.pack")),
        root().join("assets/cache").is_dir(),
    ) else {
        eprintln!("skipped: no recorded pack or cache");
        return;
    };
    let (Some(keys), Some(doors), Some(spawns)) = (
        server_file("xteas.json"),
        server_file("door_configs.json"),
        server_file("npc_spawns.json"),
    ) else {
        eprintln!("skipped: the pinned server files are not available from git");
        return;
    };
    let provenance: serde_json::Value =
        serde_json::from_slice(&std::fs::read(recorded("provenance.json")).unwrap()).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&recorded_bytes)),
        provenance["pack_sha256"].as_str().unwrap(),
        "the recorded demo.pack is not the pack its provenance records"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&spawns)),
        provenance["npc_spawn_source_sha256"].as_str().unwrap(),
        "the spawn file is not the one the recorded pack was built from"
    );
    let inputs = Inputs {
        keys: Inputs::parse_keys(&keys).unwrap(),
        doors: Inputs::parse_doors(&doors).unwrap(),
        spawns: Inputs::parse_spawns(&spawns).unwrap(),
    };
    let built = world_pack(
        &root().join("assets/cache"),
        &inputs,
        &Options::default(),
        &server_commit(),
    )
    .unwrap();
    assert!(built.notes.is_empty(), "{:?}", built.notes);
    let recorded = Pack::decode(recorded_bytes).unwrap();
    let native = Pack::decode(built.bytes).unwrap();
    assert_eq!(native.manifest.source, recorded.manifest.source);

    let mut differing = vec![];
    for name in recorded.manifest.files.keys() {
        let old = recorded.get(name).unwrap();
        let Ok(new) = native.get(name) else {
            differing.push(format!("{name} is missing"));
            continue;
        };
        if name == "scene.json" {
            let old: serde_json::Value = serde_json::from_slice(old).unwrap();
            let new: serde_json::Value = serde_json::from_slice(new).unwrap();
            // Every field the recorded scene has is unchanged. The native
            // scene always names itself and its start.
            for (key, value) in old.as_object().unwrap() {
                if key == "definitions" {
                    for (id, definition) in value.as_object().unwrap() {
                        let mut ours = new["definitions"][id].clone();
                        // Morph children were added to the script later, and
                        // the placement fields by the native builder.
                        for added in ["children", "placement"] {
                            if definition.get(added).is_none()
                                && let Some(object) = ours.as_object_mut()
                            {
                                object.remove(added);
                            }
                        }
                        if &ours != definition {
                            differing.push(format!("scene definition {id}"));
                        }
                    }
                    assert_eq!(
                        new["definitions"].as_object().unwrap().len(),
                        value.as_object().unwrap().len()
                    );
                } else if &new[key] != value {
                    differing.push(format!("scene field {key}"));
                }
            }
            assert_eq!(new["name"], "Lumbridge");
            assert_eq!(new["start"], serde_json::json!([3222, 3218]));
        } else if old != new {
            differing.push(format!("{name} differs"));
        }
    }
    assert!(
        differing.is_empty(),
        "{} of {} recorded entries differ: {:?}",
        differing.len(),
        recorded.manifest.files.len(),
        &differing[..differing.len().min(20)]
    );

    // What the native pack has beyond the recorded one, by family.
    let mut added = BTreeMap::<&str, usize>::new();
    for name in native.manifest.files.keys() {
        if !recorded.manifest.files.contains_key(name) {
            *added.entry(name.split('/').next().unwrap()).or_default() += 1;
        }
    }
    eprintln!("added by family: {added:?}");
    assert!(
        added.keys().all(|family| [
            "npc",
            "npc-catalog-complete",
            "light",
            "mapscene",
            "minimap",
            "loc",
            // The world map details and labels the minimap's text reads.
            "worldmap",
            // The underwater maps of the scene's regions, the entry that
            // says they are all listed, and the models of their locations.
            "underwater",
            "underwater-complete",
            // Models and animations of underwater locations and of morph
            // children (the script packed a child's definition alone).
            "model",
            "seq",
            "frame",
            "base"
        ]
        .contains(family)),
        "{added:?}"
    );

    // The shared entries keep the script's order. Movement sets are left out
    // of this comparison: the current script writes one again for each NPC
    // that uses it (the recorded pack's script wrote it once), and a pack
    // only remembers where an entry's latest bytes are.
    let comparable =
        |name: &&str| recorded.manifest.files.contains_key(*name) && !name.starts_with("bas/");
    let kept: Vec<&str> = order(&native).into_iter().filter(comparable).collect();
    let original: Vec<&str> = order(&recorded).into_iter().filter(comparable).collect();
    assert_eq!(kept.len(), original.len());
    if let Some(at) = (0..kept.len()).find(|&at| kept[at] != original[at]) {
        let window = at.saturating_sub(3)..(at + 14).min(kept.len());
        panic!(
            "order differs at {at}:\n then {:?}\n now  {:?}",
            &original[window.clone()],
            &kept[window]
        );
    }
}

#[test]
fn a_lenient_build_without_keys_keeps_the_terrain() {
    if !root().join("assets/cache").is_dir() {
        eprintln!("skipped: no cache");
        return;
    }
    let options = Options {
        lenient: true,
        ..Options::default()
    };
    let built = world_pack(
        &root().join("assets/cache"),
        &Inputs::default(),
        &options,
        "test",
    )
    .unwrap();
    let pack = Pack::decode(built.bytes).unwrap();
    let scene: serde_json::Value = serde_json::from_slice(pack.get("scene.json").unwrap()).unwrap();
    assert_eq!(scene["regions"].as_array().unwrap().len(), 4);
    for region in scene["regions"].as_array().unwrap() {
        let name = format!("terrain/{}_{}", region[0], region[1]);
        assert!(pack.get(&name).unwrap().len() > 4096, "{name}");
    }
    // Lumbridge's location files are encrypted: each is noted and left empty.
    eprintln!("notes: {:?}", built.notes);
    assert!(!built.notes.is_empty());
    // The player's body and the floors are there regardless.
    assert!(pack.get("kit/18").is_ok() && pack.get("bas/1426").is_ok());
    assert!(pack.get("materials").is_ok() && pack.get("minimap/assets").is_ok());
    // Without the keys the strict build fails, as the script did.
    assert!(
        world_pack(
            &root().join("assets/cache"),
            &Inputs::default(),
            &Options::default(),
            "test"
        )
        .is_err()
    );
}
