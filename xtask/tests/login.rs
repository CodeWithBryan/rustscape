use rs09_protocol::{
    buffer::Reader,
    isaac::{Isaac, OpcodeCipher},
    login::{self, AuthResponse, LoginInfo, LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../fixtures/login/oracle.jsonl");
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn bytes(case: &Value, field: &str) -> Vec<u8> {
    let text = case[field].as_str().unwrap();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
fn number(case: &Value, field: &str) -> i64 {
    case[field].as_i64().unwrap()
}
fn ints(value: &Value) -> Vec<i32> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap() as i32)
        .collect()
}

#[test]
fn login_fixtures_pin_original_java_kotlin_and_explicit_adapters() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../fixtures/login/provenance.json")).unwrap();
    assert_eq!(
        metadata["upstream"],
        serde_json::from_str::<Value>(include_str!("../../upstream.lock.json")).unwrap()
    );
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(
        metadata["caseCount"].as_u64().unwrap() as usize,
        CORPUS.lines().count()
    );
    let known = [
        "name",
        "long_name",
        "server_string",
        "rsa",
        "client_frame",
        "decode",
        "reply",
        "rsa_decrypt",
    ];
    assert_eq!(
        known.iter().map(|kind| cases(kind).len()).sum::<usize>(),
        CORPUS.lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for (path, expected) in metadata["sources"].as_object().unwrap() {
        if path.starts_with("oracle/") {
            let actual = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&byte| byte != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(actual)), "{path}");
        }
    }
}

#[test]
fn client_base37_and_server_long_names_match_their_distinct_original_rules() {
    for case in cases("name") {
        let name = bytes(&case, "raw");
        let encoded = login::encode_name(&name);
        assert_eq!(encoded, number(&case, "encoded"));
        assert_eq!(login::decode_name(encoded), case["decoded"]);
        assert_eq!(login::handshake(&name), [14, ((encoded >> 16) & 31) as u8]);
    }
    for case in cases("long_name") {
        assert_eq!(login::decode_name(number(&case, "value")), case["decoded"]);
    }
}

#[test]
fn kotlin_byte_strings_preserve_signed_byte_to_char_conversion() {
    for case in cases("server_string") {
        assert_eq!(login::server_string(&bytes(&case, "raw")), case["value"]);
    }
}

#[test]
fn signed_rsa_and_framed_underflow_match_big_integer_and_original_buffer() {
    for case in cases("rsa") {
        let key =
            RsaKey::new(case["exp"].as_str().unwrap(), case["mod"].as_str().unwrap()).unwrap();
        assert_eq!(
            key.frame(&bytes(&case, "raw")).unwrap(),
            bytes(&case, "framed")
        );
    }
    let private = RsaKey::reference_private();
    for case in cases("rsa_decrypt") {
        assert_eq!(
            private
                .unframe(&mut Reader::new(&bytes(&case, "framed")))
                .unwrap(),
            bytes(&case, "result")
        );
    }
}

