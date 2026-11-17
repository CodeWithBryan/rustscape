//! Differential tests: expected values come from original, unmodified JVM code.
use rs09_game_core::{
    collision::CollisionMap,
    direction::Direction,
    int_utils,
    random::{JavaRandom, shuffle},
};
use rs09_protocol::{
    buffer::{Reader, Writer},
    checksum,
    huffman::Huffman,
    isaac::{Isaac, OpcodeCipher},
    xtea,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const FIXTURES: &str = include_str!("../../fixtures/rt4-v1.jsonl");

fn cases(kinds: &[&str]) -> Vec<Value> {
    FIXTURES
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| kinds.contains(&case["kind"].as_str().unwrap()))
        .collect()
}
fn number(case: &Value, field: &str) -> i64 {
    case[field].as_i64().unwrap()
}
fn size(case: &Value, field: &str) -> usize {
    number(case, field) as usize
}
fn decode(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
        .collect()
}
fn bytes(case: &Value, field: &str) -> Vec<u8> {
    decode(case[field].as_str().unwrap())
}
fn signed(case: &Value, field: &str) -> Vec<i32> {
    case[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_i64().unwrap() as i32)
        .collect()
}

#[test]
fn fixture_provenance_and_profile_are_explicit() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../fixtures/rt4-v1.provenance.json")).unwrap();
    let lock: Value = serde_json::from_str(include_str!("../../upstream.lock.json")).unwrap();
    assert_eq!(provenance["upstream"], lock);
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(FIXTURES.as_bytes()))
    );
    assert_eq!(size(&provenance, "caseCount"), FIXTURES.lines().count());
    let known = [
        "defaults",
        "isaac",
        "isaac_toggle",
        "read",
        "read_long",
        "bits",
        "checksum",
        "write",
        "varint",
        "write_long",
        "xtea",
        "huffman",
        "integer",
        "reverse",
        "pow",
        "shuffle",
        "clamp",
        "random",
        "js5",
        "collision_clear",
        "collision_wall",
        "collision_scenery",
        "collision_tile",
        "collision_path",
        "direction",
        "direction_delta",
        "direction_flag",
        "direction_probe",
    ];
    let all = cases(&known);
    assert_eq!(
        all.len(),
        FIXTURES.lines().count(),
        "every fixture kind needs a test dispatcher"
    );
    let profile = cases(&["defaults"]);
    assert_eq!(profile.len(), 1);
    let native = rs09_protocol::profile::RS09_REVISION_530;
    assert_eq!(native.revision, lock["revision"].as_u64().unwrap() as u32);
    assert_eq!(profile[0]["isaac"], native.use_isaac);
    assert_eq!(profile[0]["loginStrings"], native.login_use_strings);
    assert_eq!(profile[0]["extraInfo"], native.login_extra_info);
    assert_eq!(profile[0]["fakeIndex28"], native.login_fake_index_28);
    assert_eq!(profile[0]["sampleRate"], 22050);
}

#[test]
fn isaac_block_boundaries_and_disabled_state_match_java() {
    for case in cases(&["isaac"]) {
        let seed: Vec<_> = signed(&case, "seed")
            .into_iter()
            .map(|n| n as u32)
            .collect();
        let mut random = Isaac::new(&seed).unwrap();
        for (position, value) in signed(&case, "values").into_iter().enumerate() {
            assert_eq!(
                random.next_key() as i32,
                value,
                "ISAAC key {position}, seed {seed:?}"
            );
        }
    }
    for case in cases(&["isaac_toggle"]) {
        let seed: Vec<_> = signed(&case, "seed")
            .into_iter()
            .map(|n| n as u32)
            .collect();
        let mut random = OpcodeCipher::new(&seed, false).unwrap();
        for value in signed(&case, "disabled") {
            assert_eq!(random.next_key() as i32, value);
        }
        random.set_enabled(true);
        for value in signed(&case, "enabled") {
            assert_eq!(random.next_key() as i32, value);
        }
        random.set_enabled(false);
        assert_eq!(random.next_key() as i64, number(&case, "paused"));
        random.set_enabled(true);
        assert_eq!(
            i64::from(random.next_key() as i32),
            number(&case, "resumed")
        );
    }
}

