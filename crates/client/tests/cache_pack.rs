//! The native pack builder against the pack an earlier `tools/prepare_ui.py`
//! wrote from the same pinned cache. Requires `assets/cache` and `assets/visual`.
//!
//! The recorded pack predates three additions to that script (the parameter
//! definitions, the `p12_full` overhead font and the scrollbar sprite), so
//! byte identity is not expected. Everything the recorded pack contains must
//! be present, identical and in the same order, and the only additions must
//! be those three.
use rs09_client::pack::Pack;
use sha2::{Digest, Sha256};
use std::path::Path;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn server_commit() -> String {
    let lock: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root().join("upstream.lock.json")).unwrap()).unwrap();
    lock["server"]["commit"].as_str().unwrap().to_owned()
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
fn native_interface_pack_reproduces_the_recorded_python_pack() {
    let recorded_bytes = std::fs::read(root().join("assets/visual/ui.pack")).unwrap();
    let provenance: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root().join("assets/visual/ui.provenance.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&recorded_bytes)),
        provenance["sha256"].as_str().unwrap(),
        "assets/visual/ui.pack is not the pack its provenance records"
    );
    let recorded = Pack::decode(recorded_bytes).unwrap();
    let native = Pack::decode(
        rs09_client::cache_pack::ui_pack(&root().join("assets/cache"), &server_commit()).unwrap(),
    )
    .unwrap();
    assert_eq!(native.manifest.source, recorded.manifest.source);

    let mut compared = 0usize;
    for name in recorded.manifest.files.keys() {
        let (old, new) = (recorded.get(name).unwrap(), native.get(name).unwrap());
        if name == "overhead/assets" {
            let mut old: serde_json::Value = serde_json::from_slice(old).unwrap();
            let new: serde_json::Value = serde_json::from_slice(new).unwrap();
            let id = new["p12_full"].clone();
            assert!(id.is_u64(), "p12_full resolves to a sprite group");
            old.as_object_mut().unwrap().insert("p12_full".into(), id);
            assert_eq!(old, new);
        } else {
            assert!(old == new, "{name} differs from the recorded pack");
            compared += 1;
        }
    }
    assert_eq!(compared, recorded.manifest.files.len() - 1);

    let added: Vec<&str> = native
        .manifest
        .files
        .keys()
        .map(String::as_str)
        .filter(|name| !recorded.manifest.files.contains_key(*name))
        .collect();
    let other: Vec<&str> = added
        .iter()
        .copied()
        .filter(|name| !name.starts_with("param/"))
        .collect();
    assert_eq!(
        other,
        ["param-catalog-complete", "sprite/scrollbar", "title/assets"]
    );
    assert!(
        added.len() > other.len(),
        "parameter definitions are packed"
    );

    let kept: Vec<&str> = order(&native)
        .into_iter()
        .filter(|name| recorded.manifest.files.contains_key(*name))
        .collect();
    assert!(kept == order(&recorded), "payload order differs");

    // The recorded counts, recomputed from the native manifest.
    let count = |prefix: &str| {
        native
            .manifest
            .files
            .keys()
            .filter(|name| name.starts_with(prefix))
            .count() as u64
    };
    for (key, prefix) in [
        ("interface", "interface/"),
        ("script", "script/"),
        ("font", "font/"),
        ("enum", "enum/"),
        ("item", "item/"),
        ("varbit", "varbit/"),
        ("item_models", "model/"),
        ("sounds", "sound/"),
    ] {
        assert_eq!(
            count(prefix),
            provenance["counts"][key].as_u64().unwrap(),
            "{key}"
        );
    }
}

/// Every cache-derived entry of every prepared pack on disk must come back
/// byte-identical from the lazily resolved whole-cache pack. Generated scene
/// descriptions and map files are outside its scope and are skipped by name.
#[test]
fn whole_cache_pack_serves_every_entry_of_the_prepared_packs() {
    let cache = rs09_client::cache_pack::cache_pack(&root().join("assets/cache"), &server_commit())
        .unwrap();
    let mut total = 0usize;
    for name in [
        "ui.pack",
        "ui-models.pack",
        "ui-players.pack",
        "animations.pack",
        "demo.pack",
        "live/lumbridge.pack",
        "live/lumbridge-minimap.pack",
        "tour/varrock.pack",
    ] {
        let prepared =
            Pack::decode(std::fs::read(root().join("assets/visual").join(name)).unwrap()).unwrap();
        let mut compared = 0usize;
        for entry in prepared.manifest.files.keys() {
            if entry == "scene.json"
                || entry.starts_with("terrain/")
                || entry.starts_with("locations/")
            {
                continue;
            }
            let (old, new) = (
                prepared.get(entry).unwrap(),
                cache
                    .get(entry)
                    .unwrap_or_else(|e| panic!("{name}: {entry}: {e}")),
            );
            if entry == "overhead/assets" || entry == "minimap/assets" {
                // Older packs list fewer named sprites (the cache pack's
                // minimap entry also names `floorshadows`, as `world_pack`'s
                // does); shared names must agree.
                let old: serde_json::Value = serde_json::from_slice(old).unwrap();
                let new: serde_json::Value = serde_json::from_slice(new).unwrap();
                for (key, id) in old.as_object().unwrap() {
                    assert_eq!(&new[key], id, "{name}: overhead sprite {key}");
                }
            } else {
                assert!(old == new, "{name}: {entry} differs from the cache pack");
            }
            compared += 1;
        }
        assert!(compared > 1000, "{name}: only {compared} entries compared");
        total += compared;
    }
    // Six packs overlap heavily; this is the number of comparisons, not assets.
    assert!(total > 500_000, "{total} entries compared");
    // Quick chat definitions resolve and decode; the counts marker lists the
    // phrase capacity of the local and global archives.
    let counts: Vec<u32> = serde_json::from_slice(cache.get("quickchat/counts").unwrap()).unwrap();
    assert!(
        counts.len() == 2 && counts.iter().all(|&n| n > 0),
        "{counts:?}"
    );
    let mut phrases = 0usize;
    for name in cache.manifest.files.keys() {
        if let Some(id) = name.strip_prefix("qcphrase/") {
            let id: i32 = id.parse().unwrap();
            let phrase = rs09_client::ui::quickchat::PhraseType::decode(
                cache.get(name).unwrap(),
                id >= 32768,
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            // A dynamic value follows every text segment but the last.
            if let (Some(segments), Some(commands)) = (&phrase.segments, &phrase.commands) {
                assert_eq!(segments.len(), commands.len() + 1, "{name}");
            }
            phrases += 1;
        } else if let Some(id) = name.strip_prefix("qccat/") {
            let id: i32 = id.parse().unwrap();
            rs09_client::ui::quickchat::CatType::decode(cache.get(name).unwrap(), id >= 32768)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
    assert!(phrases > 100, "{phrases} quick chat phrases decoded");
    // Names outside the cache are refused rather than guessed.
    assert!(cache.get("model/999999").is_err());
    assert!(cache.get("scene.json").is_err());
}
