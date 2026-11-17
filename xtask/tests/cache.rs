//! Full original Cache/Js5Index/Js5 are the oracle, including storage and extraction.
use rs09_cache::{
    Limits,
    archive::Archive,
    group,
    index::Index,
    store::{StorageMode, Store},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Barrier},
};

const CORPUS: &str = include_str!("../../fixtures/cache/oracle.jsonl");
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn n(value: &Value, key: &str) -> i64 {
    value[key].as_i64().unwrap()
}
fn hex(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}
fn key(case: &Value) -> [u32; 4] {
    case["key"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap() as u32)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
fn source(name: &str) -> Vec<u8> {
    std::fs::read(root().join("fixtures/cache").join(name)).unwrap()
}
fn store(data: Vec<u8>, limits: Limits) -> Store {
    let indices = [3, 7, 255]
        .into_iter()
        .map(|id| (id, source(&format!("main_file_cache.idx{id}"))))
        .collect();
    Store::from_bytes(data, indices, limits)
}
fn resident(limits: Limits) -> Store {
    store(source("main_file_cache.dat2"), limits)
}

#[test]
fn cache_fixture_inputs_and_original_revisions_are_pinned() {
    let metadata: Value = serde_json::from_slice(&source("provenance.json")).unwrap();
    let lock: Value = serde_json::from_str(include_str!("../../upstream.lock.json")).unwrap();
    assert_eq!(metadata["upstream"], lock);
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(n(&metadata, "caseCount") as usize, CORPUS.lines().count());
    for (name, expected) in metadata["inputs"].as_object().unwrap() {
        assert_eq!(
            format!("{:x}", Sha256::digest(source(name))),
            expected.as_str().unwrap()
        );
    }
    assert_eq!(
        cases("index").len()
            + cases("store").len()
            + cases("group").len()
            + cases("group_fallback").len(),
        CORPUS.lines().count()
    );
}

#[test]
fn reference_tables_and_sparse_name_hashes_match_unmodified_java() {
    for case in cases("index") {
        let input = hex(case["input"].as_str().unwrap());
        let index = Index::decode(&input, None, Limits::default()).unwrap();
        let expected = &case["expected"];
        assert_eq!(u32::from(index.format), n(&case, "protocol") as u32);
        assert_eq!(index.capacity, n(expected, "capacity") as u32);
        assert_eq!(index.version, n(expected, "version") as i32);
        assert_eq!(
            u64::from(index.checksum),
            expected["checksum"].as_u64().unwrap()
        );
        assert_eq!(
            index.groups.len(),
            expected["groups"].as_array().unwrap().len()
        );
        assert!(
            Index::decode(
                &input,
                Some(index.checksum.wrapping_add(1)),
                Limits::default()
            )
            .is_err()
        );
        for (group, expected) in index
            .groups
            .iter()
            .zip(expected["groups"].as_array().unwrap())
        {
            assert_eq!(group.id, n(expected, "id") as u32);
            assert_eq!(
                u64::from(group.checksum),
                expected["checksum"].as_u64().unwrap()
            );
            assert_eq!(group.version, n(expected, "version") as i32);
            assert_eq!(group.capacity, n(expected, "capacity") as u32);
            assert_eq!(group.name_hash.map(i64::from), expected["name"].as_i64());
            assert_eq!(
                group.capacity as usize == group.files.len(),
                expected["dense"].as_bool().unwrap()
            );
            assert_eq!(
                group.files.len(),
                expected["files"].as_array().unwrap().len()
            );
            for (file, expected) in group
                .files
                .iter()
                .zip(expected["files"].as_array().unwrap())
            {
                assert_eq!(file.id, n(expected, "id") as u32);
                assert_eq!(file.name_hash.map(i64::from), expected["name"].as_i64());
            }
            for query in expected["lookups"].as_array().unwrap() {
                assert_eq!(
                    group
                        .file_by_hash(n(query, "hash") as i32)
                        .map_or(-1, i64::from),
                    n(query, "id")
                );
            }
        }
        for query in expected["lookups"].as_array().unwrap() {
            assert_eq!(
                index
                    .group_by_hash(n(query, "hash") as i32)
                    .map_or(-1, i64::from),
                n(query, "id")
            );
        }
    }
}

#[test]
fn sector_chains_and_invalid_headers_match_unmodified_java() {
    for case in cases("store") {
        let mut data = source("main_file_cache.dat2");
        for mutation in case["mutations"].as_array().unwrap() {
            data[n(mutation, "offset") as usize] = n(mutation, "value") as u8;
        }
        let store = store(data, Limits::default());
        let result = store
            .read(n(&case, "archive") as u8, n(&case, "group") as u32)
            .unwrap();
        assert_eq!(result, case["expected"].as_str().map(hex), "{case}");
    }
}

#[test]
fn encrypted_stripes_sparse_files_and_empty_files_match_unmodified_java() {
    let archive = Archive::open(resident(Limits::default()), 3, None, Limits::default()).unwrap();
    for case in cases("group") {
        let group_id = n(&case, "group") as u32;
        for file in case["files"].as_array().unwrap() {
            let result = archive
                .file(group_id, n(file, "id") as u32, key(&case))
                .unwrap()
                .unwrap();
            assert_eq!(
                result.as_slice(),
                hex(file["expected"].as_str().unwrap()),
                "group {group_id}"
            );
        }
        assert!(
            archive
                .file(group_id, u32::MAX, key(&case))
                .unwrap()
                .is_none()
        );
    }
    assert!(archive.group(u32::MAX, [0; 4]).unwrap().is_none());
}

#[test]
fn file_backend_readers_are_correct_under_concurrent_access() {
    let store = Store::open(
        &root().join("fixtures/cache"),
        StorageMode::File,
        Limits::default(),
    )
    .unwrap();
    let queries: Vec<_> = cases("store")
        .into_iter()
        .filter(|case| case["mutations"].as_array().unwrap().is_empty())
        .collect();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let store = store.clone();
            let queries = &queries;
            scope.spawn(move || {
                for _ in 0..12 {
                    for case in queries {
                        assert_eq!(
                            store
                                .read(n(case, "archive") as u8, n(case, "group") as u32)
                                .unwrap(),
                            case["expected"].as_str().map(hex)
                        );
                    }
                }
            });
        }
    });
}

