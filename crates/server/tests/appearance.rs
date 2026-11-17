use rs09_server::world::appearance::Data;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/appearance/oracle.jsonl");
#[test]
fn appearance_bytes_and_naked_body_defaults_match_original() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if c["kind"] == "spot" {
            let spot = rs09_server::world::appearance::SpotAnimation {
                id: c["id"].as_i64().unwrap() as i32,
                height: c["height"].as_i64().unwrap() as i32,
                delay: c["delay"].as_i64().unwrap() as i32,
            };
            let bytes: String = spot.encode().iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(bytes, c["expected"].as_str().unwrap(), "spot case{i}");
            continue;
        }
        if c["kind"] == "audio" {
            let number = |key: &str| c[key].as_i64().unwrap() as i32;
            let frame = if let Some(loc) = c["location"].as_array() {
                rs09_protocol::game::updates::area_sound(
                    loc[0].as_i64().unwrap() as i32,
                    loc[1].as_i64().unwrap() as i32,
                    number("id"),
                    number("radius"),
                    number("loops"),
                    number("delay"),
                )
            } else {
                rs09_protocol::game::updates::synth_sound(
                    number("id"),
                    number("loops"),
                    number("delay"),
                )
            };
            let mut bytes = vec![];
            frame
                .write(
                    rs09_protocol::game::Direction::ServerToClient,
                    None,
                    &mut bytes,
                )
                .unwrap();
            let bytes: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(bytes, c["expected"].as_str().unwrap(), "audio case {i}");
            continue;
        }
        if c["kind"] == "animation" {
            let bytes: String = rs09_server::world::appearance::animation(
                c["id"].as_i64().unwrap() as i32,
                c["delay"].as_i64().unwrap() as i32,
            )
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
            assert_eq!(bytes, c["expected"].as_str().unwrap(), "animation case {i}");
            continue;
        }
        let mut data: Data = serde_json::from_value(c.clone()).unwrap();
        if c.get("bodyParts").is_none() {
            data.body_parts = Data::naked(data.username.clone(), data.gender).body_parts;
        }
        let bytes: String = data.encode().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            bytes,
            c["expected"].as_str().unwrap(),
            "appearance case {i}"
        );
    }
}
#[test]
fn appearance_fixture_sources_and_bytes_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/appearance/provenance.json")).unwrap();
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
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
