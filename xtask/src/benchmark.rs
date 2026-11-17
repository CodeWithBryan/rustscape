use crate::{Result, normalized, run, source_paths, verify_reference};
use rs09_protocol::{buffer::Reader, isaac::Isaac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, hint::black_box, path::Path, process::Command, time::Instant};

fn workload(name: &str, count: usize) -> u64 {
    let mut checksum = 0u64;
    match name {
        "isaac_keys" => {
            let mut random = Isaac::new(&[1, 2, 3, 4]).unwrap();
            for _ in 0..count {
                checksum = checksum
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(random.next_key()));
            }
        }
        "buffer_g4" => {
            let input: Vec<_> = (0..1024).map(|i| (i * 17 + i / 7) as u8).collect();
            let mut reader = Reader::new(&input);
            for _ in 0..count {
                if reader.remaining() == 0 {
                    reader.seek(0).unwrap();
                }
                checksum = checksum
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(reader.g4().unwrap() as u32));
            }
        }
        _ => unreachable!(),
    }
    checksum
}

pub fn run_benchmarks(root: &Path) -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("run benchmarks with cargo run --release -p xtask -- bench".into());
    }
    let upstream = verify_reference(root)?;
    let build = root.join("runs/bench");
    fs::create_dir_all(build.join("classes"))?;
    run(Command::new("javac")
        .args(["--release", "8", "-encoding", "UTF-8", "-d"])
        .arg(build.join("classes"))
        .args(source_paths(root)?))?;
    let count = 4_000_000;
    let java = run(Command::new("java")
        .arg("-cp")
        .arg(build.join("classes"))
        .arg("OracleBenchmark")
        .arg(count.to_string()))?;
    let mut results = Vec::new();
    for line in String::from_utf8(java.stdout)?.lines() {
        let java: Value = serde_json::from_str(line)?;
        let name = java["name"].as_str().ok_or("missing benchmark name")?;
        for _ in 0..20 {
            black_box(workload(name, count));
        }
        let mut samples = Vec::new();
        let mut checksum = 0;
        for _ in 0..9 {
            let start = Instant::now();
            checksum = black_box(workload(name, black_box(count)));
            samples.push(start.elapsed().as_nanos() as u64);
        }
        if java["checksum"]
            .as_str()
            .ok_or("missing checksum")?
            .parse::<u64>()?
            != checksum
        {
            return Err(format!("{name} benchmark checksums differ").into());
        }
        let mut java_samples: Vec<_> = java["samplesNs"]
            .as_array()
            .ok_or("missing JVM samples")?
            .iter()
            .map(|value| value.as_u64().ok_or("invalid sample"))
            .collect::<std::result::Result<_, _>>()?;
        java_samples.sort_unstable();
        let mut native_samples = samples.clone();
        native_samples.sort_unstable();
        let median = java_samples.len() / 2;
        let ratio = java_samples[median] as f64 / native_samples[median] as f64;
        println!(
            "{name}: JVM median {:.2} ms, Rust median {:.2} ms, JVM/Rust {:.2}x; checksum {checksum}",
            java_samples[median] as f64 / 1e6,
            native_samples[median] as f64 / 1e6,
            ratio
        );
        results.push(
            json!({"name": name, "iterations": count, "checksum": checksum.to_string(),
            "jvmSamplesNs": java["samplesNs"], "rustSamplesNs": samples, "jvmOverRust": ratio}),
        );
    }
    let rustc = run(Command::new("rustc").arg("--version"))?;
    let java_version = run(Command::new("java").arg("-version"))?;
    let source_hashes: std::collections::BTreeMap<_, _> = [
        "crates/protocol/src/isaac.rs",
        "crates/protocol/src/buffer.rs",
        "oracle/OracleBenchmark.java",
        "xtask/src/benchmark.rs",
        "Cargo.lock",
    ]
    .into_iter()
    .map(|name| {
        Ok((
            name,
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(name))?))
            ),
        ))
    })
    .collect::<Result<_>>()?;
    let report = json!({
        "format": 1, "upstream": upstream, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "rustc": String::from_utf8_lossy(&rustc.stdout).trim(),
        "java": String::from_utf8_lossy(&java_version.stderr).trim(),
        "profile": "release; thin LTO; codegen-units=1", "warmups": 20, "samples": 9,
        "scope": "Synthetic isolated primitives, sequential process runs; includes per-run object setup, excludes process startup. No world, content, networking, AI inference, or player-capacity claim.",
        "corpus": "ISAAC seed [1,2,3,4]; g4 input[i]=(i*17+i/7) modulo 256, 1024 bytes repeatedly read.",
        "sourceSha256": source_hashes, "results": results
    });
    fs::write(
        build.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("Report saved to {}", build.join("report.json").display());
    Ok(())
}