#[test]
fn shared_group_cache_coalesces_concurrent_loads_and_respects_budgets() {
    let limits = Limits::default();
    let archive = Archive::open(resident(limits), 3, None, limits).unwrap();
    let case = cases("group")
        .into_iter()
        .find(|case| case["files"].as_array().unwrap().len() > 1)
        .unwrap();
    let group_id = n(&case, "group") as u32;
    let barrier = Arc::new(Barrier::new(8));
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let archive = &archive;
                let case = &case;
                scope.spawn(move || {
                    barrier.wait();
                    archive.group(group_id, key(case)).unwrap().unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    for result in &results {
        assert!(Arc::ptr_eq(result, &results[0]));
    }
    assert_eq!(archive.cached_bytes(), results[0].retained_bytes());
    let limits = Limits {
        cached_bytes: 0,
        ..limits
    };
    let uncached = Archive::open(resident(limits), 3, None, limits).unwrap();
    let a = uncached.group(group_id, key(&case)).unwrap().unwrap();
    let b = uncached.group(group_id, key(&case)).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert_eq!(uncached.cached_bytes(), 0);
    for file in case["files"].as_array().unwrap() {
        assert_eq!(
            a.get(n(file, "id") as u32).unwrap().as_slice(),
            hex(file["expected"].as_str().unwrap())
        );
    }
}

#[test]
fn malformed_groups_and_resource_limits_return_errors() {
    let archive = Archive::open(resident(Limits::default()), 3, None, Limits::default()).unwrap();
    let case = cases("group")
        .into_iter()
        .find(|case| case["files"].as_array().unwrap().len() > 1 && key(case) == [0; 4])
        .unwrap();
    let group_id = n(&case, "group") as u32;
    let mut bytes = resident(Limits::default())
        .read(3, group_id)
        .unwrap()
        .unwrap();
    // Edit the decompressed stripe count, then re-wrap as an uncompressed container.
    let mut data = rs09_cache::decompress(&bytes, 1_000_000).unwrap();
    *data.last_mut().unwrap() = 255;
    let mut writer = rs09_protocol::buffer::Writer::new();
    writer.p1(0);
    writer.p4(data.len() as i32);
    writer.pdata(&data);
    bytes = writer.into_vec();
    assert!(
        group::unpack(
            &bytes,
            archive.index.group(group_id).unwrap(),
            [0; 4],
            Limits::default()
        )
        .is_err()
    );
    let raw = resident(Limits::default())
        .read(3, group_id)
        .unwrap()
        .unwrap();
    assert!(
        group::unpack(
            &raw,
            archive.index.group(group_id).unwrap(),
            [0; 4],
            Limits {
                max_uncompressed: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    let indices: BTreeMap<_, _> = [(7, vec![255; 6])].into_iter().collect();
    assert!(
        Store::from_bytes(vec![0; 520], indices, Limits::default())
            .read(7, 0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn rt4_client_fallback_is_explicit_and_matches_original_failure_behavior() {
    let archive = Archive::open(resident(Limits::default()), 3, None, Limits::default()).unwrap();
    for case in cases("group_fallback") {
        let input = hex(case["input"].as_str().unwrap());
        let info = archive.index.group(n(&case, "group") as u32).unwrap();
        assert!(group::unpack(&input, info, key(&case), Limits::default()).is_err());
        let decoded = group::unpack_with_policy(
            &input,
            info,
            key(&case),
            Limits::default(),
            group::FailurePolicy::Rt4Client,
        )
        .unwrap();
        assert!(decoded.compatibility_fallback().is_some());
        for file in case["files"].as_array().unwrap() {
            assert_eq!(
                decoded.get(n(file, "id") as u32).unwrap().as_slice(),
                hex(file["expected"].as_str().unwrap())
            );
        }
    }
}
