#[path = "../../oracle/game-packets/native.rs"]
mod native;
use rs09_protocol::game::{
    CLIENT_PACKET_SIZES, Decoder, Direction, Frame, SERVER_PACKET_SIZES, Size,
};
use rs09_protocol::isaac::OpcodeCipher;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../fixtures/game-packets/oracle.jsonl");
#[test]
fn absolute_walk_writer_reproduces_canonical_original_decoder_inputs() {
    let mut count = 0;
    for line in CORPUS.lines() {
        let c: Value = serde_json::from_str(line).unwrap();
        let Some(data) = c["data"].as_array() else {
            continue;
        };
        if c["kind"] != "decode"
            || c["opcode"] != 215
            || data.len() != 5
            || !matches!(data[0].as_u64(), Some(128 | 129))
        {
            continue;
        }
        let a = &c["expected"]["action"];
        let frame = rs09_protocol::game::actions::worldspace_walk(
            a["destX"].as_i64().unwrap() as i32,
            a["destY"].as_i64().unwrap() as i32,
            a["isRun"].as_bool().unwrap(),
        );
        assert_eq!(serde_json::json!(frame.payload), c["data"]);
        count += 1;
    }
    assert_eq!(count, 4);
}
#[test]
fn ui_script_payloads_are_bounded_and_truncation_never_produces_an_update() {
    use rs09_protocol::game::updates::{self, ScriptArg, ServerUpdate};
    let frame = updates::run_client_script(
        42,
        b"is",
        &[ScriptArg::Text(b"bank".to_vec()), ScriptArg::Int(-7)],
        65537,
    )
    .unwrap();
    assert_eq!(
        ServerUpdate::decode(&frame).unwrap(),
        Some(ServerUpdate::RunClientScript {
            id: 42,
            counter: 1,
            types: b"is".to_vec(),
            arguments: vec![ScriptArg::Int(-7), ScriptArg::Text(b"bank".to_vec())],
        })
    );
    for end in 0..frame.payload.len() {
        assert!(
            ServerUpdate::decode(&Frame {
                opcode: 115,
                payload: frame.payload[..end].to_vec(),
            })
            .is_err(),
            "truncated script at {end}"
        );
    }
    for (types, arguments) in [
        (b"s".as_slice(), vec![]),
        (b"s".as_slice(), vec![ScriptArg::Int(1)]),
        (b"i".as_slice(), vec![ScriptArg::Text(vec![])]),
        (b"s".as_slice(), vec![ScriptArg::Text(vec![0])]),
        (b"\0".as_slice(), vec![ScriptArg::Int(1)]),
        (b"\xff".as_slice(), vec![ScriptArg::Int(1)]),
    ] {
        assert!(updates::run_client_script(0, types, &arguments, 0).is_err());
    }
    let maximum =
        updates::run_client_script(0, b"s", &[ScriptArg::Text(vec![b'a'; 65526])], 0).unwrap();
    assert_eq!(maximum.payload.len(), 65535);
    assert!(
        updates::run_client_script(0, b"s", &[ScriptArg::Text(vec![b'a'; 65527])], 0,).is_err()
    );
}