fn info(value: LoginInfo) -> Value {
    fn keys(seed: Option<[i32; 4]>) -> Vec<i32> {
        let seed: Vec<_> = seed.map_or_else(Vec::new, |seed| seed.map(|word| word as u32).to_vec());
        let mut cipher = Isaac::new(&seed).unwrap();
        (0..8).map(|_| cipher.next_key() as i32).collect()
    }
    fn disabled(seed: Option<[i32; 4]>) -> Vec<u32> {
        let seed: Vec<_> = seed.map_or_else(Vec::new, |seed| seed.map(|word| word as u32).to_vec());
        let mut cipher = OpcodeCipher::new(&seed, false).unwrap();
        let mut result = Vec::new();
        for _ in 0..520 {
            let value = cipher.next_key();
            if !result.contains(&value) {
                result.push(value);
            }
        }
        result
    }
    json!({"opcode":value.opcode,"showAds":value.show_ads,"windowMode":value.window_mode,
        "width":value.screen_width,"height":value.screen_height,"displayMode":value.display_mode,
        "affiliate":value.affiliate,"settingsHash":value.settings_hash,"packetCount":value.packet_count,
        "crcs":value.crc_sums,"username":value.username,"password":value.password,
        "incomingInitialKeys":keys(value.isaac_seed),"outgoingInitialKeys":keys(value.outgoing_seed()),
        "incomingDisabled":disabled(value.isaac_seed),"outgoingDisabled":disabled(value.outgoing_seed())})
}
fn check_decode(case: &Value, private: &RsaKey) {
    let mut lookups = Vec::new();
    let result = login::decode(&bytes(case, "packet"), private, |name| {
        lookups.push(name.to_owned());
        case["online"].as_bool().unwrap()
            && [
                "alice",
                "agent_42",
                "bob",
                "",
                "zzzzzzzzzzzz",
                "a_b",
                "rust_client",
                "mixed_case",
                "0123456789",
                "a",
            ]
            .contains(&name)
    });
    assert_eq!(
        json!({"response":result.response as u8,"info":result.info.map(info),"onlineLookups":lookups}),
        case["expected"],
        "case {} ({})",
        case["id"],
        case["kind"]
    );
}

#[test]
fn complete_client_packet_bytes_match_verbatim_login_manager_writer() {
    let public = RsaKey::reference_public();
    for case in cases("client_frame") {
        let fields = ints(&case["fields"]);
        let username = bytes(&case, "username");
        let password = bytes(&case, "password");
        let settings = bytes(&case, "settings");
        let request = LoginRequest {
            reconnect: fields[9] == 40,
            login_type: fields[0],
            advertisements_suppressed: fields[1] != 0,
            window_mode: fields[2],
            screen_width: fields[3],
            screen_height: fields[4],
            display_mode: fields[5],
            affiliate: fields[6],
            settings_hash: fields[7],
            packet_count: fields[8],
            uid: bytes(&case, "uid").try_into().unwrap(),
            isaac_seed: ints(&case["seed"]).try_into().unwrap(),
            crc_sums: ints(&case["crcs"]).try_into().unwrap(),
            username: &username,
            password: &password,
            settings: &settings,
        };
        let mut profile = RS09_REVISION_530;
        profile.login_extra_info = case["extra"].as_bool().unwrap();
        profile.login_fake_index_28 = case["fake28"].as_bool().unwrap();
        assert_eq!(
            request.encode(profile, &public).unwrap(),
            bytes(&case, "packet"),
            "case {}",
            case["id"]
        );
    }
}

#[test]
fn kotlin_login_decoder_matches_metadata_responses_partial_info_and_lookup_order() {
    let private = RsaKey::reference_private();
    for case in cases("client_frame").into_iter().chain(cases("decode")) {
        check_decode(&case, &private);
    }
}

#[test]
fn login_reply_codes_rights_indices_and_ordinal_21_match_original_writer() {
    for case in cases("reply") {
        let response = AuthResponse::from_byte(number(&case, "response") as u8).unwrap();
        assert_eq!(
            login::reply(
                response,
                number(&case, "rights") as i32,
                number(&case, "index") as i32,
                number(&case, "key")
            ),
            bytes(&case, "bytes")
        );
    }
    assert!(AuthResponse::from_byte(23).is_none());
    assert!(AuthResponse::from_byte(255).is_none());
}

#[test]
fn invalid_key_and_unrepresentable_native_frames_are_rejected() {
    for (exp, modulus) in [("-1", "3"), ("1", "0"), ("1", "-3"), ("broken", "3")] {
        assert!(RsaKey::new(exp, modulus).is_err());
    }
    let public = RsaKey::reference_public();
    assert!(public.transform(&[]).is_err());
    assert!(
        LoginRequest {
            password: b"contains\0zero",
            ..LoginRequest::default()
        }
        .encode(RS09_REVISION_530, &public)
        .is_err()
    );
    let huge = vec![b'a'; 32768];
    assert!(
        LoginRequest {
            settings: &huge,
            ..LoginRequest::default()
        }
        .encode(RS09_REVISION_530, &public)
        .is_err()
    );
}
