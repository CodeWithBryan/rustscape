use rs09_client::{
    ambient::Host,
    ambient_npc::{Actor, Collection, Definition, Source},
    walking::PlayerBas,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Default)]
struct Mixer {
    streams: BTreeMap<u64, [i32; 5]>,
    events: Vec<Vec<i32>>,
    next: u64,
}
impl Host for Mixer {
    fn start(&mut self, id: i32, volume: i32, loops: i32) -> Option<u64> {
        if id % 11 == 0 {
            return None;
        }
        self.next += 1;
        self.streams
            .insert(self.next, [self.next as i32, id, volume, loops, 1]);
        self.events
            .push(vec![0, self.next as i32, id, volume, loops]);
        Some(self.next)
    }
    fn volume(&mut self, id: u64, volume: i32) {
        self.streams.get_mut(&id).unwrap()[2] = volume;
        self.events.push(vec![1, id as i32, volume]);
    }
    fn stop(&mut self, id: u64) {
        self.streams.get_mut(&id).unwrap()[4] = 0;
        self.events.push(vec![2, id as i32]);
    }
    fn linked(&self, id: u64) -> bool {
        self.streams[&id][4] != 0
    }
}
fn numbers(v: &Value) -> Vec<i32> {
    serde_json::from_value(v.clone()).unwrap()
}
fn bas(v: &Value) -> PlayerBas {
    let n = numbers(v);
    PlayerBas {
        idle: n[0],
        walk: n[1],
        run: n[2],
        stand: [n[3], n[4]],
        turns: [n[5], n[6], n[7]],
        slow: n[8],
        slow_turns: [n[9], n[10], n[11]],
        run_turns: [n[12], n[13], n[14]],
        ..PlayerBas::default()
    }
}
fn definition(v: &Value) -> Option<Definition> {
    if v.is_null() {
        return None;
    }
    let n = numbers(v);
    Some(Definition {
        size: n[0],
        radius: n[1] * 128,
        sounds: [n[2], n[3], n[4], n[5]],
    })
}
#[test]
fn every_cached_npc_sound_radius_matches_original_without_changing_prior_metadata() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio/npc-metadata.json")).unwrap();
    let original = std::fs::read(root.join(fixture["input"]["path"].as_str().unwrap())).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&original)),
        fixture["input"]["sha256"].as_str().unwrap()
    );
    let prior: Value = serde_json::from_slice(&original).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 8590);
    for (case, radius) in prior["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(fixture["cases"].as_array().unwrap())
    {
        assert_eq!(case["id"], radius["id"]);
        let hex = case["wire"].as_str().unwrap();
        let bytes: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let definition = rs09_client::npc::Definition::decode(&bytes).unwrap();
        assert_eq!(
            json!(definition.sound_radius),
            radius["radius"],
            "NPC {}",
            case["id"]
        );
        assert_eq!(json!(definition.sounds), case["expected"][13]);
    }
}
#[test]
fn original_npc_ambient_identity_morph_movement_gain_and_collection_order_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio/npc.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in fixture["sources"].as_object().unwrap() {
        let bytes = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            hash.as_str().unwrap()
        );
    }
    for (scenario, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut collection = Collection::default();
        let mut mixer = Mixer::default();
        let mut actors = BTreeMap::<i32, Actor>::new();
        let mut identities = Vec::new();
        for (step, c) in case["commands"].as_array().unwrap().iter().enumerate() {
            let key = || c[1].as_i64().unwrap() as i32;
            match c[0].as_str().unwrap() {
                "N" => {
                    let n = numbers(&c[2]);
                    let actor = Rc::new(RefCell::new(Source {
                        type_id: key(),
                        fine: [n[0], n[1]],
                        size: n[2],
                        sequence: n[3],
                        multi: n[4] != 0,
                        bas: bas(&c[3]),
                        resolved: definition(&c[4]),
                    }));
                    identities.push(actor.clone());
                    actors.insert(key(), actor);
                }
                "P" => {
                    let n = numbers(&c[2]);
                    let mut actor = actors[&key()].borrow_mut();
                    actor.fine = [n[0], n[1]];
                    actor.size = n[2];
                    actor.sequence = n[3];
                    actor.bas = bas(&c[3]);
                }
                "M" => actors[&key()].borrow_mut().resolved = definition(&c[2]),
                "A" => {
                    let n = numbers(&c[2]);
                    collection.add(actors[&key()].clone(), [n[0], n[1]], n[2]);
                }
                "D" => collection.remove(&actors[&key()], &mut mixer),
                "U" => collection.update_multi(&mut mixer),
                "C" => {
                    if key() != 0 {
                        collection.clear(&mut mixer);
                    }
                }
                "F" => {
                    if let Some(stream) = mixer.streams.get_mut(&(key() as u64)) {
                        stream[4] = 0;
                    }
                }
                "R" => {
                    let n = numbers(&c[1]);
                    collection.redraw([n[0], n[1]], n[2], n[3], n[4], &mut mixer);
                }
                _ => unreachable!(),
            }
            let entries: Vec<_> = collection.entries.iter().map(|entry| {
                let id = identities.iter().position(|n| Rc::ptr_eq(n, &entry.actor)).unwrap()+1;
                let e = &entry.emitter;
                json!({"actor":id,"fields":[e.min[0],e.min[1],e.max[0],e.max[1],e.plane,e.definition.sound,e.definition.radius,entry.movement_speed,i32::from(e.multi)],"primary":e.primary.map(|id|mixer.streams[&id])})
            }).collect();
            assert_eq!(
                json!({"entries":entries,"events":mixer.events}),
                case["expected"][step],
                "scenario {scenario} step {step}: {c}"
            );
            mixer.events.clear();
        }
    }
}

#[test]
fn emitter_references_retain_old_actor_identity_and_release_without_cycles() {
    let actor = Rc::new(RefCell::new(Source {
        type_id: 0,
        fine: [0; 2],
        size: 1,
        sequence: -1,
        bas: PlayerBas::default(),
        multi: false,
        resolved: None,
    }));
    let weak = Rc::downgrade(&actor);
    let mut collection = Collection::default();
    let mut mixer = Mixer::default();
    collection.add(actor.clone(), [0; 2], 0);
    collection.add(actor.clone(), [0; 2], 0);
    collection.remove(&actor, &mut mixer);
    assert_eq!(collection.entries.len(), 1);
    drop(actor);
    assert!(weak.upgrade().is_some());
    collection.clear(&mut mixer);
    assert!(weak.upgrade().is_none());
}
