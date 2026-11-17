//! Replays `tests/data/store_write.jsonl` against [`DiskStore`].
//!
//! The file is the output of `oracle/StoreWriteOracle.java`: the original,
//! unmodified `Cache.write`/`Cache.read` (over `BufferedFile`/`FileOnDisk`,
//! JDK 25) run through a script of appends, overwrites, damaged chains and
//! truncated files. Every write result, every read result and the length and
//! SHA-256 of every file at three points must be the same here. Regenerate it
//! with the command in that Java file's header.
use rs09_cache::disk::{ARCHIVE_MAX_GROUP, DiskStore, MASTER_MAX_GROUP};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn payload(length: usize, seed: usize) -> Vec<u8> {
    (0..length)
        .map(|i| (i * 31 + seed * 7 + (i >> 8)) as u8)
        .collect()
}
fn limit(archive: u8) -> usize {
    if archive == 255 {
        MASTER_MAX_GROUP
    } else {
        ARCHIVE_MAX_GROUP
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn digest(bytes: &[u8]) -> Value {
    serde_json::json!({"length": bytes.len(), "sha256": hex(&Sha256::digest(bytes))})
}

#[test]
fn the_original_cache_class_and_the_port_leave_the_same_files() {
    let script = include_str!("data/store_write.jsonl");
    let empty = || BTreeMap::from([(3u8, Vec::new()), (7, Vec::new()), (255, Vec::new())]);
    let mut store = DiskStore::in_memory(Vec::new(), empty());
    let (mut writes, mut reads, mut absent, mut snapshots, mut edits) = (0, 0, 0, 0, 0);
    for (number, line) in script.lines().enumerate() {
        let step: Value = serde_json::from_str(line).expect("oracle line");
        let field = |name: &str| step[name].as_u64().expect("number") as usize;
        let context = format!("line {}: {line:.120}", number + 1);
        match step["kind"].as_str().expect("kind") {
            "write" => {
                let archive = field("archive") as u8;
                let ok = store.write(
                    archive,
                    field("group") as u32,
                    &payload(field("length"), field("seed")),
                    limit(archive),
                );
                assert_eq!(ok, step["ok"].as_bool().unwrap(), "{context}");
                writes += 1;
            }
            "read" => {
                let archive = field("archive") as u8;
                let result = store.read(archive, field("group") as u32, limit(archive));
                let expected = step["expected"].as_str();
                assert_eq!(result.as_deref().map(hex).as_deref(), expected, "{context}");
                reads += 1;
                absent += usize::from(expected.is_none());
            }
            "mutate" => {
                let (mut data, mut indexes) = store.memory_bytes().unwrap();
                let file = step["file"].as_str().unwrap();
                let bytes = match file.strip_prefix("idx") {
                    Some(archive) => indexes.get_mut(&archive.parse().unwrap()).unwrap(),
                    None => &mut data,
                };
                for byte in &mut bytes[field("offset")..field("offset") + field("count")] {
                    *byte ^= 0xa5;
                }
                store = DiskStore::in_memory(data, indexes);
                edits += 1;
            }
            "truncate" => {
                let (mut data, indexes) = store.memory_bytes().unwrap();
                assert!(field("length") <= data.len(), "{context}");
                data.truncate(field("length"));
                store = DiskStore::in_memory(data, indexes);
                edits += 1;
            }
            "files" => {
                let (data, indexes) = store.memory_bytes().unwrap();
                assert_eq!(digest(&data), step["data"], "{context}");
                for (archive, bytes) in &indexes {
                    assert_eq!(
                        digest(bytes),
                        step["indexes"][archive.to_string()],
                        "index {archive}, {context}"
                    );
                }
                snapshots += 1;
            }
            other => panic!("unknown step {other}"),
        }
    }
    // The script itself, so a shortened fixture cannot pass unnoticed.
    assert_eq!(
        (writes, reads, absent, edits, snapshots),
        (34, 58, 19, 10, 3)
    );
}
