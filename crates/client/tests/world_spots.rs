use rs09_client::{
    animation,
    world_spot::{Context, State},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn snapshot(s: &State) -> Value {
    json!(
        s.effects
            .iter()
            .map(|e| [
                e.id,
                e.plane,
                e.fine[0],
                e.fine[1],
                e.z,
                e.start,
                e.frame as i32,
                e.clock,
                i32::from(e.finished)
            ])
            .collect::<Vec<_>>()
    )
}
#[test]
fn original_packet_placement_clocks_and_scene_lifetime_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value =
        serde_json::from_slice(&fs::read(root.join("fixtures/ui/world-spots.json")).unwrap())
            .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        use sha2::{Digest, Sha256};
        let b = fs::read(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap();
        let b = String::from_utf8(b).unwrap().replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(b.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 640);
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let mut s = State {
            zone: serde_json::from_value(c["zone"].clone()).unwrap(),
            ..State::default()
        };
        let delays: Vec<u16> = serde_json::from_value(c["delays"].clone()).unwrap();
        let seq = animation::Sequence {
            frames: vec![0; delays.len()],
            delays,
            ..Default::default()
        };
        if let Some(cached) = c.get("cached").filter(|v| !v.is_null()) {
            use sha2::{Digest, Sha256};
            let raw = hex(cached["sequenceRaw"].as_str().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&raw)),
                cached["sequenceSha256"].as_str().unwrap()
            );
            let actual = animation::Sequence::decode(&raw).unwrap();
            assert_eq!(actual.delays, seq.delays);
            let spot = hex(cached["spotRaw"].as_str().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&spot)),
                cached["spotSha256"].as_str().unwrap()
            );
            assert_eq!(
                rs09_client::spot::Definition::decode(&spot)
                    .unwrap()
                    .0
                    .sequence,
                n(&c["sequence"])
            );
        }
        let sequences = BTreeMap::from([(n(&c["sequence"]) as u32, seq)]);
        let bytes = hex(c["payload"].as_str().unwrap());
        let mut reader = rs09_protocol::buffer::Reader::new(&bytes);
        let id = if n(&c["opcode"]) == 17 {
            reader.g1().unwrap();
            reader.g2().unwrap()
        } else {
            reader.take(8).unwrap();
            reader.ig2add().unwrap()
        };
        let spots = BTreeMap::from([(id, n(&c["sequence"]))]);
        let mut heights = vec![];
        let mut height = |p: i32, f: [i32; 2]| {
            heights.push([p, f[0], f[1]]);
            p * 97 + f[0] / 17 - f[1] / 13 - 400
        };
        let payload = bytes;
        s.receive(
            &rs09_protocol::game::Frame {
                opcode: n(&c["opcode"]) as u8,
                payload,
            },
            Context {
                origin: serde_json::from_value(c["origin"].clone()).unwrap(),
                plane: n(&c["plane"]),
                cycle: n(&c["cycle"]),
            },
            &spots,
            &sequences,
            &mut height,
        )
        .unwrap();
        assert_eq!(
            json!([snapshot(&s), [], heights]),
            c["expected"][0],
            "case{i}receive id{id}"
        );
        for (k, u) in c["updates"].as_array().unwrap().iter().enumerate() {
            s.update(n(&u[0]), n(&u[1]), n(&u[2]), &sequences).unwrap();
            let draws: Vec<_> = s
                .visible(n(&u[1]))
                .map(|e| [e.plane, e.fine[0], e.fine[1], e.z, 60, 0, -1, 0])
                .collect();
            assert_eq!(
                json!([snapshot(&s), draws, heights]),
                c["expected"][k + 1],
                "case{i}update{k}"
            );
        }
    }
    println!("PASS640originalcases5120snapshots");
}

fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn enclosed_zone_effects_keep_order_captured_height_and_independent_lifetimes() {
    use rs09_client::animation::Sequence;
    use rs09_protocol::game::Frame;
    let spots = BTreeMap::from([(184, 7)]);
    let sequences = BTreeMap::from([(
        7,
        Sequence {
            frames: vec![1, 2],
            delays: vec![2, 3],
            ..Default::default()
        },
    )]);
    let mut state = State::default();
    let context = Context {
        origin: [3168, 3168],
        plane: 0,
        cycle: 100,
    };
    let mut calls = vec![];
    // Enclosed origin48/48 followed by two independent original17 requests.
    let frame = Frame {
        opcode: 230,
        payload: vec![
            176, 80, 17, 0x12, 0, 184, 20, 0, 0, 17, 0x12, 0, 184, 40, 0, 4,
        ],
    };
    state
        .receive(&frame, context, &spots, &sequences, &mut |p, f| {
            calls.push((p, f));
            -400
        })
        .unwrap();
    assert_eq!(state.effects.len(), 2);
    assert_eq!(
        state
            .effects
            .iter()
            .map(|e| (e.z, e.start))
            .collect::<Vec<_>>(),
        vec![(-420, 100), (-440, 104)]
    );
    assert_eq!(calls, vec![(0, [6336, 6464]); 2]);
    state.update(0, 100, 2, &sequences).unwrap();
    assert_eq!(state.visible(100).count(), 1);
    assert_eq!(state.effects[0].frame, 0); // Equality does not advance.
    state.update(0, 104, 4, &sequences).unwrap();
    assert_eq!(state.effects.len(), 1);
    assert_eq!(
        (
            state.effects[0].z,
            state.effects[0].frame,
            state.effects[0].clock
        ),
        (-440, 1, 2)
    );
    state.update(1, 105, 1, &sequences).unwrap();
    assert!(state.effects.is_empty());
}

#[test]
fn bounded_effect_overflow_and_malformed_packets_are_explicit() {
    use rs09_protocol::game::Frame;
    let mut state = State::default();
    let context = Context {
        origin: [0; 2],
        plane: 0,
        cycle: 0,
    };
    let spots = BTreeMap::from([(184, -1)]);
    let mut heights = 0;
    let mut height = |_, _| {
        heights += 1;
        0
    };
    let valid = Frame {
        opcode: 17,
        payload: vec![0, 0, 184, 0, 0, 0],
    };
    for _ in 0..1024 {
        state
            .receive(&valid, context, &spots, &BTreeMap::new(), &mut height)
            .unwrap();
    }
    assert!(
        state
            .receive(&valid, context, &spots, &BTreeMap::new(), &mut height)
            .is_err()
    );
    let short = Frame {
        opcode: 17,
        payload: vec![0],
    };
    assert!(
        state
            .receive(&short, context, &spots, &BTreeMap::new(), &mut height)
            .is_err()
    );
    assert_eq!(heights, 1024);
    assert_eq!(state.effects.len(), 1024);
    state.update(0, 0, 0, &BTreeMap::new()).unwrap();
    assert!(state.effects.is_empty()); // Original sequence-1 effects retire before drawing.
}
