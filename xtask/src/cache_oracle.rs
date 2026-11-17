use crate::{Result, cache_assets, git, normalized, run, source_paths, verify_reference};
use rs09_cache::{
    Limits,
    archive::Archive,
    index::Index,
    store::{StorageMode, Store},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command, time::Instant};

const FILES: &[&str] = &[
    "main_file_cache.dat2",
    "main_file_cache.idx3",
    "main_file_cache.idx7",
    "main_file_cache.idx255",
];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = verify_reference(root)?;
    let sources = source_paths(root)?;
    let mut hashes = BTreeMap::new();
    for source in &sources {
        hashes.insert(
            source
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/"),
            format!("{:x}", Sha256::digest(normalized(&fs::read(source)?))),
        );
    }
    let build = root.join("runs/cache-oracle");
    fs::create_dir_all(build.join("classes"))?;
    run(Command::new("javac")
        .args(["--release", "8", "-encoding", "UTF-8", "-d"])
        .arg(build.join("classes"))
        .args(&sources))?;
    let output = run(Command::new("java")
        .arg("-cp")
        .arg(build.join("classes"))
        .args(["CacheOracle", "fixtures"])
        .arg(build.join("data")))?;
    let fixture = normalized(&output.stdout);
    let mut count = 0;
    for line in fixture
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let _: Value = serde_json::from_slice(line)?;
        count += 1;
    }
    let mut inputs = BTreeMap::new();
    for name in FILES {
        let data = fs::read(build.join("data").join(name))?;
        inputs.insert(*name, format!("{:x}", Sha256::digest(&data)));
    }
    let metadata = json!({
        "format": 1, "upstream": upstream, "sources": hashes, "inputs": inputs,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)), "caseCount": count,
        "scope": "Unmodified Cache/BufferedFile/FileOnDisk/Js5Index/Js5 with byte-copy JagString adapter. Synthetic inputs and a test resource provider. No live-world integration claim."
    });
    let destination = root.join("fixtures/cache");
    if check {
        if normalized(&fs::read(destination.join("oracle.jsonl"))?) != fixture
            || serde_json::from_slice::<Value>(&fs::read(destination.join("provenance.json"))?)?
                != metadata
        {
            return Err(
                "cache oracle outputs/provenance differ; inspect changes before regeneration"
                    .into(),
            );
        }
        for name in FILES {
            if fs::read(destination.join(name))? != fs::read(build.join("data").join(name))? {
                return Err(format!("cache oracle input differs: {name}").into());
            }
        }
        println!(
            "{count} JVM cache oracle cases reproduced with matching inputs and source hashes."
        );
    } else {
        fs::create_dir_all(&destination)?;
        fs::write(destination.join("oracle.jsonl"), fixture)?;
        fs::write(
            destination.join("provenance.json"),
            format!("{}\n", serde_json::to_string_pretty(&metadata)?),
        )?;
        for name in FILES {
            fs::copy(build.join("data").join(name), destination.join(name))?;
        }
        println!(
            "Generated {count} JVM cache oracle cases and {} binary input files.",
            FILES.len()
        );
    }
    Ok(())
}

fn region_keys(root: &Path) -> Result<BTreeMap<u32, [u32; 4]>> {
    let source = git(
        &root.join("reference/server"),
        &["show", "HEAD:Server/data/configs/xteas.json"],
    )?;
    let value: Value = serde_json::from_str(&source)?;
    let entries = value["xteas"]
        .as_array()
        .ok_or("unexpected pinned XTEA format")?;
    let mut result = BTreeMap::new();
    for entry in entries {
        let id = entry["regionId"]
            .as_str()
            .ok_or("missing XTEA region ID")?
            .parse::<u32>()?;
        let numbers: Vec<_> = entry["keys"]
            .as_str()
            .ok_or("missing XTEA key")?
            .split(',')
            .map(|value| value.trim().parse::<i64>().map(|value| value as u32))
            .collect::<std::result::Result<_, _>>()?;
        result.insert(
            id,
            numbers
                .try_into()
                .map_err(|_| "XTEA key must have four words")?,
        );
    }
    Ok(result)
}