#[test]
fn buffer_reads_and_bit_access_match_java() {
    for case in cases(&["read", "read_long", "varint", "bits"]) {
        let input = bytes(&case, "input");
        let mut reader = Reader::new(&input);
        let kind = case["kind"].as_str().unwrap();
        if kind == "varint" {
            assert_eq!(
                i64::from(reader.g_var_int().unwrap()),
                number(&case, "read")
            );
        } else {
            reader.seek(size(&case, "start")).unwrap();
            if kind == "read_long" {
                assert_eq!(
                    reader.g_var_long(size(&case, "length")).unwrap(),
                    number(&case, "value")
                );
            } else if kind == "bits" {
                let mut bit_reader = reader.bits();
                for (count, value) in signed(&case, "sizes")
                    .into_iter()
                    .zip(signed(&case, "values"))
                {
                    assert_eq!(
                        bit_reader.read(count as usize).unwrap() as i32,
                        value,
                        "{case}"
                    );
                }
                assert_eq!(bit_reader.remaining(), size(&case, "remaining"));
                bit_reader.finish();
            } else {
                let value = match case["method"].as_str().unwrap() {
                    "g1" => reader.g1(),
                    "g1b" => reader.g1b(),
                    "g1add" => reader.g1add(),
                    "g1neg" => reader.g1neg(),
                    "g1sub" => reader.g1sub(),
                    "g1badd" => reader.g1badd(),
                    "g1bneg" => reader.g1bneg(),
                    "g1bsub" => reader.g1bsub(),
                    "g2" => reader.g2(),
                    "ig2" => reader.ig2(),
                    "g2add" => reader.g2add(),
                    "ig2add" => reader.ig2add(),
                    "g2b" => reader.g2b(),
                    "ig2b" => reader.ig2b(),
                    "g2badd" => reader.g2badd(),
                    "ig2badd" => reader.ig2badd(),
                    "g3" => reader.g3(),
                    "ig3" => reader.ig3(),
                    "g4" => reader.g4(),
                    "ig4" => reader.ig4(),
                    "mg4" => reader.mg4(),
                    "img4" => reader.img4(),
                    "gsmart" => reader.gsmart(),
                    "gsmarts" => reader.gsmarts(),
                    "gVarSmart" => reader.g_var_smart(),
                    "g8" => {
                        assert_eq!(reader.g8().unwrap(), number(&case, "value"));
                        assert_eq!(reader.position(), size(&case, "end"));
                        continue;
                    }
                    method => panic!("unhandled read {method}"),
                }
                .unwrap();
                assert_eq!(i64::from(value), number(&case, "value"), "{case}");
            }
        }
        assert_eq!(reader.position(), size(&case, "end"), "{case}");
    }
}

#[test]
fn buffer_writes_match_java_including_overflow() {
    for case in cases(&["write", "write_long"]) {
        let mut writer = Writer::new();
        if case["kind"] == "write_long" {
            writer
                .p_var_long(size(&case, "length"), number(&case, "value"))
                .unwrap();
            if size(&case, "length") == 8 {
                let mut full = Writer::new();
                full.p8(number(&case, "value"));
                assert_eq!(full.as_slice(), bytes(&case, "bytes"));
            }
        } else {
            writer.p1(0x59);
            let value = number(&case, "value") as i32;
            match case["method"].as_str().unwrap() {
                "p1" => writer.p1(value),
                "p1add" => writer.p1add(value),
                "p1sub" => writer.p1sub(value),
                "p2" => writer.p2(value),
                "p2add" => writer.p2add(value),
                "ip2" => writer.ip2(value),
                "ip2add" => writer.ip2add(value),
                "p3" => writer.p3(value),
                "p4" => writer.p4(value),
                "ip4" => writer.ip4(value),
                "mp4" => writer.mp4(value),
                "imp4" => writer.imp4(value),
                "pVarInt" => writer.p_var_int(value),
                "psmarts" => writer.psmarts(value).unwrap(),
                method => panic!("unhandled write {method}"),
            }
        }
        assert_eq!(writer.as_slice(), bytes(&case, "bytes"), "{case}");
    }
}

#[test]
fn huffman_symbol_order_offsets_and_32_bit_codes_match_java() {
    for case in cases(&["huffman"]) {
        let table: Vec<_> = signed(&case, "table")
            .into_iter()
            .map(|n| n as u8)
            .collect();
        let codec = Huffman::new(&table).unwrap();
        let source = bytes(&case, "source");
        let encoded = codec
            .encode(&source[size(&case, "sourceStart")..size(&case, "sourceEnd")])
            .unwrap();
        assert_eq!(encoded, bytes(&case, "encoded"), "{case}");
        assert_eq!(encoded.len(), size(&case, "length"));
        let (decoded, consumed) = codec
            .decode(
                &encoded,
                size(&case, "sourceEnd") - size(&case, "sourceStart"),
            )
            .unwrap();
        let expected = bytes(&case, "decoded");
        assert_eq!(decoded, expected[4..4 + decoded.len()], "{case}");
        assert_eq!(consumed, size(&case, "consumed"));
    }
}

