#[path = "../../../oracle/prepared-masks/native.rs"]
mod native;
use rs09_server::{
    definitions::item::Shared,
    world::{
        appearance,
        doors::Location,
        player_render::Player,
        player_walk::State,
        prepared_masks::{FaceTarget, PreparedMasks},
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/prepared-masks/oracle.jsonl");
#[test]
fn original_entity_facing_and_concrete_flag_bytes_match_the_owned_binding() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        for (step, (a, e)) in actual
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(
                a, e,
                "prepared mask case {i} step {step}: {}",
                row["actions"][step]
            );
        }
    }
}
#[test]
fn prepared_mask_source_and_corpus_pins_match() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/prepared-masks/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
fn masks() -> PreparedMasks {
    PreparedMasks::new(
        Shared::new(Player::new(1, Location::new(3207, 3216, 0), 2000)),
        Shared::new(appearance::Data::default()),
    )
}
#[test]
fn retained_mask_capture_uses_the_same_body_after_wire_index_reuse() {
    let old = masks();
    let owner = old.owner();
    let weak = owner.downgrade();
    old.sync_appearance(|| 7).unwrap();
    old.prepare().unwrap();
    old.reset();
    let replacement = masks();
    replacement.body().write().username = "replacement".into();
    old.body().write().username = "retired".into();
    drop(owner);
    assert!(weak.upgrade().is_some());
    let mut bytes = vec![];
    old.write(replacement.owner(), &mut bytes, true, true)
        .unwrap();
    let mut expected = vec![4];
    expected.extend(old.body().read().encode());
    assert_eq!(bytes, expected);
    assert_eq!(old.status().appearance_stamp, 7);
    assert!(!old.owner().same(&replacement.owner()));
    drop(old);
    assert!(weak.upgrade().is_none());
}
#[test]
fn face_target_keeps_the_actual_canonical_node_until_current_and_synced_slots_release_it() {
    let masks = masks();
    let mut world = State::default();
    let target = world.add(2, Location::new(3208, 3216, 0), [1, 2, 3, 4]);
    let view = Shared::new(Player::new(2, Location::new(3208, 3216, 0), 2000));
    let weak = view.downgrade();
    masks
        .face_entity(Some(FaceTarget::Player(
            world.retain_node(target).unwrap(),
            view,
        )))
        .unwrap();
    world.actor_mut(target).unwrap().connected = false;
    world.collect();
    assert!(world.actor(target).is_ok());
    masks.prepare().unwrap();
    assert!(!masks.face_entity(None).unwrap());
    world.collect();
    assert!(world.actor(target).is_ok());
    assert!(weak.upgrade().is_some());
    masks.reset();
    world.collect();
    assert!(world.actor(target).is_err());
    assert!(weak.upgrade().is_none());
}
#[test]
fn prepared_cycle_rejects_new_animation_then_reset_preserves_only_synced_facing() {
    let masks = masks();
    masks
        .face_location(Some(Location::new(3207, 3216, 0)))
        .unwrap();
    masks.animate(535, 0).unwrap();
    masks.prepare().unwrap();
    assert!(!masks.animate(-1, 0).unwrap());
    let mut first = vec![];
    masks
        .write(masks.owner(), &mut first, false, false)
        .unwrap();
    assert_eq!(first, [0x48, 2, 23, 0, 25, 15, 161, 25]);
    masks.reset();
    let mut next = vec![];
    masks.write(masks.owner(), &mut next, false, true).unwrap();
    assert_eq!(next, [0x40, 25, 15, 161, 25]);
    assert!(!masks.owner().read().masks.required);
    assert!(masks.owner().read().masks.synced);
}
