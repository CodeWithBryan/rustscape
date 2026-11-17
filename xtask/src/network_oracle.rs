use crate::{Result, normalized, run, source_paths, verify_reference};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn native_sources(root: &Path) -> Result<BTreeMap<String, String>> {
    fn collect(root: &Path, path: &Path, hashes: &mut BTreeMap<String, String>) -> Result<()> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                collect(root, &entry?.path(), hashes)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "rs")
            || path
                .file_name()
                .is_some_and(|name| name == "Cargo.toml" || name == "Cargo.lock")
        {
            hashes.insert(
                path.strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/"),
                format!("{:x}", Sha256::digest(normalized(&fs::read(path)?))),
            );
        }
        Ok(())
    }
    let mut hashes = BTreeMap::new();
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "crates/protocol",
        "crates/game-core",
        "crates/cache",
        "crates/net",
        "crates/server",
        "crates/headless",
        "xtask",
    ] {
        collect(root, &root.join(path), &mut hashes)?;
    }
    Ok(hashes)
}

pub(crate) fn native_build() -> Result<Value> {
    let rustc = run(Command::new("rustc").arg("--version"))?;
    let executable = std::env::current_exe()?;
    Ok(json!({"rustc": String::from_utf8(rustc.stdout)?.trim(),
        "profile": if cfg!(debug_assertions) {"debug"} else {"release"},
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "serviceHostExecutableSha256": format!("{:x}", Sha256::digest(fs::read(executable)?))}))
}

pub fn sources(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = source_paths(root)?;
    for name in [
        "Js5NetQueue",
        "Js5NetRequest",
        "Js5Request",
        "SecondaryNode",
        "SecondaryLinkedList",
    ] {
        result.push(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )));
    }
    result.push(root.join("reference/client/signlink/src/main/java/rt4/MonotonicClock.java"));
    for path in [
        "core/cache/Cache.java",
        "core/net/Constants.java",
        "core/net/IoReadEvent.java",
        "core/net/IoWriteEvent.java",
        "core/net/EventProducer.java",
        "core/net/event/HSReadEvent.java",
        "core/net/event/HSWriteEvent.java",
        "core/net/event/JS5ReadEvent.java",
        "core/net/event/JS5WriteEvent.java",
        "core/net/producer/HSEventProducer.java",
        "core/net/producer/JS5EventProducer.java",
        "core/net/lobby/WorldList.java",
        "core/net/lobby/WorldDefinition.java",
        "core/net/packet/IoBuffer.java",
        "core/net/packet/PacketHeader.java",
    ] {
        result.push(root.join("reference/server/Server/src/main").join(path));
    }
    fn add(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                add(&path, paths)?;
            } else if path
                .extension()
                .is_some_and(|extension| extension == "java")
            {
                paths.push(path);
            }
        }
        Ok(())
    }
    add(&root.join("oracle/network"), &mut result)?;
    result.sort();
    result.dedup();
    Ok(result)
}