#[test]
fn xtea_and_checksums_match_java() {
    for case in cases(&["xtea"]) {
        let mut data = bytes(&case, "input");
        let key: Vec<_> = signed(&case, "key").into_iter().map(|n| n as u32).collect();
        xtea::decrypt_js5(&mut data, size(&case, "length"), key.try_into().unwrap()).unwrap();
        assert_eq!(data, bytes(&case, "bytes"), "{case}");
    }
    for case in cases(&["checksum"]) {
        let input = bytes(&case, "input");
        assert_eq!(
            u64::from(checksum::crc32(&input)),
            case["crc32"].as_u64().unwrap()
        );
        assert_eq!(
            checksum::crc64(&input).to_string(),
            case["crc64"].as_str().unwrap()
        );
    }
}

#[test]
fn integer_math_matches_java_at_signed_boundaries() {
    for case in cases(&["integer", "reverse", "pow", "shuffle", "clamp"]) {
        let value = number(&case, "value") as i32;
        match case["kind"].as_str().unwrap() {
            "integer" => {
                assert_eq!(
                    int_utils::clp2(value),
                    number(&case, "clp2") as i32,
                    "{case}"
                );
                assert_eq!(
                    int_utils::bit_count(value),
                    number(&case, "bitCount") as i32,
                    "{case}"
                );
                assert_eq!(
                    int_utils::bit_count_fast(value),
                    number(&case, "bitCountFast") as i32,
                    "{case}"
                );
                assert_eq!(
                    int_utils::is_power_of_two(value),
                    case["powerOfTwo"].as_bool().unwrap(),
                    "{case}"
                );
            }
            kind => {
                let result = match kind {
                    "reverse" => int_utils::bit_reverse(number(&case, "count") as i32, value),
                    "pow" => int_utils::pow(number(&case, "exponent") as i32, value),
                    "shuffle" => shuffle(value, number(&case, "bound") as i32).unwrap(),
                    "clamp" => int_utils::clamp(
                        number(&case, "max") as i32,
                        value,
                        number(&case, "min") as i32,
                    ),
                    _ => unreachable!(),
                };
                assert_eq!(result, number(&case, "result") as i32, "{case}");
            }
        }
    }
}

#[test]
fn seeded_random_streams_and_rejection_sampling_match_java() {
    for case in cases(&["random"]) {
        let mut random = JavaRandom::new(number(&case, "seed"));
        for expected in case["values"].as_array().unwrap() {
            match case["mode"].as_str().unwrap() {
                "int" => assert_eq!(i64::from(random.next_i32()), expected.as_i64().unwrap()),
                "long" => assert_eq!(random.next_i64(), expected.as_i64().unwrap()),
                "double" => assert_eq!(
                    random.next_double().to_bits().to_string(),
                    expected.as_str().unwrap()
                ),
                "java_bound" => assert_eq!(
                    i64::from(random.next_bound(number(&case, "bound") as i32).unwrap()),
                    expected.as_i64().unwrap()
                ),
                "rt4_bound" => assert_eq!(
                    i64::from(random.rt4_bound(number(&case, "bound") as i32).unwrap()),
                    expected.as_i64().unwrap()
                ),
                mode => panic!("unhandled random mode {mode}"),
            }
        }
    }
}

#[test]
fn js5_raw_gzip_and_multiblock_bzip_match_java() {
    for case in cases(&["js5"]) {
        let input = bytes(&case, "input");
        let expected = bytes(&case, "output");
        assert_eq!(rs09_cache::decompress(&input, 1_000_000).unwrap(), expected);
        if !expected.is_empty() {
            assert!(rs09_cache::decompress(&input, expected.len() - 1).is_err());
        }
    }
}

fn collision_map(case: &Value, field: &str) -> CollisionMap {
    let width = size(case, "width");
    let height = size(case, "height");
    let mut map = CollisionMap::new(width, height).unwrap();
    for (index, flags) in signed(case, field).into_iter().enumerate() {
        map.set((index / height) as i32, (index % height) as i32, flags)
            .unwrap();
    }
    map
}

