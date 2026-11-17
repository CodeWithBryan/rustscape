use rs09_client::{pack::Pack, texture};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fixture() -> Value {
    serde_json::from_slice(&std::fs::read(root().join("fixtures/visual/textures.json")).unwrap())
        .unwrap()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[test]
fn texture_fixtures_pin_original_sources_and_raw_cache_assets() {
    let f = fixture();
    let lock: Value =
        serde_json::from_slice(&std::fs::read(root().join("upstream.lock.json")).unwrap()).unwrap();
    assert_eq!(f["client_commit"], lock["client"]["commit"]);
    assert_eq!(
        hash(&std::fs::read(root().join("fixtures/visual/textures.pack")).unwrap()),
        f["pack_sha256"].as_str().unwrap()
    );
    for (name, expected) in f["sources"].as_object().unwrap() {
        let p = root()
            .join("reference/client/client/src/main/java/rt4")
            .join(name);
        if p.exists() {
            let text = std::fs::read_to_string(p).unwrap().replace("\r\n", "\n");
            assert_eq!(
                hash(text.as_bytes()),
                expected.as_str().unwrap(),
                "reference {name}"
            );
        }
    }
}
#[test]
fn all_680_cache_texture_graphs_match_jvm_rgba_at_64_and_128_pixels() {
    let f = fixture();
    let pack =
        Pack::decode(std::fs::read(root().join("fixtures/visual/textures.pack")).unwrap()).unwrap();
    assert_eq!(f["cases"].as_array().unwrap().len(), 1360);
    for c in f["cases"].as_array().unwrap() {
        let id = c["id"].as_i64().unwrap() as i32;
        let size = c["size"].as_u64().unwrap() as usize;
        let bytes = texture::generate(&pack, id, size).unwrap();
        assert_eq!(
            hash(&bytes),
            c["expected"].as_str().unwrap(),
            "material {id}, {size}x{size}"
        );
    }
}
