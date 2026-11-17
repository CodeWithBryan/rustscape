use rs09_protocol::{
    js5,
    lobby::{self, World},
    session::{Decoder, Incoming, Phase},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../fixtures/network/oracle.jsonl");
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn n(value: &Value, field: &str) -> i64 {
    value[field].as_i64().unwrap()
}
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn bytes(case: &Value, field: &str) -> Vec<u8> {
    hex(case[field].as_str().unwrap())
}

#[test]
fn network_expectations_pin_original_sources_and_cover_every_case() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../fixtures/network/provenance.json")).unwrap();
    let upstream: Value = serde_json::from_str(include_str!("../../upstream.lock.json")).unwrap();
    assert_eq!(metadata["upstream"], upstream);
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(n(&metadata, "caseCount") as usize, CORPUS.lines().count());
    let known = [
        "archive_reply",
        "master_payload",
        "master_reply",
        "login_handshake",
        "cache_handshake",
        "request_stream",
        "world_list",
        "client_queue_bounds",
    ];
    let total: usize = known.iter().map(|kind| cases(kind).len()).sum();
    assert_eq!(total, CORPUS.lines().count());
}

#[test]
fn original_client_queue_bounds_are_twenty_per_priority() {
    let cases = cases("client_queue_bounds");
    assert_eq!(cases.len(), 1);
    assert_eq!(n(&cases[0], "urgent"), 20);
    assert_eq!(n(&cases[0], "prefetch"), 20);
}

#[test]
fn archive_headers_markers_priority_xor_and_missing_version_padding_match_java() {
    for case in cases("archive_reply") {
        let request = js5::Request {
            archive: n(&case, "archive") as u8,
            group: n(&case, "group") as u16,
            urgent: case["urgent"].as_bool().unwrap(),
        };
        let actual = js5::encode_archive(
            request,
            n(&case, "key") as u8,
            &bytes(&case, "input"),
            1_000_000,
        )
        .unwrap();
        assert_eq!(
            actual,
            bytes(&case, "expected"),
            "archive {} group {} key {} len {}",
            request.archive,
            request.group,
            n(&case, "key"),
            bytes(&case, "input").len()
        );
    }
}

#[test]
fn master_checksum_slots_and_separate_formatter_match_java() {
    for case in cases("master_payload") {
        let entries: Vec<_> = case["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                entry.as_array().map(|pair| {
                    (
                        pair[0].as_i64().unwrap() as u32,
                        pair[1].as_i64().unwrap() as i32,
                    )
                })
            })
            .collect();
        assert_eq!(js5::master_payload(&entries), bytes(&case, "expected"));
    }
    for case in cases("master_reply") {
        assert_eq!(
            js5::encode_master(&bytes(&case, "input"), 1_000_000).unwrap(),
            bytes(&case, "expected")
        );
    }
}

#[test]
fn login_and_cache_handshake_bytes_and_transitions_match_java() {
    for case in cases("login_handshake") {
        let mut decoder = Decoder::new(1024, 512);
        decoder.push(&bytes(&case, "input")).unwrap();
        assert_eq!(
            decoder.next_packet().unwrap(),
            Some(Incoming::LoginHandshake {
                name_hash: n(&case, "nameHash") as u8
            })
        );
        assert_eq!(decoder.phase(), Phase::Login);
        let key = rs09_game_core::random::JavaRandom::new(n(&case, "seed")).next_i64();
        assert_eq!(key, n(&case, "serverKey"));
        assert_eq!(
            rs09_protocol::session::login_handshake_reply(key).as_slice(),
            bytes(&case, "expected")
        );
    }
    for case in cases("cache_handshake") {
        let mut decoder = Decoder::new(1024, 512);
        decoder.push(&bytes(&case, "input")).unwrap();
        assert_eq!(
            decoder.next_packet().unwrap(),
            Some(Incoming::CacheHandshake {
                revision: n(&case, "revision") as u32
            })
        );
        let accepted = !case["closed"].as_bool().unwrap();
        assert_eq!(
            decoder.phase(),
            if accepted {
                Phase::Cache
            } else {
                Phase::Closed
            }
        );
        assert_eq!(
            bytes(&case, "expected"),
            if accepted { vec![0] } else { vec![] }
        );
    }
}