#[test]
fn collision_mutations_and_clipped_rectangles_match_java() {
    for case in cases(&[
        "collision_clear",
        "collision_wall",
        "collision_scenery",
        "collision_tile",
    ]) {
        let kind = case["kind"].as_str().unwrap();
        if kind == "collision_clear" {
            let map = CollisionMap::new(size(&case, "width"), size(&case, "height")).unwrap();
            assert_eq!(map.flags(), signed(&case, "flags"));
        } else if kind == "collision_tile" {
            let mut map = CollisionMap::new(16, 16).unwrap();
            let mask = number(&case, "mask") as i32;
            map.set_mask(8, 7, mask, true).unwrap();
            assert_eq!(map.flags(), signed(&case, "added"));
            if mask == 0x40000 {
                map.set_mask(8, 7, mask, false).unwrap();
            }
            assert_eq!(map.flags(), signed(&case, "removed"));
        } else {
            let mut map = collision_map(&case, "initial");
            let at = (number(&case, "x") as i32, number(&case, "y") as i32);
            let projectiles = case["projectiles"].as_bool().unwrap();
            let angle = number(&case, "angle") as u8;
            if kind == "collision_wall" {
                let shape = number(&case, "shape") as u8;
                map.wall(at, angle, shape, projectiles, true).unwrap();
                assert_eq!(
                    map.flags(),
                    signed(&case, "added"),
                    "wall {shape} angle {angle}"
                );
                map.wall(at, angle, shape, projectiles, false).unwrap();
            } else {
                let dimensions = (number(&case, "sizeX") as i32, number(&case, "sizeY") as i32);
                map.scenery(at, dimensions, projectiles, true);
                assert_eq!(
                    map.flags(),
                    signed(&case, "added"),
                    "scenery {at:?} angle {angle}"
                );
                let dimensions = if angle == 1 || angle == 3 {
                    (dimensions.1, dimensions.0)
                } else {
                    dimensions
                };
                map.scenery(at, dimensions, projectiles, false);
            }
            assert_eq!(
                map.flags(),
                signed(&case, "removed"),
                "{kind} angle {angle}"
            );
        }
    }
}

#[test]
fn collision_lines_match_java_for_every_direction_and_flag_bit() {
    for case in cases(&["collision_path"]) {
        let map = collision_map(&case, "flags");
        for query in case["queries"].as_array().unwrap() {
            let source = (number(query, "sx") as i32, number(query, "sy") as i32);
            let destination = (number(query, "dx") as i32, number(query, "dy") as i32);
            assert_eq!(
                map.is_path_clear(source, destination),
                query["clear"].as_bool().unwrap(),
                "{source:?} -> {destination:?}"
            );
        }
    }
}

#[test]
fn server_direction_codes_selection_and_probe_order_match_java() {
    for case in cases(&["direction_delta"]) {
        let x = number(&case, "x") as i32;
        let y = number(&case, "y") as i32;
        assert_eq!(
            Direction::from_delta(x, y).code(),
            number(&case, "code") as i32
        );
        assert_eq!(case["code"], case["locationCode"]);
        assert_eq!(
            Direction::logical_delta(x, y).code(),
            number(&case, "logicalCode") as i32
        );
    }
    for case in cases(&["direction_flag"]) {
        let result = Direction::walk_flag(
            number(&case, "flags") as i32,
            number(&case, "rotation") as u8,
        );
        assert_eq!(
            result.map_or(-1, Direction::code),
            number(&case, "code") as i32
        );
    }
    for case in cases(&["direction"]) {
        let direction = Direction::from_code(number(&case, "code") as i32).unwrap();
        assert_eq!(
            direction.step(),
            (number(&case, "x") as i32, number(&case, "y") as i32)
        );
        assert_eq!(case["x"], case["walkX"]);
        assert_eq!(case["y"], case["walkY"]);
        assert_eq!(
            direction.opposite().code(),
            number(&case, "opposite") as i32
        );
    }
    for case in cases(&["direction_probe"]) {
        let direction = Direction::from_code(number(&case, "code") as i32).unwrap();
        let mut probes = Vec::new();
        let slot = number(&case, "slot");
        let allowed = direction.can_move_from(
            number(&case, "z") as i32,
            number(&case, "x") as i32,
            number(&case, "y") as i32,
            |z, x, y| {
                let index = probes.len() as i64;
                probes.push(vec![z, x, y]);
                if slot < 0 || slot == index {
                    number(&case, "mask") as i32
                } else {
                    0
                }
            },
        );
        assert_eq!(allowed, case["allowed"].as_bool().unwrap(), "{case}");
        assert_eq!(
            serde_json::to_value(probes).unwrap(),
            case["probes"],
            "{case}"
        );
    }
}
