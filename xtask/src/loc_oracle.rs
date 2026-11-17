//! `cargo xtask loc-oracle [--check]`: the unchanged RT4 location classes run
//! over the pinned cache; their observations become `fixtures/locs`.
use crate::{Result, cache_assets, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "LocType",
    "LocTypeList",
    "LocEntity",
    "GlModel",
    "GlBoundingBox",
    "GlBuffer",
    "GlModelRecoloring",
    "RawModel",
    "Model",
    "Entity",
    "MathUtils",
    "VertexNormal",
    "TriangleNormal",
    "ColorUtils",
    "AnimFrame",
    "AnimBase",
    "AnimFrameset",
    "TextureProvider",
    "IntNode",
    "StringNode",
    "SoftLruHashTable",
    "SecondaryLinkedList",
    "HardReferenceNode",
    "ReferenceNode",
    "ReferenceNodeFactory",
    "SoftReferenceNode",
    "SoftReferenceNodeFactory",
    "Node",
    "SecondaryNode",
    "LinkedList",
    "HashTable",
    "Buffer",
    "IntUtils",
    "ByteArrayNode",
    "ArrayUtils",
    "GlobalConfig",
    "Packet",
    "IsaacRandom",
    "RandomUtils",
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
const OTHER: &[&str] = &[
    "reference/client/signlink/src/main/java/rt4/MonotonicClock.java",
    "reference/client/signlink/src/main/java/rt4/FileOnDisk.java",
    "oracle/loc/adapters/com/jogamp/opengl/GL2.java",
    "oracle/loc/adapters/plugin/api/API.java",
    "oracle/loc/adapters/rt4/FogManager.java",
    "oracle/loc/adapters/rt4/GlRenderer.java",
    "oracle/loc/adapters/rt4/GlVertexBufferObject.java",
    "oracle/loc/adapters/rt4/JagString.java",
    "oracle/loc/adapters/rt4/LocalizedText.java",
    "oracle/loc/adapters/rt4/MaterialManager.java",
    "oracle/loc/adapters/rt4/MiniMenu.java",
    "oracle/loc/adapters/rt4/ParticleSystem.java",
    "oracle/loc/adapters/rt4/Preferences.java",
    "oracle/loc/adapters/rt4/Rasteriser.java",
    "oracle/loc/adapters/rt4/SceneGraph.java",
    "oracle/loc/adapters/rt4/SeqType.java",
    "oracle/loc/adapters/rt4/SoftwareIndexedSprite.java",
    "oracle/loc/adapters/rt4/SoftwareModel.java",
    "oracle/loc/adapters/rt4/VarpDomain.java",
    "oracle/loc/adapters/rt4/client.java",
    "oracle/loc/LocOracle.java",
];
/// Cache files the observations depend on.
const INPUTS: &[&str] = &[
    "main_file_cache.dat2",
    "main_file_cache.idx7",
    "main_file_cache.idx16",
    "main_file_cache.idx26",
    "main_file_cache.idx255",
];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    cache_assets::verify(root)?;
    let upstream = crate::verify_reference(root)?;
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("reference/client/client/src/main/java/rt4/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for source in &sources {
        hashes.insert(
            source.clone(),
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(source))?))
            ),
        );
    }
    let build = root.join("runs/loc-oracle");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes)?;
    }
    fs::create_dir_all(&classes)?;
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
        .arg("LocOracle")
        .arg(&cache))?;
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
        "inputs": inputs,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "Unchanged LocType, LocTypeList, GlModel, RawModel and Model over the pinned cache with GlRenderer.enabled set: the placement fields of every location type; LocType.getGlModel for rotations 0..7 of types covering shapes 0..22 (reached through reflection because it is private, and through getAnimatedEntity without a sequence); getStaticEntity and getAnimatedEntity of types with a hill skew and GlModel.alignToTerrain types 1..5 over seeded synthetic heights; LocType.getMultiLoc over variable values; and, with Preferences.highDetailLighting off, the colours GlModel.packInterleavedVertexData (private, reached through reflection) packs for the same getGlModel and getAnimatedEntity models under three values of FogManager.light0Position, each model built from emptied LocTypeList caches and Rasteriser.palette mapping every index to itself. The cache reader set-up, the texture flag table, VarpDomain, JagString and the GL, software-renderer and plugin classes under oracle/loc/adapters are adapters. No GL context exists, nothing is drawn and no shadow is projected. SceneGraph.addLoc and Loc are not run."
    });
    let directory = root.join("fixtures/locs");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "location oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err(
                "location oracle provenance is stale; review source or input changes".into(),
            );
        }
        println!("{total} JVM location observations reproduced with matching sources and inputs.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM location observations: {counts:?}");
    }
    Ok(())
}
