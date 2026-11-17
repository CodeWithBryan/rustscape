#[path = "../../../oracle/animator/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/animator/oracle.jsonl");
#[test]
fn complete_original_animation_definitions_and_animator_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        let a = actual.as_array().unwrap();
        let e = row["expected"].as_array().unwrap();
        assert_eq!(a.len(), e.len());
        for (step, (a, e)) in a.iter().zip(e).enumerate() {
            assert_eq!(
                a, e,
                "animator case {i} step {step}: {}",
                row["actions"][step]
            );
        }
    }
}
#[test]
fn animator_sources_and_corpus_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/animator/provenance.json")).unwrap();
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

use rs09_server::{
    definitions::{animation as defs, item::Shared},
    world::{
        animation::{Animation, Graphics, Priority},
        appearance,
        doors::Location,
        player_render::Player,
        prepared_animation,
        prepared_masks::PreparedMasks,
    },
};
struct Files;
impl defs::Source for Files {
    fn file(&mut self, _: &defs::Registry, _: i32, _: i32) -> defs::Result<Option<Vec<u8>>> {
        Err(defs::Error::Binding(
            "this ownership test uses explicit definitions".into(),
        ))
    }
}
fn owner(
    world: &prepared_animation::World<Files>,
    index: i32,
) -> (
    PreparedMasks,
    rs09_server::world::animation::Animator<prepared_animation::Owner>,
) {
    let masks = PreparedMasks::new(
        Shared::new(Player::new(index, Location::new(3207, 3216, 0), 2000)),
        Shared::new(appearance::Data::default()),
    );
    let animator = world.factory.actor(masks.clone());
    (masks, animator)
}
fn duration(world: &prepared_animation::World<Files>, id: i32, cycles: i32) -> defs::Handle {
    let d = Shared::new(defs::Definition {
        durations: Some(Shared::new(vec![cycles])),
        ..defs::Definition::default()
    });
    world
        .definitions
        .entries
        .write()
        .insert(id, Some(d.clone()));
    d
}
#[test]
fn owned_animator_enforces_priority_and_clock_before_actual_mask_preparation() {
    let mut world = prepared_animation::World::new(Files);
    world.ticks = 100;
    duration(&world, 1, 90);
    let (masks, a) = owner(&world, 1);
    let high = Animation::with(1, 200, Some(Priority::High));
    assert!(
        a.animate(
            &mut world,
            Some(high.clone()),
            Some(Graphics::new(111, 128, 60))
        )
        .unwrap()
    );
    assert_eq!(a.deadline(), 103);
    assert_eq!(a.owner().unwrap().clocks().read()[1], 103);
    assert!(
        !a.animate(
            &mut world,
            Some(Animation::new(535)),
            Some(Graphics::new(369, 0, 0))
        )
        .unwrap()
    );
    assert_eq!(a.graphics().unwrap().id(), 111);
    // The retained mutable context is read when writing the actual wire mask.
    high.set_id(863);
    masks.prepare().unwrap();
    let mut wire = vec![];
    masks.write(masks.owner(), &mut wire, false, false).unwrap();
    let mut expected = vec![0x18, 1];
    expected.extend(appearance::animation(863, 200));
    expected.extend(
        appearance::SpotAnimation {
            id: 111,
            height: 128,
            delay: 60,
        }
        .encode(),
    );
    assert_eq!(wire, expected);
    // Animator ignores a false mask registration result during the render phase.
    world.ticks = 104;
    let next = Animation::new(535);
    assert!(a.animate(&mut world, Some(next.clone()), None).unwrap());
    assert_eq!(a.deadline(), 105);
    assert!(a.animation().unwrap().same(&next));
    let mut same = vec![];
    masks.write(masks.owner(), &mut same, false, false).unwrap();
    assert_eq!(same, wire);
    masks.reset();
    assert!(!masks.status().update_required());
}
#[test]
fn rejected_reset_clears_clock_but_keeps_the_current_context_and_priority() {
    let mut world = prepared_animation::World::new(Files);
    world.ticks = 100;
    duration(&world, 1, 90);
    let (_, a) = owner(&world, 1);
    let high = Animation::with(1, 0, Some(Priority::VeryHigh));
    a.animate(&mut world, Some(high.clone()), None).unwrap();
    a.reset(&mut world).unwrap();
    assert_eq!(a.deadline(), 0);
    assert_eq!(a.owner().unwrap().clocks().read()[1], 0);
    assert!(a.animation().unwrap().same(&high));
    assert_eq!(a.priority(), Some(Priority::VeryHigh));
    assert!(!a.is_animating(&mut world).unwrap());
}
#[test]
fn reset_context_is_shared_within_its_world_and_cached_definition_survives_id_change() {
    let mut world = prepared_animation::World::new(Files);
    world.ticks = 10;
    let d = duration(&world, 1, 90);
    let (_, a) = owner(&world, 1);
    let (_, b) = owner(&world, 2);
    let reset = world.factory.resets().animator_reset();
    reset.set_id(1);
    a.stop(&mut world).unwrap();
    reset.set_id(2);
    d.read().durations.as_ref().unwrap().write()[0] = 60;
    b.stop(&mut world).unwrap();
    assert!(a.animation().unwrap().same(&b.animation().unwrap()));
    assert_eq!(a.animation().unwrap().id(), 2);
    assert!(reset.cached_definition().unwrap().same(&d));
    assert_eq!(b.deadline(), 12);
    let foreign = prepared_animation::World::new(Files);
    assert!(!foreign.factory.resets().animator_reset().same(&reset));
}
#[test]
fn captured_animator_retains_exact_old_owner_across_wire_index_reuse_and_releases_it() {
    let mut world = prepared_animation::World::new(Files);
    let (old_masks, a) = owner(&world, 1);
    let weak = old_masks.owner().downgrade();
    let (replacement, b) = owner(&world, 1);
    let old_clock = a.owner().unwrap().clocks().clone();
    drop(old_masks);
    a.animate(&mut world, Some(Animation::new(535)), None)
        .unwrap();
    assert_eq!(old_clock.read()[1], 1);
    assert_eq!(b.owner().unwrap().clocks().read()[1], 0);
    assert!(!replacement.status().update_required());
    assert!(weak.upgrade().is_some());
    drop(a);
    assert!(weak.upgrade().is_none());
}
#[test]
fn foreign_animation_world_is_rejected_without_resolving_a_replacement_actor() {
    let world = prepared_animation::World::new(Files);
    let (_, a) = owner(&world, 1);
    let mut foreign = prepared_animation::World::new(Files);
    assert!(
        a.animate(&mut foreign, Some(Animation::new(535)), None)
            .is_err()
    );
    assert!(a.animation().is_none());
    assert_eq!(a.owner().unwrap().clocks().read()[1], 0);
}