fn keys_by_group(index: &Index, keys: &BTreeMap<u32, [u32; 4]>) -> BTreeMap<u32, [u32; 4]> {
    keys.iter()
        .filter_map(|(&region, &key)| {
            index
                .group_by_ascii_name(&format!("l{}_{}", region >> 8, region & 255))
                .map(|group| (group, key))
        })
        .collect()
}

pub fn audit(root: &Path, jvm: bool) -> Result<()> {
    cache_assets::verify(root)?;
    let start = Instant::now();
    let limits = Limits {
        cached_groups: 0,
        ..Limits::default()
    };
    let store = Store::open(
        &cache_assets::directory(root),
        StorageMode::Resident,
        limits,
    )?;
    let keys = region_keys(root)?;
    let mut results = Vec::new();
    let mut failures = Vec::new();
    for id in store.archives().filter(|&id| id != 255) {
        let archive = Archive::open_with_policy(
            store.clone(),
            id,
            None,
            limits,
            rs09_cache::group::FailurePolicy::Rt4Client,
        )?;
        let group_keys = if id == 5 {
            keys_by_group(&archive.index, &keys)
        } else {
            BTreeMap::new()
        };
        let mut hash = Sha256::new();
        let mut groups = 0usize;
        let mut files = 0usize;
        let mut bytes = 0usize;
        let mut fallbacks = Vec::new();
        for info in &archive.index.groups {
            if info.capacity == 0 {
                continue;
            }
            let key = group_keys.get(&info.id).copied().unwrap_or([0; 4]);
            match archive.group(info.id, key) {
                Ok(Some(group)) => {
                    if group.compatibility_fallback().is_some() {
                        fallbacks.push(info.id);
                    }
                    for (file, data) in group.iter() {
                        hash.update(info.id.to_be_bytes());
                        hash.update(file.to_be_bytes());
                        hash.update((data.len() as u32).to_be_bytes());
                        hash.update(data);
                        files += 1;
                        bytes += data.len();
                    }
                    groups += 1;
                }
                Ok(None) => failures.push(format!("{id}/{} missing", info.id)),
                Err(error) => failures.push(format!("{id}/{}: {error}", info.id)),
            }
        }
        println!("Archive {id}: {groups} groups, {files} files, {bytes} decoded bytes");
        results.push(
            json!({"archive": id, "groups": groups, "files": files, "bytes": bytes,
            "sha256": format!("{:x}", hash.finalize()), "clientFallbackGroups": fallbacks}),
        );
    }
    let output = root.join("runs/cache-audit");
    fs::create_dir_all(&output)?;
    let elapsed = start.elapsed().as_secs_f64();
    let mut hashes = BTreeMap::new();
    for path in source_paths(root)? {
        hashes.insert(
            path.strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/"),
            format!("{:x}", Sha256::digest(normalized(&fs::read(path)?))),
        );
    }
    for path in [
        "Cargo.lock",
        "crates/cache/src/lib.rs",
        "crates/cache/src/store.rs",
        "crates/cache/src/index.rs",
        "crates/cache/src/group.rs",
        "crates/cache/src/archive.rs",
        "crates/protocol/src/buffer.rs",
        "crates/protocol/src/xtea.rs",
        "crates/protocol/src/checksum.rs",
        "xtask/src/cache_oracle.rs",
        "xtask/src/cache_assets.rs",
    ] {
        hashes.insert(
            path.to_owned(),
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(path))?))
            ),
        );
    }
    let key_source = git(
        &root.join("reference/server"),
        &["show", "HEAD:Server/data/configs/xteas.json"],
    )?;
    let rustc = run(Command::new("rustc").arg("--version"))?;
    let mut report = json!({"format": 1, "upstream": verify_reference(root)?, "archives": results,
        "failures": failures, "elapsedSeconds": start.elapsed().as_secs_f64(),
        "nativeExtractionSeconds": elapsed, "assetObjects": cache_assets::manifest(root)?,
        "xteaSourceSha256": format!("{:x}", Sha256::digest(normalized(key_source.as_bytes()))),
        "sourceSha256": hashes, "jvmVerified": false,
        "rustc": String::from_utf8_lossy(&rustc.stdout).trim(),
        "scope": "Decoded cache file bytes in RT4 compatibility mode, including explicit original fallback groups; not interpreted definitions, live gameplay, or a performance benchmark."});
    fs::write(
        output.join("native.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    if jvm {
        fs::write(
            output.join("comparison.json"),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        let sources = source_paths(root)?;
        fs::create_dir_all(output.join("classes"))?;
        run(Command::new("javac")
            .args(["--release", "8", "-encoding", "UTF-8", "-d"])
            .arg(output.join("classes"))
            .args(sources))?;
        let key_path = output.join("keys.txt");
        let text: String = keys
            .iter()
            .map(|(region, key)| format!("{region} {} {} {} {}\n", key[0], key[1], key[2], key[3]))
            .collect();
        fs::write(&key_path, text)?;
        let jvm = run(Command::new("java")
            .arg("-Xmx2g")
            .arg("-XX:MaxDirectMemorySize=512m")
            .arg("-cp")
            .arg(output.join("classes"))
            .args(["CacheOracle", "audit"])
            .arg(cache_assets::directory(root))
            .arg(key_path))?;
        let mut comparison = Vec::new();
        let mut fallbacks = Vec::new();
        for line in String::from_utf8(jvm.stdout)?.lines() {
            if let Some(diagnostic) = line.strip_prefix("T3 - ") {
                if let Some(group) = diagnostic
                    .split(',')
                    .nth(1)
                    .and_then(|group| group.parse::<u32>().ok())
                {
                    fallbacks.push(group);
                }
                continue;
            }
            let mut result: Value = match serde_json::from_str(line) {
                Ok(result) => result,
                Err(_) => continue, // Original Js5 can print corruption diagnostics.
            };
            if result["archive"].is_null() {
                continue;
            }
            result["clientFallbackGroups"] = serde_json::to_value(&fallbacks)?;
            fallbacks.clear();
            comparison.push(result);
        }
        fs::write(
            output.join("jvm.json"),
            format!("{}\n", serde_json::to_string_pretty(&comparison)?),
        )?;
        if comparison.len() != report["archives"].as_array().unwrap().len() {
            return Err("JVM/native archive counts differ".into());
        }
        for (native, jvm) in report["archives"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&comparison)
        {
            for field in [
                "archive",
                "groups",
                "files",
                "bytes",
                "sha256",
                "clientFallbackGroups",
            ] {
                if native[field] != jvm[field] {
                    return Err(format!(
                        "JVM/native archive {} differs in {field}: {} != {}",
                        native["archive"], native[field], jvm[field]
                    )
                    .into());
                }
            }
            if !jvm["failures"]
                .as_array()
                .ok_or("missing JVM failures")?
                .is_empty()
            {
                return Err(format!("JVM cache failures in archive {}", jvm["archive"]).into());
            }
        }
        println!(
            "Original JVM and Rust produced identical decoded bytes in all {} archives.",
            comparison.len()
        );
        let version = run(Command::new("java").arg("-version"))?;
        report["java"] = json!(String::from_utf8_lossy(&version.stderr).trim());
        report["jvmVerified"] = json!(true);
        fs::write(
            output.join("comparison.json"),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
    }
    if !report["failures"].as_array().unwrap().is_empty() {
        return Err(format!(
            "native cache audit found {} failures; see runs/cache-audit/native.json",
            report["failures"].as_array().unwrap().len()
        )
        .into());
    }
    println!("Cache extraction audit passed; native extraction took {elapsed:.2}s.");
    Ok(())
}
