//! `cargo xtask actors-oracle [--check]`: the reference client's actor code
//! (BAS decoding, movement acceleration, accelerated turning, body lean,
//! scene insertion with crowd thinning, projectiles and the CS2 5401 colour
//! conversion) run in a JVM; its observations become `fixtures/actors`.
use crate::{Result, cache_assets, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    // Observed.
    "BasType",
    "NpcList",
    "PathingEntity",
    "SeqType",
    "ProjAnim",
    "ColorUtils",
    // Their unchanged dependencies.
    "Entity",
    "SlotAnimation",
    "MathUtils",
    "Buffer",
    "IntUtils",
    "Node",
    "LinkedList",
    "HashTable",
    "ByteArrayNode",
    "ArrayUtils",
    "GlobalConfig",
    "Packet",
    "IsaacRandom",
    "RandomUtils",
    "ThreadUtils",
    "Js5Compression",
    "GzipDecompressor",
    "Bzip2Decompressor",
    "Bzip2DState",
    "Js5Index",
    "IntHashTable",
    "Js5",
    "Js5ResourceProvider",
    "ByteArray",
    "DirectByteArray",
    "Cache",
    "BufferedFile",
];
/// Reference files from which single methods are copied verbatim into an
/// adapter class: `(template, [(file, signature prefix)])` is read from the
/// `//@EXTRACT` lines of each template.
const TEMPLATES: &[&str] = &["Player", "ScriptRunner"];
const ADAPTERS: &[&str] = &[
    "AnimFrameset",
    "BasTypeList",
    "Camera",
    "client",
    "FogManager",
    "GlModel",
    "GlRenderer",
    "LoginManager",
    "Model",
    "Npc",
    "NpcType",
    "ParticleSystem",
    "PlayerList",
    "Preferences",
    "SceneGraph",
    "SeqTypeList",
    "SoundPlayer",
    "SpotAnimType",
    "SpotAnimTypeList",
];
const OTHER: &[&str] = &[
    "reference/client/signlink/src/main/java/rt4/MonotonicClock.java",
    "reference/client/signlink/src/main/java/rt4/FileOnDisk.java",
    "oracle/adapters/rt4/JagString.java",
    "oracle/music/adapters/rt4/SignLink.java",
    "oracle/actors/ActorsOracle.java",
];
const SEED: &str = "20261116";
/// Cache files the BAS sweep reads.
const INPUTS: &[&str] = &[
    "main_file_cache.dat2",
    "main_file_cache.idx2",
    "main_file_cache.idx255",
];
const JAVA: &str = "reference/client/client/src/main/java/rt4";

/// The text of one method, from the line holding `signature` to its closing
/// brace. The reference sources are tab-indented with one tab for members,
/// so the method ends at the first following line that is exactly "\t}".
fn method(source: &str, file: &str, signature: &str) -> Result<String> {
    let mut lines = source.lines();
    let mut text = String::new();
    for line in lines.by_ref() {
        if line.trim_start().starts_with(signature) {
            text.push_str(line);
            text.push('\n');
            break;
        }
    }
    if text.is_empty() {
        return Err(format!("{file}: no method starting `{signature}`").into());
    }
    for line in lines {
        text.push_str(line);
        text.push('\n');
        if line == "\t}" {
            let (open, close) = (text.matches('{').count(), text.matches('}').count());
            if open != close {
                return Err(format!("{file}: unbalanced braces in `{signature}`").into());
            }
            return Ok(text);
        }
    }
    Err(format!("{file}: `{signature}` has no end").into())
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    cache_assets::verify(root)?;
    let upstream = crate::verify_reference(root)?;
    let build = root.join("runs/actors-oracle");
    let (classes, generated) = (build.join("classes"), build.join("generated/rt4"));
    for directory in [&classes, &generated] {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
        fs::create_dir_all(directory)?;
    }
    let mut hashed: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{JAVA}/{name}.java"))
        .chain(
            ADAPTERS
                .iter()
                .map(|name| format!("oracle/actors/adapters/rt4/{name}.java")),
        )
        .chain(
            TEMPLATES
                .iter()
                .map(|name| format!("oracle/actors/templates/rt4/{name}.java")),
        )
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    let mut sources: Vec<String> = hashed.clone();
    sources.retain(|path| !path.starts_with("oracle/actors/templates/"));
    let mut extracted = Vec::new();
    for name in TEMPLATES {
        let template = String::from_utf8(normalized(&fs::read(
            root.join(format!("oracle/actors/templates/rt4/{name}.java")),
        )?))?;
        let mut text = String::new();
        for line in template.lines() {
            let Some(request) = line.trim_start().strip_prefix("//@EXTRACT ") else {
                text.push_str(line);
                text.push('\n');
                continue;
            };
            let (file, signature) = request.split_once(' ').ok_or("malformed //@EXTRACT line")?;
            let path = format!("{JAVA}/{file}");
            let source = String::from_utf8(normalized(&fs::read(root.join(&path))?))?;
            text.push_str(&method(&source, file, signature)?);
            extracted.push(format!("{file}: {signature}"));
            if !hashed.contains(&path) {
                hashed.push(path);
            }
        }
        let path = generated.join(format!("{name}.java"));
        fs::write(&path, text)?;
        sources.push(path.to_string_lossy().into_owned());
    }
    hashed.sort();
    let mut hashes = BTreeMap::new();
    for source in &hashed {
        hashes.insert(
            source.clone(),
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(source))?))
            ),
        );
    }
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources))?;
    let cache = cache_assets::directory(root);
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("ActorsOracle")
        .arg(&cache)
        .arg(SEED))?;
    let fixture = normalized(&output.stdout);
    let mut counts = BTreeMap::<String, usize>::new();
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        *counts
            .entry(value["case"].as_str().unwrap_or("?").to_owned())
            .or_default() += 1;
    }
    let mut inputs = BTreeMap::new();
    for name in INPUTS {
        let mut hasher = Sha256::new();
        std::io::copy(&mut fs::File::open(cache.join(name))?, &mut hasher)?;
        inputs.insert(*name, format!("{:x}", hasher.finalize()));
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "extractedMethods": extracted,
        "inputs": inputs,
        "seed": SEED,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "Unchanged BasType.decode over every BAS record of the pinned cache and seeded opcode streams; unchanged NpcList.processEntity (movement with acceleration, forced movement, facing with accelerated yaw, animation clocks) and PathingEntity.applyBodyLean for players and NPCs over seeded commands, with the cache's accelerated BAS records (sequence ids replaced) and seeded ones; ScriptRunner.clearTileEntityCounts/addPlayersToScene/addNpcsToScene and Player.animate copied verbatim from the reference into adapter classes at generation time; unchanged ProjAnim.setTarget/update/render over seeded flights; unchanged ColorUtils.rgbToHsl over all 2^24 colours. Models, the scene graph (flat terrain, insertions recorded), sequences' frame data, sounds and the player/NPC tables are adapters. Nothing is rendered."
    });
    let directory = root.join("fixtures/actors");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "actors oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("actors oracle provenance is stale; review source or input changes".into());
        }
        println!("{total} JVM actor observations reproduced with matching sources and inputs.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM actor observations: {counts:?}");
    }
    Ok(())
}
