//! Checks a cache directory against its own indexes, and optionally against
//! another directory group by group.
//!
//! `cargo run --release -p rs09-cache --example store_check -- DIR
//!     [--compare OTHER] [--keys XTEAS.json]`
//!
//! For every archive the master index (`idx255`) lists, each group of the
//! archive's index is read and its container checked the way the original
//! client checks a stored group: CRC over everything but the two-byte version
//! trailer, and the trailer against the low 16 bits of the index's version.
//! With `--compare` the same group is read from both directories and compared
//! as stored bytes without the trailer, as trailers, and as decoded files
//! (XTEA keys for the map archive come from `--keys`; a group that cannot be
//! decoded on either side is reported, not skipped silently).
//!
//! Prints one JSON object and exits non-zero when a comparison found a
//! difference. It reads both stores and writes nothing.
use rs09_cache::{
    Limits,
    group::{FailurePolicy, unpack_with_policy},
    index::Index,
    store::{StorageMode, Store},
};
use rs09_protocol::checksum::crc32;
use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error, path::Path};

/// Length of the container proper: header, lengths and payload, no trailer.
fn container_length(stored: &[u8]) -> Option<usize> {
    let header = stored.get(..5)?;
    let length = usize::try_from(i32::from_be_bytes(header[1..].try_into().ok()?)).ok()?;
    length.checked_add(if header[0] == 0 { 5 } else { 9 })
}

fn region_keys(path: &Path) -> Result<BTreeMap<u32, [u32; 4]>, Box<dyn Error>> {
    let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let mut keys = BTreeMap::new();
    for entry in value["xteas"].as_array().ok_or("unexpected key file")? {
        let region = entry["regionId"]
            .as_str()
            .ok_or("missing region")?
            .parse()?;
        let words: Vec<u32> = entry["keys"]
            .as_str()
            .ok_or("missing key")?
            .split(',')
            .map(|word| word.trim().parse::<i64>().map(|word| word as u32))
            .collect::<Result<_, _>>()?;
        keys.insert(
            region,
            words.try_into().map_err(|_| "key needs four words")?,
        );
    }
    Ok(keys)
}

fn index_of(store: &Store, archive: u8, limits: Limits) -> Result<Option<Index>, Box<dyn Error>> {
    let Some(stored) = store.read(255, u32::from(archive))? else {
        return Ok(None);
    };
    Ok(Some(Index::decode(&stored, None, limits)?))
}

