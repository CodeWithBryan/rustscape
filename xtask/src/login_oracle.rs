use crate::{Result, run};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, atomic::Ordering},
};

pub(crate) fn invocation(root: &Path, args: &[String], script: &str) -> Result<Command> {
    let mut python = std::env::var_os("RS09_PYTHON").unwrap_or_else(|| "python".into());
    let mut forwarded = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--python" => {
                i += 1;
                python = args.get(i).ok_or("missing Python executable")?.into();
            }
            "--check" => forwarded.push(args[i].clone()),
            "--java-home" => {
                forwarded.push(args[i].clone());
                i += 1;
                forwarded.push(args.get(i).ok_or("missing JDK home")?.clone());
            }
            option => return Err(format!("unknown oracle option {option}").into()),
        }
        i += 1;
    }
    let mut command = Command::new(python);
    command
        .current_dir(root)
        .arg(root.join(script))
        .args(forwarded);
    Ok(command)
}
pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let output = run(&mut invocation(root, args, "oracle/login/generate.py")?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}

struct OracleChild(Child);
impl Drop for OracleChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn interop(root: &Path, args: &[String]) -> Result<()> {
    let limits = rs09_cache::Limits::default();
    let store = rs09_cache::store::Store::open(
        &root.join("fixtures/cache"),
        rs09_cache::store::StorageMode::Resident,
        limits,
    )?;
    let service = rs09_server::runtime::Service::new(
        Arc::new(rs09_server::catalog::Catalog::new(
            store, limits, 128, 1_000_000,
        )?),
        rs09_server::runtime::Config::default(),
        530,
    )?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .max_blocking_threads(8)
        .enable_all()
        .build()?;
    let server = runtime.block_on(service.start("127.0.0.1:0".parse()?, None))?;
    let mut command = invocation(root, args, "oracle/login/generate.py")?;
    command
        .args(["--check", "--interop-client"])
        .arg(server.tcp.to_string());
    let output = run(&mut command);
    runtime.block_on(server.shutdown())?;
    let output = output?;
    let output = String::from_utf8(output.stdout)?;
    let forward: Value =
        serde_json::from_str(output.lines().last().ok_or("missing JVM probe result")?)?;
    if forward["requests"] != 64 || service.metrics.login_packets.load(Ordering::Relaxed) != 64 {
        return Err("original login-writer probe request counts differ".into());
    }
    let setup: Value =
        serde_json::from_slice(&fs::read(root.join("runs/login-oracle/runtime.json"))?)?;
    let mut child = OracleChild(
        Command::new(setup["java"].as_str().ok_or("missing Java executable")?)
            .current_dir(root)
            .args([
                "-cp",
                setup["classpath"].as_str().ok_or("missing JVM classpath")?,
                "LoginProbe",
                "server",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let mut stdout = BufReader::new(child.0.stdout.take().ok_or("missing JVM server stdout")?);
    let mut line = String::new();
    stdout.read_line(&mut line)?;
    let ready: Value = serde_json::from_str(&line)?;
    let port = u16::try_from(
        ready["port"]
            .as_u64()
            .ok_or("missing oracle listener port")?,
    )?;
    runtime.block_on(async {
        use rs09_net::{Chunk, login::LoginPeer};
        use rs09_protocol::{
            login::{LoginRequest, RsaKey},
            profile::RS09_REVISION_530,
        };
        let codes = [2, 6, 21, 23, 29, 0, 2, 8];
        for (i, code) in codes.into_iter().enumerate() {
            let mut peer = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                LoginPeer::connect(([127, 0, 0, 1], port).into(), false, b"Rust_Client"),
            )
            .await??;
            let nonce = 0x12345678abcdef00u64 as i64 + i as i64;
            if peer.server_key != nonce {
                return Err("native peer read wrong original server key".into());
            }
            let request = LoginRequest {
                username: b"Rust_Client",
                password: b"test_password",
                reconnect: i % 2 != 0,
                isaac_seed: [17, 27, 0, 0],
                ..LoginRequest::default()
            };
            let reply = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                peer.submit(request, RS09_REVISION_530, &RsaKey::reference_public()),
            )
            .await??;
            if reply.code != code {
                return Err("native peer read wrong original login reply".into());
            }
            if code == 21 && reply.detail != Some(nonce as u8)
                || code == 29 && reply.detail != Some(17)
            {
                return Err("native peer read wrong original login detail".into());
            }
            if code == 2 {
                let world = reply.world.ok_or("missing original world login fields")?;
                if world.staff_level != 2
                    || world.player_index != 2047
                    || !world.parental_ad_consent
                    || !world.player_member
                    || !world.map_members
                {
                    return Err("native peer decoded wrong original world login fields".into());
                }
                let (mut reader, _writer, pending) = peer.into_transport();
                let mut bytes = pending.to_vec();
                while bytes.len() < 5 {
                    match tokio::time::timeout(std::time::Duration::from_secs(5), reader.read())
                        .await??
                    {
                        Chunk::Binary(chunk) => bytes.extend_from_slice(&chunk),
                        _ => return Err("lost game bytes after original login reply".into()),
                    }
                }
                if bytes != [1, 0, 2, 0xaa, 0xbb] {
                    return Err("wrong game bytes retained after original login reply".into());
                }
            }
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    let mut remainder = String::new();
    stdout.read_to_string(&mut remainder)?;
    if !child.0.wait()?.success() {
        return Err("JVM login server probe failed".into());
    }
    let reverse: Value = serde_json::from_str(remainder.trim())?;
    if reverse["requests"] != 8 {
        return Err("JVM login server probe counts differ".into());
    }
    if service.metrics.active.load(Ordering::Relaxed) != 0
        || service.metrics.queued_bytes.load(Ordering::Relaxed) != 0
    {
        return Err("login interop leaked session/byte permits".into());
    }
    let report = json!({"format":1,"upstream":crate::verify_reference(root)?,
        "scope":"Original codec methods over real TCP in both directions; explicit account/world/session adapters. No authenticated gameplay or player-capacity claim.",
        "jvmToNative":forward,"nativeToJvm":reverse,"nativeBuild":crate::network_oracle::native_build()?,
        "nativeSources":crate::network_oracle::native_sources(root)?,
        "oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/login/provenance.json"))?)?,
        "javaVersion":setup["javaVersion"],"activeAfterShutdown":0,"queuedAfterShutdown":0});
    let directory = root.join("runs/login-interop");
    fs::create_dir_all(&directory)?;
    fs::write(
        directory.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "Live login codec interoperability passed: 64 original JVM client requests, 8 native-to-JVM sessions."
    );
    Ok(())
}