#[test]
fn cache_ingress_matches_original_across_every_chunk_boundary() {
    for case in cases("request_stream") {
        let mut decoder = Decoder::new(100_000, 32767);
        decoder.push(&js5::handshake(530)).unwrap();
        decoder.next_packet().unwrap();
        let mut requests = Vec::new();
        let mut key = 0u8;
        let input = bytes(&case, "input");
        for chunk in input.chunks(n(&case, "chunk") as usize) {
            decoder.push(chunk).unwrap();
            while let Some(packet) = decoder.next_packet().unwrap() {
                match packet {
                    Incoming::CacheRequest(request) => requests.push(vec![
                        i64::from(request.archive),
                        i64::from(request.group),
                        i64::from(request.urgent),
                    ]),
                    Incoming::CacheKey { key: value, .. } => key = value,
                    Incoming::CacheControl { .. } | Incoming::Disconnect => {}
                    packet => panic!("unexpected {packet:?}"),
                }
            }
            if decoder.phase() == Phase::Closed {
                break;
            }
        }
        assert_eq!(serde_json::to_value(requests).unwrap(), case["requests"]);
        assert_eq!(i64::from(key as i8), n(&case, "key"));
        assert_eq!(
            decoder.phase() == Phase::Closed,
            case["closed"].as_bool().unwrap()
        );
        if decoder.phase() != Phase::Closed {
            assert_eq!(decoder.buffered(), n(&case, "pending") as usize);
        }
    }
}

#[test]
fn world_list_encoding_and_update_stamps_match_original_iobuffer() {
    for case in cases("world_list") {
        let worlds: Vec<_> = case["worlds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|world| World {
                id: n(world, "id") as u16,
                location: n(world, "location") as u8,
                flags: n(world, "flags") as u32,
                activity: world["activity"].as_str().unwrap().as_bytes().to_vec(),
                host: world["host"].as_str().unwrap().as_bytes().to_vec(),
                region: world["region"].as_str().unwrap().as_bytes().to_vec(),
                country: n(world, "country") as u16,
                players: n(world, "players") as u16,
            })
            .collect();
        assert_eq!(
            lobby::world_list(
                &worlds,
                n(&case, "stamp") as i32,
                n(&case, "requested") as i32
            )
            .unwrap(),
            bytes(&case, "expected")
        );
    }
}

#[test]
fn client_block_decoder_handles_fragmented_original_server_packets() {
    for case in cases("archive_reply") {
        let input = bytes(&case, "input");
        let wire = bytes(&case, "expected");
        let length = u32::from_be_bytes(input[1..5].try_into().unwrap()) as usize;
        let container_length = length + if input[0] == 0 { 5 } else { 9 };
        for split in [1, 7, 511, 512, 1024] {
            let mut decoder = js5::ResponseDecoder::new(n(&case, "key") as u8, 1_000_000);
            let mut replies = Vec::new();
            for chunk in wire.chunks(split) {
                replies.extend(decoder.feed(chunk).unwrap());
            }
            assert_eq!(replies.len(), 1);
            assert_eq!(replies[0].container, input[..container_length]);
            assert_eq!(replies[0].request.archive, n(&case, "archive") as u8);
            assert_eq!(replies[0].request.group, n(&case, "group") as u16);
            assert_eq!(replies[0].request.urgent, case["urgent"].as_bool().unwrap());
            let padded = n(&case, "archive") != 255 && input[0] != 0 && input.len() == length + 9;
            assert_eq!(decoder.is_idle(), !padded);
        }
    }
}

#[test]
fn bounded_ingress_and_malformed_reply_lengths_are_rejected() {
    assert!(Decoder::new(4, 512).push(&[0; 5]).is_err());
    let mut decoder = js5::ResponseDecoder::new(0, 1024);
    assert!(decoder.feed(&[1, 0, 0, 0, 127, 255, 255, 255]).is_err());
    assert!(
        js5::encode_archive(
            js5::Request {
                archive: 1,
                group: 0,
                urgent: true
            },
            0,
            &[0; 4],
            1024
        )
        .is_err()
    );
    let mut session = Decoder::new(1024, 8);
    for byte in [14, 1] {
        session.push(&[byte]).unwrap();
    }
    session.next_packet().unwrap();
    session.push(&[16, 0, 9]).unwrap();
    assert!(session.next_packet().is_err());
}
