#[path = "../../../oracle/interfaces/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/interfaces/oracle.jsonl");
fn diff(a: &Value, b: &Value, p: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for k in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
                format!("{p}/{k}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = diff(a, b, format!("{p}/{i}")) {
                return Some(d);
            }
        }
    }
    Some(format!("{p}:native={a},original={b}"))
}
#[test]
fn original_interface_metadata_windows_chat_tabs_plugins_and_counter_prefixes() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, l) in CORPUS.lines().enumerate() {
            let c: Value = serde_json::from_str(l).unwrap();
            if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
                tx.send(Err(format!("interface case{i}:{d}"))).unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("interface callback worker failed or deadlocked")
        .unwrap();
}
#[test]
fn interface_fixture_and_pinned_configuration_sources_are_unchanged() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/interfaces/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let initial: Value =
        serde_json::from_str(include_str!("../../../oracle/interfaces/initial.lock.json")).unwrap();
    assert_eq!(p["initialInterfaceBaseline"], initial);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let objects: Value =
        serde_json::from_str(include_str!("../../../oracle/interfaces/objects.lock.json")).unwrap();
    assert_eq!(p["typedObjectsBaseline"], objects);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(objects["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(objects["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(
        p["configSha256"],
        format!(
            "{:x}",
            Sha256::digest(
                std::fs::read(root.join("fixtures/interfaces/interface_configs.json")).unwrap()
            )
        )
    );
    for (path, h) in p["sources"].as_object().unwrap() {
        let p = root.join(path);
        if p.is_file() {
            let b: Vec<_> = std::fs::read(&p)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                h.as_str().unwrap(),
                "{}",
                p.display()
            );
        }
    }
}
