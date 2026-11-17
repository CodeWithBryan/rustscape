//! The second part of `cargo xtask materials-oracle [--check]`: the unchanged
//! `UnderwaterMaterialRenderer`, `WaterMaterialRenderer`, `MaterialManager`
//! and `GlTile` run against a GL2 that records every call. The traces become
//! `fixtures/underwater/material.jsonl`; `fixtures/materials` is not touched.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "UnderwaterMaterialRenderer",
    "WaterMaterialRenderer",
    "MaterialManager",
    "MaterialRenderer",
    "GlTile",
    "ColorUtils",
    "GlobalConfig",
    "Buffer",
    "HashTable",
    "IntNode",
    "IntUtils",
    "Node",
    "TextureProvider",
    "PerlinNoiseGenerator",
    "RidgedNoiseGenerator",
    "RidgedNoiseTexture",
    "FractalNoiseGenerator",
    "FractalNoiseTexture",
    "RandomUtils",
    "MonochromeImageCache",
    "MonochromeImageCacheEntry",
    "LinkedList",
];
const OTHER: &[&str] = &[
    "oracle/adapters/rt4/JagString.java",
    "oracle/underwater/material/com/jogamp/opengl/GL2.java",
    "oracle/underwater/material/com/jogamp/opengl/GL2GL3.java",
    "oracle/underwater/material/rt4/Adapters.java",
    "oracle/underwater/material/rt4/UnderwaterMaterialOracle.java",
];
/// Read for the frame's order but not run: they need the whole client.
const TRANSCRIBED: &[&str] = &["SceneGraph", "LoginManager", "FogManager", "GlRenderer"];

pub fn generate(root: &Path, check: bool, upstream: &Value) -> Result<()> {
    let client = "reference/client/client/src/main/java/rt4";
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{client}/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
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
    let classes = root.join("runs/materials-oracle/underwater-classes");
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
        .arg("rt4.UnderwaterMaterialOracle"))?;
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
        "scope": "Unchanged UnderwaterMaterialRenderer, WaterMaterialRenderer, MaterialManager and GlTile (with the noise generators MaterialManager.init runs) against a com.jogamp.opengl.GL2 interface implemented by a recording proxy: every GL call of each case is listed in order with its arguments, display lists replayed in place. Observed: the gradient texture's bytes and parameters, both display lists, the texture-coordinate planes and environment colour setArgument sets for the floor (argument -1) and for scenery (minus the surface height) at a given camera, clock, atmosphere, water colour and fog range, that MaterialManager.setMaterial does nothing for any other material while renderingUnderwater, and the state and draw calls of applyFogFade over a surface group of a water texture. GlRenderer, FogManager, SceneGraph, Tile, Preferences, Rasteriser, GlVertexBufferObject and the four other renderers are adapters; GlRenderer's state setters are noted in the trace by name. No OpenGL exists: what the combiners, the texture lookups and the fog compute from this state is not observed, and neither is the order SceneGraph.setPlainTile and setShapedTile call these in. The classes under `transcribed` are hashed only."
    });
    let directory = root.join("fixtures/underwater");
    let (fixture_path, provenance_path) = (
        directory.join("material.jsonl"),
        directory.join("material-provenance.json"),
    );
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "underwater material oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err(
                "underwater material oracle provenance is stale; review source or input changes"
                    .into(),
            );
        }
        println!(
            "{} JVM underwater material traces reproduced with matching sources.",
            cases.len()
        );
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated JVM underwater material traces: {cases:?}");
    }
    Ok(())
}