pub fn interop(root: &Path) -> Result<()> {
    crate::cache_assets::verify(root)?;
    let paths = sources(root)?;
    let build = root.join("runs/network-interop");
    fs::create_dir_all(build.join("classes"))?;
    run(Command::new("javac")
        .args(["--release", "8", "-encoding", "UTF-8", "-d"])
        .arg(build.join("classes"))
        .args(&paths))?;
    let limits = rs09_cache::Limits::default();
    let store = rs09_cache::store::Store::open(
        &crate::cache_assets::directory(root),
        rs09_cache::store::StorageMode::Resident,
        limits,
    )?;
    let catalog = std::sync::Arc::new(rs09_server::catalog::Catalog::new(
        store,
        limits,
        4096,
        64 * 1024 * 1024,
    )?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .max_blocking_threads(8)
        .enable_all()
        .build()?;
    let address = runtime.block_on(async {
        let service = rs09_server::runtime::Service::new(
            catalog,
            rs09_server::runtime::Config::default(),
            530,
        )?;
        service.start("127.0.0.1:0".parse().unwrap(), None).await
    })?;
    let java = run(Command::new("java")
        .arg("-cp")
        .arg(build.join("classes"))
        .args(["NetworkProbe", "127.0.0.1"])
        .arg(address.tcp.port().to_string())
        .arg(crate::cache_assets::directory(root)));
    runtime.block_on(address.shutdown())?;
    let output = java?;
    let result: Value = serde_json::from_slice(&output.stdout)?;
    let mut hashes = BTreeMap::new();
    for path in paths {
        hashes.insert(
            path.strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/"),
            format!("{:x}", Sha256::digest(normalized(&fs::read(path)?))),
        );
    }
    hashes.extend(native_sources(root)?);
    let java_version = run(Command::new("java").arg("-version"))?;
    let report = json!({"format": 1, "upstream": verify_reference(root)?, "jvm": result,
        "sources": hashes, "assetObjects": crate::cache_assets::manifest(root)?,
        "nativeBuild": native_build()?, "javaVersion": String::from_utf8(java_version.stderr)?.trim(),
        "scope": "TCP interoperability with original cache client queue, master index and lobby world list; not authentication, gameplay, or player capacity"});
    fs::write(
        build.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "Original JVM client parsed {} native-server replies with matching data.",
        result["requests"]
    );
    Ok(())
}

pub fn smoke(root: &Path, args: &[String]) -> Result<()> {
    use std::sync::{Arc, atomic::Ordering};
    let mut clients = 500usize;
    let mut build = root.join("runs/network-smoke");
    let mut explicit_output = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--clients" => {
                i += 1;
                clients = args.get(i).ok_or("missing cache client count")?.parse()?;
            }
            "--output" => {
                if explicit_output {
                    return Err("duplicate network smoke output".into());
                }
                i += 1;
                build = root.join(
                    args.get(i)
                        .ok_or("missing network smoke output directory")?,
                );
                explicit_output = true;
            }
            other => return Err(format!("unknown network smoke option {other}").into()),
        }
        i += 1;
    }
    if !(1..=10_000).contains(&clients) {
        return Err("cache client count must be1..10000".into());
    }
    if explicit_output && build.exists() && fs::read_dir(&build)?.next().is_some() {
        return Err("isolated network smoke output must be empty".into());
    }
    crate::cache_assets::verify(root)?;
    let limits = rs09_cache::Limits::default();
    let store = rs09_cache::store::Store::open(
        &crate::cache_assets::directory(root),
        rs09_cache::store::StorageMode::Resident,
        limits,
    )?;
    let reference = store
        .read(255, 10)?
        .ok_or("missing pinned wordpack reference")?;
    let index = rs09_cache::index::Index::decode(&reference, None, limits)?;
    let group = index
        .group_by_ascii_name("huffman")
        .or_else(|| index.groups.first().map(|group| group.id))
        .ok_or("missing pinned Huffman group")?;
    let group = u16::try_from(group)?;
    let packed = store
        .read(10, u32::from(group))?
        .ok_or("missing pinned Huffman table")?;
    let length =
        u32::from_be_bytes(packed[1..5].try_into()?) as usize + if packed[0] == 0 { 5 } else { 9 };
    let mut hash = Sha256::new();
    for _ in 0..10 {
        hash.update(&packed[..length]);
    }
    let expected = format!("{:x}", hash.finalize());
    let catalog = Arc::new(rs09_server::catalog::Catalog::new(
        store,
        limits,
        4096,
        64 * 1024 * 1024,
    )?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .max_blocking_threads(8)
        .enable_all()
        .build()?;
    let service =
        rs09_server::runtime::Service::new(catalog, rs09_server::runtime::Config::default(), 530)?;
    let server =
        runtime.block_on(service.start("127.0.0.1:0".parse()?, Some("127.0.0.1:0".parse()?)))?;
    let executable = std::env::current_exe()?
        .parent()
        .ok_or("missing executable directory")?
        .join(format!("rs09-headless{}", std::env::consts::EXE_SUFFIX));
    let mut results = Vec::new();
    let result = (|| -> Result<()> {
        if !executable.is_file() {
            return Err("build rs09-headless in the same profile before network-smoke".into());
        }
        for websocket in [false, true] {
            let address = if websocket {
                server.websocket.unwrap()
            } else {
                server.tcp
            };
            let mut command = Command::new(&executable);
            command
                .current_dir(root)
                .args([
                    "--clients",
                    &clients.to_string(),
                    "--rounds",
                    "10",
                    "--address",
                ])
                .arg(address.to_string());
            command
                .args(["--archive", "10", "--group"])
                .arg(group.to_string());
            if websocket {
                command.arg("--websocket");
            }
            let output = run(&mut command)?;
            let report: Value = serde_json::from_slice(&output.stdout)?;
            if report["clients"].as_u64() != Some(clients as u64)
                || report["requests"].as_u64() != Some((clients * 10) as u64)
                || report["checksum"] != expected
            {
                return Err(
                    "headless executable results do not match pinned file bytes/counts".into(),
                );
            }
            println!(
                "{}: {clients} cache clients, {} verified requests in {}s.",
                if websocket { "WebSocket" } else { "TCP" },
                clients * 10,
                report["seconds"]
            );
            results.push(report);
        }
        Ok(())
    })();
    runtime.block_on(server.shutdown())?;
    result?;
    if service.metrics.active.load(Ordering::Relaxed) != 0
        || service.metrics.queued_bytes.load(Ordering::Relaxed) != 0
    {
        return Err("network smoke leaked session/queued-byte resources".into());
    }
    fs::create_dir_all(&build)?;
    let report = json!({"format": 1, "upstream": verify_reference(root)?, "results": results,
        "scope": format!("{clients} concurrent cache connections per backend; game actors and player capacity are outside this check"),
        "nativeBuild": native_build()?, "sources": native_sources(root)?,
        "headlessExecutableSha256": format!("{:x}", Sha256::digest(fs::read(executable)?)),
        "assetObjects": crate::cache_assets::manifest(root)?,
        "workload": {"repetitionsPerBackend": 1, "warmupRuns": 0, "clientsPerBackend": clients,
            "requestsPerClient": 10, "archive": 10, "group": group,
            "scheduling": "spawn all client tasks; no synchronized-start barrier",
            "timingIncludes": "connect, protocol handshake, requests, response hashing, close",
            "catalogReuse": "one catalog shared by TCP then WebSocket probes",
            "interpretation": "functional smoke run, not a throughput benchmark"},
        "serverRequests": service.metrics.requests.load(Ordering::Relaxed),
        "serverSentBytes": service.metrics.sent_bytes.load(Ordering::Relaxed),
        "peakQueuedBytes": service.metrics.peak_queued_bytes.load(Ordering::Relaxed),
        "activeAfterShutdown": service.metrics.active.load(Ordering::Relaxed),
        "queuedAfterShutdown": service.metrics.queued_bytes.load(Ordering::Relaxed),
        "packetPreparations": service.catalog.loads()});
    fs::write(
        build.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("Headless CLI probes completed and all server sessions/byte permits were released.");
    Ok(())
}
pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = verify_reference(root)?;
    let sources = sources(root)?;
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
    let build = root.join("runs/network-oracle");
    fs::create_dir_all(build.join("classes"))?;
    run(Command::new("javac")
        .args(["--release", "8", "-encoding", "UTF-8", "-d"])
        .arg(build.join("classes"))
        .args(&sources))?;
    let output = run(Command::new("java")
        .arg("-cp")
        .arg(build.join("classes"))
        .arg("NetworkOracle"))?;
    let fixture = normalized(&output.stdout);
    let mut count = 0;
    for line in fixture
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let _: Value = serde_json::from_slice(line)?;
        count += 1;
    }
    let provenance = json!({"format": 1, "upstream": upstream, "sources": hashes,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)), "caseCount": count,
        "sourceHashAlgorithm": "sha256, CR removed",
        "scope": "Complete original cache wire formatters, handshake/cache read handlers and WorldList/IoBuffer; explicit I/O, queue, RNG, metadata and game-state adapters. Authentication, scheduling and gameplay are not emulated."});
    let destination = root.join("fixtures/network");
    if check {
        if normalized(&fs::read(destination.join("oracle.jsonl"))?) != fixture
            || serde_json::from_slice::<Value>(&fs::read(destination.join("provenance.json"))?)?
                != provenance
        {
            return Err(
                "network oracle output/provenance differs; inspect before regenerating".into(),
            );
        }
        println!("{count} original JVM network cases reproduced.");
    } else {
        fs::create_dir_all(&destination)?;
        fs::write(destination.join("oracle.jsonl"), fixture)?;
        fs::write(
            destination.join("provenance.json"),
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {count} original JVM network cases.");
    }
    Ok(())
}
