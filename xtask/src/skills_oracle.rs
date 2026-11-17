use crate::{Result, run};
use rs09_server::player::skills::Skills;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, hint::black_box, path::Path, process::Command, time::Instant};
pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let output = run(&mut crate::login_oracle::invocation(
        root,
        args,
        "oracle/skills/generate.py",
    )?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}
fn lookup(queries: &[f64], passes: usize) -> u64 {
    let mut checksum = 0u64;
    for _ in 0..passes {
        for &query in queries {
            checksum = checksum
                .wrapping_mul(31)
                .wrapping_add(Skills::level_from_xp(black_box(query)) as u64);
        }
    }
    black_box(checksum)
}
pub fn benchmark(root: &Path, args: &[String]) -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("run cargo run --release -p xtask -- skills-bench".into());
    }
    let corpus = fs::read(root.join("fixtures/skills/oracle.jsonl"))?;
    let thresholds = String::from_utf8(corpus.clone())?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|case| {
            case["kind"] == "curve" && (1..=99).contains(&case["level"].as_i64().unwrap())
        })
        .map(|case| case["expected"].as_i64().unwrap() as f64)
        .collect::<Vec<_>>();
    if thresholds.len() != 99 {
        return Err("missing original benchmark curve".into());
    }
    let mut random = rs09_game_core::random::JavaRandom::new(530);
    let queries = (0..1024)
        .map(|i| match i % 3 {
            0 => random.next_double() * 200_000_000.0,
            1 => random.next_double() * 13_034_431.0,
            _ => thresholds[i % 99] + random.next_double() - 0.5,
        })
        .collect::<Vec<_>>();
    let input = json!({"queries":queries.iter().map(|value|format!("{:016x}",value.to_bits())).collect::<Vec<_>>(),"warmupPasses":128,"passes":256,"repetitions":7});
    let encoded = serde_json::to_vec_pretty(&input)?;
    let directory = root.join("runs/skills-benchmark");
    fs::create_dir_all(&directory)?;
    let input_file = directory.join("queries.json");
    fs::write(&input_file, &encoded)?;
    let mut invocation = crate::login_oracle::invocation(root, args, "oracle/skills/generate.py")?;
    invocation
        .arg("--check")
        .arg("--benchmark-file")
        .arg(&input_file);
    let output = String::from_utf8(run(&mut invocation)?.stdout)?;
    let original: Value = serde_json::from_str(
        output
            .lines()
            .last()
            .ok_or("missing skill benchmark result")?,
    )?;
    lookup(&queries, 128);
    let mut native = Vec::new();
    let mut checksum = 0;
    for _ in 0..7 {
        let start = Instant::now();
        checksum = lookup(&queries, 256);
        native.push(start.elapsed().as_nanos() as u64);
    }
    if original["checksum"] != format!("{checksum:016x}") {
        return Err("skill benchmark checksums differ".into());
    }
    let mut reference_samples = original["samplesNs"]
        .as_array()
        .ok_or("missing reference samples")?
        .iter()
        .map(|n| n.as_u64().ok_or("invalid reference sample"))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if reference_samples.len() != 7 {
        return Err("reference repetition count differs".into());
    }
    reference_samples.sort_unstable();
    let mut native_samples = native.clone();
    native_samples.sort_unstable();
    let ratio = reference_samples[3] as f64 / native_samples[3] as f64;
    let cpu = if cfg!(windows) {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_Processor | Select-Object -First 1).Name",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        String::from_utf8(run(&mut command)?.stdout)?
            .trim()
            .to_owned()
    } else {
        std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unavailable".into())
    };
    let report = json!({"format":1,"upstream":crate::verify_reference(root)?,"name":"Skills.levelFromXP","scope":"Single-threaded XP lookup only: unmodified Java method versus cached native thresholds. Includes the checksum loop, excludes process startup, compilation, query construction, entity setup and curve/JIT warmup. No gameplay tick, networking or player-capacity claim.","hardware":{"cpu":cpu,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"logicalProcessors":std::thread::available_parallelism()?.get()},"nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,"oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/skills/provenance.json"))?)?,"fixtureSha256":format!("{:x}",Sha256::digest(&corpus)),"querySha256":format!("{:x}",Sha256::digest(encoded)),"corpus":input,"queryCountPerSample":1024*256,"checksum":format!("{checksum:016x}"),"jvm":original,"nativeSamplesNs":native,"jvmOverNativeMedian":ratio});
    fs::write(
        directory.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "XP lookup: JVM median {:.3} ms, Rust median {:.3} ms, ratio {:.2}x; checksum {checksum:016x}.",
        reference_samples[3] as f64 / 1e6,
        native_samples[3] as f64 / 1e6,
        ratio
    );
    Ok(())
}
