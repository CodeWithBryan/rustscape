use rs09_client::ambient::{Definition, Emitter, Host};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[test]
fn original_varp_poll_returns_preserve_intermediate_active_values_for_morphs() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio/varps.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in f["sources"].as_object().unwrap() {
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
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let mut pending = rs09_client::ui::varps::Pending::default();
        let mut active = BTreeMap::new();
        let mut now = 1000i64;
        let mut poll = 0;
        for command in c["commands"].as_array().unwrap() {
            let command = command.as_array().unwrap();
            let number = |i: usize| command[i].as_i64().unwrap() as i32;
            match command[0].as_str().unwrap() {
                "V" => pending.server(number(1), number(2), now),
                "L" => {
                    active.insert(number(1), number(2));
                    pending.client(number(1), now);
                }
                "T" => now += i64::from(number(1)),
                "P" => {
                    let mut observed = vec![];
                    let changed = pending.poll_with(&mut active, now, |id, values| {
                        let bytes = (0..64)
                            .flat_map(|id| values.get(&id).copied().unwrap_or(0).to_be_bytes())
                            .collect::<Vec<_>>();
                        observed.push(json!([id, format!("{:x}", Sha256::digest(bytes))]));
                    });
                    assert_eq!(json!(observed), c["expected"][poll], "case{i} poll{poll}");
                    assert_eq!(
                        changed,
                        observed
                            .iter()
                            .map(|v| v[0].as_i64().unwrap() as i32)
                            .collect::<Vec<_>>()
                    );
                    poll += 1;
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(poll, 32);
    }
}
#[test]
fn original_real_location_audio_fields_and_child_sound_presence_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio/metadata.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in f["sources"].as_object().unwrap() {
        let name = if name.ends_with(".java") {
            name.clone()
        } else {
            format!("{name}.java")
        };
        let bytes =
            std::fs::read(root.join(format!("reference/client/client/src/main/java/rt4/{name}")))
                .unwrap()
                .into_iter()
                .filter(|&v| v != b'\r')
                .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            hash.as_str().unwrap()
        );
    }
    let mut defs = BTreeMap::new();
    for c in f["cases"].as_array().unwrap() {
        let hex = c["raw"].as_str().unwrap();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        defs.insert(
            c["id"].as_i64().unwrap() as i32,
            rs09_client::interaction::LocationType::decode(&bytes).unwrap(),
        );
    }
    for c in f["cases"].as_array().unwrap() {
        let d = &defs[&(c["id"].as_i64().unwrap() as i32)];
        let a = &d.ambient;
        assert_eq!(
            json!([
                a.sound,
                a.radius,
                a.intervals[0],
                a.intervals[1],
                a.sounds,
                d.has_area_sound(|id| defs.get(&id).is_some_and(|d| d.ambient.audible()))
            ]),
            c["expected"],
            "{c}"
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 1274);
}
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
fn definition(c: &Value) -> Definition {
    let n: Vec<i32> = serde_json::from_value(c["fields"].clone()).unwrap();
    Definition {
        sound: n[2],
        radius: n[3] * 128,
        intervals: [n[4], n[5]],
        sounds: serde_json::from_value(c["sounds"].clone()).unwrap(),
    }
}
fn snapshot(e: &Emitter, host: &mut Mixer, random_calls: usize) -> Value {
    let d = &e.definition;
    let result = json!({"fields":[e.min[0],e.min[1],e.max[0],e.max[1],e.plane,d.sound,d.radius,d.intervals[0],d.intervals[1],e.remaining,i32::from(e.multi)],"sounds":d.sounds,"primary":e.primary.map(|id|host.streams[&id]),"secondary":e.secondary.map(|id|host.streams[&id]),"randomCalls":random_calls,"events":host.events});
    host.events.clear();
    result
}
#[test]
fn original_location_ambient_bounds_morph_lifetimes_rng_and_distance_gain_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio/ambient.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in f["sources"].as_object().unwrap() {
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
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let args: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let d = definition(&c["definition"]);
        let n: Vec<i32> = serde_json::from_value(c["definition"]["fields"].clone()).unwrap();
        let mut host = Mixer::default();
        let mut calls = 0;
        let mut emitter = Emitter::location(
            [args[0], args[1]],
            [n[0], n[1]],
            args[3],
            args[2],
            d.clone(),
            (args[4] != 0).then_some(Some(d)),
            || {
                calls += 1;
                c["initialRandom"].as_f64().unwrap()
            },
        );
        assert_eq!(
            snapshot(&emitter, &mut host, calls),
            c["initialExpected"],
            "initial{i}"
        );
        for (j, s) in c["steps"].as_array().unwrap().iter().enumerate() {
            if let Some(update) = s.get("update") {
                emitter.update((!update.is_null()).then(|| definition(update)), &mut host);
            }
            if let Some(finish) = s.get("finish") {
                let finish = finish.as_u64().unwrap();
                for (bit, id) in [(1, emitter.primary), (2, emitter.secondary)] {
                    if finish & bit != 0
                        && let Some(id) = id
                    {
                        host.streams.get_mut(&id).unwrap()[4] = 0;
                    }
                }
            }
            let n: Vec<i32> = serde_json::from_value(s["args"].clone()).unwrap();
            calls = 0;
            emitter.tick([n[0], n[1]], n[2], n[3], n[4], &mut host, || {
                let v = s["random"][calls].as_f64().unwrap();
                calls += 1;
                v
            });
            assert_eq!(
                snapshot(&emitter, &mut host, calls),
                s["expected"],
                "case{i} tick{j}"
            );
        }
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 128);
}
