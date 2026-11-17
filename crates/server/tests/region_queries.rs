#[path = "../../../oracle/region-queries/native.rs"]
mod native;
#[allow(dead_code)]
#[path = "../../../oracle/scene/native.rs"]
mod scene_native;
use rs09_server::{definitions::item::Shared, world::doors::Location};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/region-queries/oracle.jsonl");
#[test]
fn original_region_queries_match_callbacks_aliases_and_failed_prefixes() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "region query case{i}: {line}"
        );
    }
}
#[test]
fn region_query_sources_and_fixture_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/region-queries/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
#[test]
fn owning_scene_loads_landscape_and_uses_absolute_flags_independent_of_chunk_base() {
    use rs09_server::world::scene::{self, Assets};
    struct Underlay(scene_native::MemoryAssets);
    impl Assets for Underlay {
        fn archive_id(&self, name: &str) -> scene::Result<i32> {
            self.0.archive_id(name)
        }
        fn mapscape(&self, _: i32) -> scene::Result<Option<Vec<u8>>> {
            Ok(Some([82, 0].repeat(16384)))
        }
        fn landscape(&self, group: i32, key: [u32; 4]) -> scene::Result<Option<Vec<u8>>> {
            self.0.landscape(group, key)
        }
        fn keys(&self, region: i32) -> scene::Result<[u32; 4]> {
            self.0.keys(region)
        }
    }
    let initial = scene_native::create(&json!({"land":"00000000"}));
    let mut scene = scene::Scene::new(Underlay(initial.assets), initial.definitions, initial.doors);
    let mut services = scene_native::Bindings::default();
    let l = Location::new(3207, 3216, 0);
    assert!(scene.regions.is_empty());
    assert!(scene.context(&mut services).is_landscape(Some(&l)).unwrap());
    assert!(scene.regions[&12850].read().loaded);
    let chunk = scene.chunk(&l).unwrap();
    let [x, y, z] = l.coordinates();
    let key = (z << 22) | ((x >> 3) << 11) | (y >> 3);
    let index = ((x & 7) * 8 + (y & 7)) as usize;
    let mut flags = [0; 64];
    flags[index] = 2;
    scene.flags.insert((key, false), Shared::new(flags));
    chunk.write().flags.base_x = 999;
    chunk.write().flags.base_y = 1000;
    assert_eq!(
        scene
            .context(&mut services)
            .clipping(l.clone(), false)
            .unwrap(),
        2
    );
    assert!(
        scene
            .context(&mut services)
            .teleport_permitted(Some(&l))
            .unwrap()
    );
    assert!(scene.context(&mut services).is_clipped(Some(&l)).unwrap());
    assert_eq!(
        scene
            .context(&mut services)
            .water_clipping(Some(&l))
            .unwrap(),
        2
    );
    chunk.write().flags.landscape = None;
    assert!(
        !scene
            .context(&mut services)
            .teleport_permitted(Some(&l))
            .unwrap()
    );
    assert!(scene.context(&mut services).is_clipped(Some(&l)).unwrap());
    assert!(scene.context(&mut services).is_landscape(None).is_err());
}
#[test]
fn missing_region_short_circuits_without_creating_flag_arrays() {
    let mut scene = scene_native::create(&json!({"land":"00000000","missing":true}));
    let mut services = scene_native::Bindings::default();
    let l = Location::new(3207, 3216, 0);
    assert_eq!(
        scene
            .context(&mut services)
            .clipping(l.clone(), false)
            .unwrap(),
        -1
    );
    assert!(!scene.context(&mut services).is_landscape(Some(&l)).unwrap());
    assert!(scene.flags.is_empty());
    assert!(scene.context(&mut services).is_clipped(Some(&l)).unwrap());
    assert_eq!(
        scene
            .context(&mut services)
            .water_clipping(Some(&l))
            .unwrap(),
        !0x200000
    );
    assert!(scene.flags.is_empty());
}
#[test]
fn empty_flag_maps_preserve_different_walking_and_projectile_defaults() {
    let mut scene = scene_native::create(&json!({"land":"00000000"}));
    let mut services = scene_native::Bindings::default();
    let l = Location::new(3207, 3216, 0);
    scene.context(&mut services).load(12850, None).unwrap();
    scene.flags.clear();
    assert_eq!(
        scene
            .context(&mut services)
            .clipping(l.clone(), false)
            .unwrap(),
        -1
    );
    assert_eq!(scene.context(&mut services).clipping(l, true).unwrap(), 0);
    assert_eq!(scene.flags.len(), 2);
}
