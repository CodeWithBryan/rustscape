//! JVM differentials for the HD floor mesh (`gl_tile`), the baked tile shade
//! (`tile_shade`) and merged scenery normals (`normal_merge`).
//!
//! `fixtures/terrain/oracle.json` holds SHA-256 digests of what the original
//! code produced over four 104 x 104 scenes cut from the pinned cache
//! (Lumbridge, Varrock, Falador, Draynor); `examples/terrain_oracle.rs`
//! generates it and `fixtures/terrain/provenance.json` lists the original
//! sources and the scope. Here the Rust ports run over the same scenes and
//! must reproduce every byte. The scenes need `assets/cache` and the recorded
//! map keys in `runs/player-session/xteas.json`; without either the tests are
//! skipped.
mod terrain_support;

use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};
use terrain_support as support;

struct Run {
    /// Section name to the digest of the Rust result.
    digests: BTreeMap<String, String>,
    expected: BTreeMap<String, String>,
    stats: BTreeMap<String, support::Stats>,
}

fn run() -> Option<&'static Run> {
    static RUN: OnceLock<Option<Run>> = OnceLock::new();
    RUN.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = match support::open(&root) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("skipped: no cache or map keys ({error})");
                return None;
            }
        };
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("fixtures/terrain/oracle.json")).unwrap(),
        )
        .unwrap();
        let hex = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        let scenes: Vec<_> = support::SCENES
            .iter()
            .map(|(name, base)| source.scene(name, *base).unwrap())
            .collect();
        assert_eq!(
            hex(&support::input(&source, &scenes)),
            fixture["inputSha256"].as_str().unwrap(),
            "the scenes cut from the cache are not the ones the oracle was given; \
             if a decoder changed on purpose, regenerate with \
             `cargo run -p rs09-client --example terrain_oracle`"
        );
        let mut digests = BTreeMap::new();
        let mut stats = BTreeMap::new();
        digests.insert("clear/shadow".to_owned(), hex(&support::clear_section()));
        for (name, bytes) in support::palette_sections() {
            digests.insert(name, hex(&bytes));
        }
        for scene in &scenes {
            let (sections, scene_stats) = support::sections(&source, scene);
            stats.insert(scene.name.clone(), scene_stats);
            for (name, bytes) in sections {
                digests.insert(format!("{}/{name}", scene.name), hex(&bytes));
            }
        }
        let expected = fixture["sections"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, entry)| (name.clone(), entry["sha256"].as_str().unwrap().to_owned()))
            .collect();
        Some(Run {
            digests,
            expected,
            stats,
        })
    })
    .as_ref()
}

/// Every fixture section whose name passes `wanted` is reproduced, and there
/// are `count` of them.
fn reproduces(wanted: impl Fn(&str) -> bool, count: usize) -> Option<&'static Run> {
    let run = run()?;
    let names: Vec<_> = run
        .expected
        .keys()
        .filter(|name| wanted(name.split('/').nth(1).unwrap()))
        .collect();
    assert_eq!(names.len(), count);
    let wrong: Vec<_> = names
        .into_iter()
        .filter(|name| run.digests.get(*name) != run.expected.get(*name))
        .collect();
    assert!(wrong.is_empty(), "differs from the original: {wrong:?}");
    Some(run)
}

#[test]
fn the_floor_mesh_is_the_originals_vertex_for_vertex() {
    // Per scene and level: the underlay groups, the overlay groups and the
    // diagonal chosen for every plain tile.
    let Some(run) = reproduces(
        |name| {
            ["plain", "shaped", "diag"]
                .iter()
                .any(|p| name.starts_with(p))
        },
        4 * 4 * 3,
    ) else {
        return;
    };
    for (name, stats) in &run.stats {
        // Each scene has tiles drawn in several texture layers and blended
        // overlay edges, so both paths are exercised.
        assert!(
            stats.layered > 100 && stats.blended > 100,
            "{name}: {stats:?}"
        );
    }
}

#[test]
fn walls_and_scenery_shade_the_floor_as_in_the_original() {
    let Some(run) = reproduces(
        |name| name == "shadow" || name.starts_with("light"),
        4 * 5 + 1,
    ) else {
        return;
    };
    for (name, stats) in &run.stats {
        assert!(stats.shaded > 500, "{name}: {stats:?}");
    }
}

/// Standard-detail lighting (`Preferences.highDetailLighting` off): the
/// palette at the brightness preferences 1, 2 and 4, `buildScene`'s other
/// `tileLightness` loop under two light positions for every scene and level,
/// and the floor mesh that lightness and the preference 4 palette give.
#[test]
fn the_floor_without_high_detail_lighting_is_the_originals() {
    let Some(run) = reproduces(
        |name| name.starts_with("sd") || name.starts_with("brightness"),
        4 * (2 * 4 + 4 + 4) + 3,
    ) else {
        return;
    };
    // The baked lightness is not the high-detail one in disguise.
    for scene in run.stats.keys() {
        for level in 0..4 {
            assert_ne!(
                run.digests[&format!("{scene}/sdlight0_{level}")],
                run.digests[&format!("{scene}/light{level}")]
            );
        }
        assert_ne!(
            run.digests[&format!("{scene}/sdlight0_0")],
            run.digests[&format!("{scene}/sdlight1_0")]
        );
        assert_ne!(
            run.digests[&format!("{scene}/sdplain0")],
            run.digests[&format!("{scene}/plain0")]
        );
    }
}

#[test]
fn mergeable_scenery_shares_normals_as_in_the_original() {
    let Some(run) = reproduces(|name| name == "merge", 4) else {
        return;
    };
    for (name, stats) in &run.stats {
        assert!(
            stats.merged > 50 && stats.merged <= stats.mergeable,
            "{name}: {stats:?}"
        );
    }
}