fn survey(store: &Store, limits: Limits) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut archives = Vec::new();
    for archive in 0..store.group_slots(255).min(255) as u8 {
        let Some(stored) = store.read(255, u32::from(archive))? else {
            archives.push(json!({"archive": archive, "index": "absent"}));
            continue;
        };
        let index = Index::decode(&stored, None, limits)?;
        let index_extra = container_length(&stored).map(|n| stored.len() as i64 - n as i64);
        let (mut present, mut missing, mut crc_bad, mut version_bad) = (0, 0, 0, 0);
        let (mut no_trailer, mut odd_length, mut bytes, mut largest) = (0, 0, 0usize, 0usize);
        let mut examples = Vec::new();
        for group in index.groups.iter().filter(|group| !group.files.is_empty()) {
            let Some(stored) = store.read(archive, group.id)? else {
                missing += 1;
                if examples.len() < 8 {
                    examples.push(json!({"group": group.id, "problem": "missing"}));
                }
                continue;
            };
            present += 1;
            bytes += stored.len();
            largest = largest.max(stored.len());
            let Some(length) = container_length(&stored).filter(|&n| n <= stored.len()) else {
                odd_length += 1;
                continue;
            };
            match stored.len() - length {
                0 => no_trailer += 1,
                2 => {
                    let version = u16::from_be_bytes([stored[length], stored[length + 1]]);
                    if i32::from(version) != group.version & 0xffff {
                        version_bad += 1;
                    }
                }
                _ => odd_length += 1,
            }
            // The client's rule: everything but the last two stored bytes.
            if stored.len() <= 2 || crc32(&stored[..stored.len() - 2]) != group.checksum {
                crc_bad += 1;
                if examples.len() < 8 {
                    examples.push(json!({"group": group.id, "problem": "crc",
                        "trailerBytes": stored.len() - length}));
                }
            }
        }
        archives.push(json!({"archive": archive, "indexCrc": index.checksum,
            "indexVersion": index.version, "indexFormat": index.format,
            "indexBytesPastContainer": index_extra,
            "groups": index.groups.iter().filter(|group| !group.files.is_empty()).count(),
            "emptyGroups": index.groups.iter().filter(|group| group.files.is_empty()).count(),
            "present": present, "missing": missing, "storedBytes": bytes, "largestStored": largest,
            "crcMismatch": crc_bad, "trailerVersionMismatch": version_bad,
            "withoutTrailer": no_trailer, "oddLength": odd_length, "examples": examples}));
    }
    Ok(archives)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let option = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let directory = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage: store_check DIR [--compare OTHER] [--keys XTEAS.json]")?;
    let limits = Limits::default();
    let store = Store::open(directory.as_ref(), StorageMode::Resident, limits)?;
    let mut report = json!({"directory": directory, "archives": survey(&store, limits)?});
    let mut differences = 0usize;
    if let Some(other_directory) = option("--compare") {
        let other = Store::open(other_directory.as_ref(), StorageMode::Resident, limits)?;
        let keys = match option("--keys") {
            Some(path) => region_keys(path.as_ref())?,
            None => BTreeMap::new(),
        };
        let mut archives = Vec::new();
        let slots = store.group_slots(255).max(other.group_slots(255)).min(255) as u8;
        for archive in 0..slots {
            let (Some(index), Some(other_index)) = (
                index_of(&store, archive, limits)?,
                index_of(&other, archive, limits)?,
            ) else {
                let absent = index_of(&store, archive, limits)?.is_none()
                    && index_of(&other, archive, limits)?.is_none();
                if !absent {
                    differences += 1;
                }
                archives.push(json!({"archive": archive, "index": if absent {
                    "absent in both" } else { "present in only one" }}));
                continue;
            };
            let index_equal =
                store.read(255, u32::from(archive))? == other.read(255, u32::from(archive))?;
            let group_keys: BTreeMap<u32, [u32; 4]> = if archive == 5 {
                keys.iter()
                    .filter_map(|(&region, &key)| {
                        index
                            .group_by_ascii_name(&format!("l{}_{}", region >> 8, region & 255))
                            .map(|group| (group, key))
                    })
                    .collect()
            } else {
                BTreeMap::new()
            };
            let (mut groups, mut packed_equal, mut trailer_equal, mut stored_equal) = (0, 0, 0, 0);
            let (mut decoded_groups, mut decoded_files, mut decoded_bytes) = (0, 0, 0usize);
            let (mut undecodable, mut keyed, mut only_one) = (0, 0, 0);
            let mut examples = Vec::new();
            let note = |examples: &mut Vec<Value>, group: u32, problem: &str| {
                if examples.len() < 8 {
                    examples.push(json!({"group": group, "problem": problem}));
                }
            };
            for group in index.groups.iter().filter(|group| !group.files.is_empty()) {
                groups += 1;
                let (Some(a), Some(b)) = (
                    store.read(archive, group.id)?,
                    other.read(archive, group.id)?,
                ) else {
                    only_one += 1;
                    differences += 1;
                    note(&mut examples, group.id, "not readable in both stores");
                    continue;
                };
                if a == b {
                    stored_equal += 1;
                }
                let (Some(a_length), Some(b_length)) = (
                    container_length(&a).filter(|&n| n <= a.len()),
                    container_length(&b).filter(|&n| n <= b.len()),
                ) else {
                    differences += 1;
                    note(&mut examples, group.id, "malformed container");
                    continue;
                };
                if a[..a_length] == b[..b_length] {
                    packed_equal += 1;
                } else {
                    differences += 1;
                    note(&mut examples, group.id, "container bytes differ");
                }
                if a[a_length..] == b[b_length..] {
                    trailer_equal += 1;
                }
                let key = group_keys.get(&group.id).copied().unwrap_or([0; 4]);
                if key != [0; 4] {
                    keyed += 1;
                }
                let unpack = |bytes: &[u8]| {
                    unpack_with_policy(bytes, group, key, limits, FailurePolicy::Strict)
                };
                match (unpack(&a[..a_length]), unpack(&b[..b_length])) {
                    (Ok(a), Ok(b)) => {
                        let same = a.iter().count() == b.iter().count()
                            && a.iter().zip(b.iter()).all(|(a, b)| a == b);
                        if same {
                            decoded_groups += 1;
                            decoded_files += a.iter().count();
                            decoded_bytes += a.byte_len();
                        } else {
                            differences += 1;
                            note(&mut examples, group.id, "decoded files differ");
                        }
                    }
                    (Err(_), Err(_)) => undecodable += 1,
                    _ => {
                        differences += 1;
                        note(&mut examples, group.id, "decodes in only one store");
                    }
                }
            }
            let other_groups = other_index
                .groups
                .iter()
                .filter(|group| !group.files.is_empty())
                .count();
            if !index_equal || other_groups != groups {
                differences += 1;
            }
            archives.push(
                json!({"archive": archive, "indexContainerEqual": index_equal,
                "groups": groups, "groupsInOther": other_groups,
                "containerEqual": packed_equal, "trailerEqual": trailer_equal,
                "storedBytesEqual": stored_equal, "notReadableInBoth": only_one,
                "decodedEqualGroups": decoded_groups, "decodedEqualFiles": decoded_files,
                "decodedEqualBytes": decoded_bytes, "decodedWithKey": keyed,
                "undecodableInBoth": undecodable, "examples": examples}),
            );
        }
        report["compare"] = json!({"other": other_directory, "regionKeys": keys.len(),
            "differences": differences, "archives": archives});
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    if differences != 0 {
        return Err(format!("{differences} differences").into());
    }
    Ok(())
}
