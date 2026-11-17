#[path = "../../../oracle/players/native.rs"]
mod native;
use rs09_protocol::game::{Frame, players::State};
use serde_json::Value;
#[test]
fn player_oracle_sources_match_pinned_reference() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for data in [
        include_str!("../../../fixtures/players/oracle.json"),
        include_str!("../../../fixtures/players/appearance.json"),
        include_str!("../../../fixtures/players/session.json"),
        include_str!("../../../fixtures/players/animation.json"),
        include_str!("../../../fixtures/players/motion.json"),
        include_str!("../../../fixtures/players/destination.json"),
        include_str!("../../../fixtures/players/wheel.json"),
        include_str!("../../../fixtures/players/npcs.json"),
        include_str!("../../../fixtures/players/npc-definitions.json"),
        include_str!("../../../fixtures/players/overheads.json"),
        include_str!("../../../fixtures/players/morph.json"),
        include_str!("../../../fixtures/players/npc-assets/oracle.json"),
        include_str!("../../../fixtures/players/sound.json"),
        include_str!("../../../fixtures/players/blend.json"),
    ] {
        let fixture: Value = serde_json::from_str(data).unwrap();
        for (name, hash) in fixture["sources"].as_object().unwrap() {
            let filename = if name.ends_with(".java") {
                name.clone()
            } else {
                format!("{name}.java")
            };
            let bytes = std::fs::read(root.join(format!(
                "reference/client/client/src/main/java/rt4/{filename}"
            )))
            .unwrap();
            let normalized = bytes
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(normalized)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}
#[test]
fn original_incremental_player_packets_membership_queues_and_masks_match() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/oracle.json")).unwrap();
    for (i, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let mut state = State::new(
            case["start"][0].as_i64().unwrap() as i32,
            case["start"][1].as_i64().unwrap() as i32,
        );
        for (j, packet) in case["packets"].as_array().unwrap().iter().enumerate() {
            let cycle = packet["cycle"].as_i64().unwrap() as i32;
            let frame = Frame {
                opcode: 225,
                payload: native::bytes(packet["wire"].as_str().unwrap()),
            };
            let update = state
                .decode_frame(&frame, cycle)
                .unwrap_or_else(|e| panic!("scenario{i} packet{j}: {e}"));
            assert_eq!(
                native::snapshot(&state, &update, cycle),
                packet["expected"],
                "scenario{i} packet{j}"
            );
        }
    }
}
#[test]
fn full_appearance_fields_equipment_mapping_and_palette_bounds_match_original() {
    use rs09_protocol::{
        Result,
        game::players::{AppearanceLookup, RawAppearance},
    };
    struct Lookup;
    impl AppearanceLookup for Lookup {
        fn equipment_id(&self, index: usize) -> Result<i32> {
            Ok(1000 + index as i32)
        }
        fn item_team(&self, id: i32) -> Result<i32> {
            Ok(id % 7)
        }
        fn body_color_count(&self, index: usize) -> usize {
            index + 5
        }
    }
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/appearance.json")).unwrap();
    for (i, c) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let raw = RawAppearance::decode(&native::bytes(c["wire"].as_str().unwrap())).unwrap();
        let appearance = raw.resolve(&Lookup).unwrap();
        assert_eq!(
            serde_json::json!([
                serde_json::json!([
                    raw.size() as i64,
                    raw.title() as i64,
                    raw.skull_icon as i64,
                    raw.prayer_icon as i64,
                    appearance.team as i64,
                    raw.bas as i64,
                    raw.name37,
                    raw.combat as i64,
                    raw.skill as i64,
                    raw.combat_with_summoning as i64,
                    raw.combat_range as i64,
                    raw.sound_radius as i64,
                    i64::from(raw.female()),
                    raw.npc_id as i64,
                    raw.consumed as i64
                ]),
                serde_json::json!(appearance.identikit),
                serde_json::json!(appearance.colors),
                serde_json::json!(raw.sounds)
            ]),
            c["expected"],
            "appearance case{i}"
        );
    }
}
