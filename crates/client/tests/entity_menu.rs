use rs09_client::entity_menu::{self, Actor, Kind};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn original_overlap_sources_remain_pinned() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/entity-menu.json")).unwrap();
    for (name, expected) in corpus["sources"].as_object().unwrap() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../reference/client/client/src/main/java/rt4/{name}"
        ));
        let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected.as_str().unwrap()
        );
    }
}

#[test]
fn original_actor_scene_keys_preserve_zero_coordinates_and_npc_sign() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/entity-menu.json")).unwrap();
    for case in corpus["producerKeys"].as_array().unwrap() {
        let kind = if case["kind"].as_i64().unwrap() == 0 {
            Kind::Player
        } else {
            Kind::Npc
        };
        let key = entity_menu::actor_key(
            kind,
            case["index"].as_u64().unwrap() as usize,
            case["interactive"].as_bool().unwrap(),
        );
        assert_eq!(format!("{key:016x}"), case["expected"].as_str().unwrap());
        assert_eq!(key & 0x3fff, 0);
        assert_eq!(
            entity_menu::pickable(key),
            case["pickable"].as_bool().unwrap()
        );
    }
}

#[test]
fn original_actor_overlap_order_alignment_sizes_and_wrapping() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/entity-menu.json")).unwrap();
    let actors = |list: &Value| {
        list.as_array()
            .unwrap()
            .iter()
            .map(|a| Actor {
                index: a[0].as_u64().unwrap() as usize,
                fine: [a[1].as_i64().unwrap() as i32, a[2].as_i64().unwrap() as i32],
                size: a[3].as_i64().unwrap() as i32,
            })
            .collect::<Vec<_>>()
    };
    for (i, c) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let npcs = actors(&c["npcs"]);
        let players = actors(&c["players"]);
        let keys = c["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| u64::from_str_radix(k.as_str().unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        let actual = entity_menu::expand(&keys, &npcs, &players)
            .into_iter()
            .map(|hit| {
                vec![
                    i64::from(hit.kind == Kind::Npc),
                    hit.index as i64,
                    i64::from(hit.tile[0]),
                    i64::from(hit.tile[1]),
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            c["expected"],
            "case{i}:{}",
            c["label"]
        );
    }
}

#[test]
fn missing_picked_actor_does_not_dispatch_a_stale_target() {
    assert!(entity_menu::expand(&[entity_menu::key(Kind::Npc, 7, [10, 20])], &[], &[]).is_empty());
    assert!(
        entity_menu::expand(&[entity_menu::key(Kind::Player, 7, [10, 20])], &[], &[]).is_empty()
    );
}
