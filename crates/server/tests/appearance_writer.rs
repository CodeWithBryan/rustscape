#[path = "../../../oracle/appearance-writer/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/appearance-writer/oracle.jsonl");
#[test]
fn original_late_appearance_writer_receivers_reentry_and_partial_bytes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (index, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            let legacy_matches = row["legacyExpected"].as_str().is_none_or(|expected| {
                actual["results"][0]["bytes"].as_str() == Some(format!("aa55{expected}").as_str())
            });
            if actual != row["expected"] || !legacy_matches {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../runs/appearance-writer-investigation")
                    .join(format!("mismatch-{index}.json"));
                if !path.exists() {
                    std::fs::write(
                        &path,
                        serde_json::to_vec_pretty(
                            &serde_json::json!({"case":index,"input":row,"actual":actual}),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }
                tx.send(Err(format!(
                    "appearance writer case {index}: {}",
                    path.display()
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("writer callback failed or deadlocked")
        .unwrap();
}
#[test]
fn original_writer_sources_and_observations_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/appearance-writer/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../fixtures/appearance-writer/baseline-468.json"
    ))
    .unwrap();
    let first = CORPUS.lines().take(468).collect::<Vec<_>>().join("\n") + "\n";
    assert_eq!(baseline["sha256"], format!("{:x}", Sha256::digest(first)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, digest) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let data = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                digest.as_str().unwrap(),
                "{name}"
            );
        }
    }
}

#[test]
fn shared_output_preserves_original_header_patch_after_callback_rewinds_cursor() {
    use rs09_server::player::appearance_writer::Buffer;
    let buffer = Buffer::from_bytes(vec![9; 300]);
    buffer.p1(0);
    buffer.p2(0xabcd);
    let callback = buffer.clone();
    callback.set_position(0).unwrap();
    callback.p8(-1);
    buffer.psize_add(-293).unwrap();
    callback.set_position(301).unwrap();
    let actual = buffer.bytes();
    assert_eq!(&actual[..8], &[255; 8]);
    assert_eq!(&actual[8..300], &[9; 292]);
    assert_eq!(actual[300], 91);
    assert_eq!(buffer.position(), 301);
}
