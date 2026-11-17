//! `cargo xtask materials-oracle [--check]`: the unchanged RT4 noise
//! generators behind the HD material renderers; their tables become
//! `fixtures/materials`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle, besides every
/// `TextureOp*.java` (which `Texture` constructs by opcode).
const CLIENT: &[&str] = &[
    "PerlinNoiseGenerator",
    "RidgedNoiseGenerator",
    "RidgedNoiseTexture",
    "FractalNoiseGenerator",
    "FractalNoiseTexture",
    "RandomUtils",
    "IntUtils",
    "MonochromeImageCache",
    "MonochromeImageCacheEntry",
    "ColorImageCache",
    "ColorImageCacheEntry",
    "Node",
    "LinkedList",
    "ArrayUtils",
    "Texture",
    "TextureProvider",
    "LruHashTable",
    "HashTable",
    "SecondaryLinkedList",
    "SecondaryNode",
    "ByteArrayNode",
    "ByteArrayNodeSecondary",
];
const OTHER: &[&str] = &[
    "oracle/materials/adapters/rt4/Buffer.java",
    "oracle/materials/adapters/rt4/Js5.java",
    "oracle/materials/adapters/rt4/SoftwareSprite.java",
    "oracle/materials/adapters/rt4/SpriteLoader.java",
    "oracle/materials/MaterialsOracle.java",
];
/// Read for their formulas but not compiled: they need an OpenGL binding.
/// Their hashes are recorded so a change to them is noticed.
const TRANSCRIBED: &[&str] = &[
    "MaterialManager",
    "LiquidMaterialRenderer",
    "WaterMaterialRenderer",
    "WaterfallMaterialRenderer",
    "SpecularMaterialRenderer",
    "UnlitMaterialRenderer",
    "UnderwaterMaterialRenderer",
    "GlTexture",
];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let client = "reference/client/client/src/main/java/rt4";
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{client}/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    for entry in fs::read_dir(root.join(client))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name.starts_with("TextureOp") && name.ends_with(".java") {
            sources.push(format!("{client}/{name}"));
        }
    }
    sources.sort();
    let hash = |source: &str| -> Result<String> {
        Ok(format!(
            "{:x}",
            Sha256::digest(normalized(&fs::read(root.join(source))?))
        ))
    };
    let mut hashes = BTreeMap::new();
    for source in &sources {
        hashes.insert(source.clone(), hash(source)?);
    }
    let mut transcribed = BTreeMap::new();
    for name in TRANSCRIBED {
        let source = format!("{client}/{name}.java");
        transcribed.insert(source.clone(), hash(&source)?);
    }
    let classes = root.join("runs/materials-oracle/classes");
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
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("MaterialsOracle"))?;
    let fixture = normalized(&output.stdout);
    let mut cases = vec![];
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        cases.push(value["case"].as_str().unwrap_or("?").to_owned());
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "transcribed": transcribed,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": cases,
        "scope": "Unchanged RidgedNoiseTexture and FractalNoiseTexture (with PerlinNoiseGenerator and its two subclasses) give the 64x64x64 luminance-alpha volumes of MaterialManager.initNoiseTextures: every byte is hashed, per slice and whole. Unchanged TextureOpPerlinNoise and Texture give the 256x64 table of LiquidMaterialRenderer.generateNoiseTable, whose body the oracle repeats because that class needs an OpenGL binding to load; every value is recorded. Buffer, Js5, SoftwareSprite and SpriteLoader are compile-only adapters. No GL context exists: texture upload, filtering, texture-coordinate generation, combiners, vertex programs and the cube maps of SpecularMaterialRenderer are not observed. The classes under `transcribed` are hashed only."
    });
    let directory = root.join("fixtures/materials");
    let (fixture_path, provenance_path) = (
        directory.join("noise.jsonl"),
        directory.join("provenance.json"),
    );
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "materials oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err(
                "materials oracle provenance is stale; review source or input changes".into(),
            );
        }
        println!(
            "{} JVM material tables reproduced with matching sources.",
            cases.len()
        );
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated JVM material tables: {cases:?}");
    }
    Ok(())
}
