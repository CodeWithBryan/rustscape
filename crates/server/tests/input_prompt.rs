#[path = "../../../oracle/input-prompt/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/input-prompt/oracle.jsonl");
#[test]
fn input_frames_keep_text_and_following_actions_across_every_transport_split() {
    use rs09_protocol::{
        buffer::Reader,
        game::{ClientAction, Decoder, Direction, Frame},
    };
    let frames = vec![
        Frame {
            opcode: 244,
            payload: 1047i64.to_be_bytes().to_vec(),
        },
        Frame {
            opcode: 65,
            payload: vec![255, 128, 0],
        },
        Frame {
            opcode: 23,
            payload: 1000i32.to_be_bytes().to_vec(),
        },
        Frame {
            opcode: 93,
            payload: vec![],
        },
    ];
    let mut wire = vec![];
    for frame in &frames {
        frame
            .write(Direction::ClientToServer, None, &mut wire)
            .unwrap();
    }
    for split in 0..=wire.len() {
        let mut decoder = Decoder::new(Direction::ClientToServer, 1024, 255);
        let mut actual = vec![];
        for part in [&wire[..split], &wire[split..]] {
            decoder.push(part).unwrap();
            while let Some(frame) = decoder.next_frame().unwrap() {
                actual.push(frame);
            }
        }
        assert_eq!(actual, frames, "split{split}");
        let actions = actual
            .iter()
            .map(|f| {
                ClientAction::decode(f.opcode, &mut Reader::new(&f.payload))
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actions,
            vec![
                ClientAction::InputStringResponse {
                    response: "1k".into()
                },
                ClientAction::InputStringResponse {
                    response: "\u{ffff}\u{ff80}".into()
                },
                ClientAction::InputIntResponse { response: 1000 },
                ClientAction::Ping
            ]
        );
    }
    let mut decoder = Decoder::new(Direction::ClientToServer, 1024, 255);
    let mut actual = vec![];
    for byte in wire {
        decoder.push(&[byte]).unwrap();
        while let Some(f) = decoder.next_frame().unwrap() {
            actual.push(f);
        }
    }
    assert_eq!(actual, frames);
}
#[test]
fn original_input_parser_packet_finally_prompts_and_bank_closure_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        if actual != row["expected"] {
            panic!(
                "input case{i}: {}",
                difference(&actual, &row["expected"], String::new()).unwrap()
            );
        }
    }
}
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn input_sources_and_fixtures_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/input-prompt/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let initial: Value = serde_json::from_str(include_str!(
        "../../../oracle/input-prompt/initial.lock.json"
    ))
    .unwrap();
    assert_eq!(p["initialBaseline"], initial);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let wire: Value =
        serde_json::from_str(include_str!("../../../oracle/input-prompt/wire.lock.json")).unwrap();
    assert_eq!(p["wireBaseline"], wire);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(wire["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(wire["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let data = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                hash.as_str().unwrap(),
                format!("{:x}", Sha256::digest(data)),
                "{name}"
            );
        }
    }
}