#[test]
fn ui_counter_wrapping_small_varc_quirks_and_event_sentinels_are_preserved() {
    use rs09_protocol::game::updates::{self, ServerUpdate};
    for (value, decoded) in [(-1, 255), (i32::MIN, 0), (255, 255), (256, 256)] {
        let frame = updates::varc(192, value, -1);
        assert_eq!(frame.opcode, if value <= 255 { 65 } else { 69 });
        assert_eq!(
            ServerUpdate::decode(&frame).unwrap(),
            Some(ServerUpdate::Varc {
                counter: 65535,
                id: 192,
                value: decoded,
            })
        );
    }
    assert_eq!(
        ServerUpdate::decode(&updates::interface_events(-3, -1, 65535, i32::MIN, 65536)).unwrap(),
        Some(ServerUpdate::InterfaceEvents {
            counter: 0,
            pointer: -3,
            start: -1,
            end: -1,
            mask: i32::MIN,
        })
    );
}
#[test]
fn original_varbit_writer_quirks_remain_distinct_from_client_decoder() {
    use rs09_protocol::{
        buffer::Writer,
        game::updates::{self, ServerUpdate},
    };
    assert_eq!(
        ServerUpdate::decode(&updates::varbit(4893, 128)).unwrap(),
        Some(ServerUpdate::Varbit { id: 4893, value: 0 })
    );
    assert_eq!(
        ServerUpdate::decode(&updates::varbit(4893, 256)).unwrap(),
        Some(ServerUpdate::Varbit {
            id: 5021,
            value: 128
        })
    );
    // A correctly transformed raw large message can represent the full value.
    let mut w = Writer::new();
    w.ip4(i32::MIN);
    w.ip2add(4893);
    assert_eq!(
        ServerUpdate::decode(&Frame {
            opcode: 84,
            payload: w.into_vec()
        })
        .unwrap(),
        Some(ServerUpdate::Varbit {
            id: 4893,
            value: i32::MIN
        })
    );
    for value in [i32::MIN, -129, -128, -1, 0, 127, 128, i32::MAX] {
        assert_eq!(
            ServerUpdate::decode(&updates::varp(1249, value)).unwrap(),
            Some(ServerUpdate::Varp { id: 1249, value })
        );
    }
}
#[test]
fn original_numeric_actions_and_outgoing_packets_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&c),
            c["expected"],
            "game packet case {i}: {c}"
        );
    }
}
#[test]
fn original_packet_tables_and_sources_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../fixtures/game-packets/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    assert_eq!(
        serde_json::to_value(CLIENT_PACKET_SIZES.as_slice()).unwrap(),
        p["clientPacketSizes"]
    );
    assert_eq!(
        serde_json::to_value(SERVER_PACKET_SIZES.as_slice()).unwrap(),
        p["serverPacketSizes"]
    );
    let mut observed: std::collections::BTreeSet<_> = CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|c| c["kind"] == "decode")
        .map(|c| c["opcode"].as_u64().unwrap() as u8)
        .collect();
    // Preserve the numeric baseline; text-prompt coverage lives with the
    // independently pinned modern input comparison.
    let input: std::collections::BTreeSet<_> =
        include_str!("../../fixtures/input-prompt/oracle.jsonl")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter_map(|c| c["decode"]["opcode"].as_u64().map(|v| v as u8))
            .collect();
    assert_eq!(input, [23, 65, 244].into_iter().collect());
    observed.extend(input);
    assert_eq!(
        observed,
        rs09_protocol::game::actions::SUPPORTED_CLIENT_OPCODES
            .iter()
            .copied()
            .collect()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for (path, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
#[test]
fn malformed_nested_zone_updates_fail_before_reaching_client_scene_state() {
    use rs09_protocol::game::updates::{self, ServerUpdate};
    let add = updates::loc_add(3, 5, 0, 2, 1530);
    let enclosed = updates::zone_enclosed(40, 56, std::slice::from_ref(&add)).unwrap();
    assert_eq!(
        ServerUpdate::decode(&enclosed).unwrap(),
        Some(ServerUpdate::ZoneEnclosed {
            x: 40,
            y: 56,
            frames: vec![add]
        })
    );
    for n in [0, 1, 3, 4, 5, 6] {
        assert!(
            ServerUpdate::decode(&Frame {
                opcode: 230,
                payload: enclosed.payload[..n].to_vec()
            })
            .is_err(),
            "truncated at {n}"
        );
    }
    assert!(
        ServerUpdate::decode(&Frame {
            opcode: 230,
            payload: vec![128, 128, 93]
        })
        .is_err()
    );
    assert!(
        updates::zone_enclosed(
            0,
            0,
            &[Frame {
                opcode: 22,
                payload: vec![]
            }]
        )
        .is_err()
    );
    assert!(
        updates::zone_enclosed(
            0,
            0,
            &[Frame {
                opcode: 179,
                payload: vec![0]
            }]
        )
        .is_err()
    );
}
fn drain(decoder: &mut Decoder) -> Vec<Frame> {
    let mut out = Vec::new();
    while let Some(f) = decoder.next_frame().unwrap() {
        out.push(f);
    }
    out
}
#[test]
fn every_original_size_frames_across_all_header_and_payload_boundaries() {
    for direction in [Direction::ClientToServer, Direction::ServerToClient] {
        let mut expected = Vec::new();
        let mut wire = Vec::new();
        for opcode in 0..=255u8 {
            let Some(size) = direction.size(opcode) else {
                continue;
            };
            for n in match size {
                Size::Fixed(n) => vec![n],
                Size::Byte => vec![0, 1, 255],
                Size::Short => vec![0, 1, 255, 256, 65535],
            } {
                let f = Frame {
                    opcode,
                    payload: (0..n).map(|i| i as u8).collect(),
                };
                f.write(direction, None, &mut wire).unwrap();
                expected.push(f);
            }
        }
        for chunk_size in [1, 2, 3, 7, 255, 256, 257, 65536, wire.len()] {
            let mut decoder = Decoder::new(direction, wire.len() + 4, 65535);
            let mut actual = Vec::new();
            for chunk in wire.chunks(chunk_size) {
                decoder.push(chunk).unwrap();
                actual.extend(drain(&mut decoder));
            }
            assert_eq!(
                actual, expected,
                "direction={direction:?} chunks={chunk_size}"
            );
            assert_eq!(decoder.buffered(), 0);
        }
    }
}
#[test]
fn bounds_unknown_sizes_and_cipher_advancement_are_explicit() {
    let direction = Direction::ClientToServer;
    let mut d = Decoder::new(direction, 3, 2);
    assert!(d.push(&[44, 3, 1, 2]).is_err());
    d.push(&[44, 3]).unwrap();
    assert!(d.next_frame().is_err());
    assert!(d.next_frame().is_err());
    let mut d = Decoder::new(direction, 10, 10);
    d.push(&[0]).unwrap();
    assert!(d.next_frame().is_err());
    assert!(d.push(&[93]).is_err());
    let mut encoder = OpcodeCipher::new(&[1, 2, 3, 4], true).unwrap();
    let mut decoder = Decoder::new(direction, 1024, 255)
        .with_cipher(OpcodeCipher::new(&[1, 2, 3, 4], true).unwrap());
    let frames = [
        Frame {
            opcode: 44,
            payload: vec![1, 2, 3],
        },
        Frame {
            opcode: 93,
            payload: vec![],
        },
        Frame {
            opcode: 3,
            payload: vec![9, 8],
        },
    ];
    let mut wire = Vec::new();
    let invalid = Frame {
        opcode: 3,
        payload: vec![1],
    };
    assert!(
        invalid
            .write(direction, Some(&mut encoder), &mut wire)
            .is_err()
    );
    assert!(wire.is_empty());
    for f in &frames {
        f.write(direction, Some(&mut encoder), &mut wire).unwrap();
    }
    let mut actual = Vec::new();
    for b in wire {
        decoder.push(&[b]).unwrap();
        actual.extend(drain(&mut decoder));
        actual.extend(drain(&mut decoder));
    }
    assert_eq!(actual, frames);
}
