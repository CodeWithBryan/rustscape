//! `cargo xtask camera-oracle [--check]`: the unchanged RT4 `Camera` and
//! `MathUtils` run over seeded inputs, with the pieces of `ScriptRunner`,
//! `SceneGraph`, `GlRenderer`, `GameShell` and `Protocol` that cannot be
//! compiled alone copied into adapters; the observations become
//! `fixtures/camera`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &["Camera", "MathUtils"];
/// The classes the adapters copy from; their hashes pin the copied text.
const COPIED: &[&str] = &[
    "ScriptRunner",
    "SceneGraph",
    "GlRenderer",
    "GameShell",
    "Protocol",
];
const OTHER: &[&str] = &[
    "oracle/camera/adapters/rt4/Buffer.java",
    "oracle/camera/adapters/rt4/GameShell.java",
    "oracle/camera/adapters/rt4/GlRaster.java",
    "oracle/camera/adapters/rt4/GlRenderer.java",
    "oracle/camera/adapters/rt4/InterfaceList.java",
    "oracle/camera/adapters/rt4/Keyboard.java",
    "oracle/camera/adapters/rt4/Player.java",
    "oracle/camera/adapters/rt4/PlayerList.java",
    "oracle/camera/adapters/rt4/Preferences.java",
    "oracle/camera/adapters/rt4/Protocol.java",
    "oracle/camera/adapters/rt4/SceneGraph.java",
    "oracle/camera/adapters/rt4/ScriptRunner.java",
    "oracle/camera/CameraOracle.java",
];
/// `CameraOracle` arguments: seed, then follow, viewport, projection and
/// occlusion case counts.
const ARGUMENTS: [&str; 5] = ["20261116", "120", "400", "120", "60"];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let client = |name: &&str| format!("reference/client/client/src/main/java/rt4/{name}.java");
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(client)
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for source in sources.iter().cloned().chain(COPIED.iter().map(client)) {
        let hash = format!(
            "{:x}",
            Sha256::digest(normalized(&fs::read(root.join(&source))?))
        );
        hashes.insert(source, hash);
    }
    let classes = root.join("runs/camera-oracle/classes");
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
        .arg("CameraOracle")
        .args(ARGUMENTS))?;
    let fixture = normalized(&output.stdout);
    let mut counts = BTreeMap::<String, usize>::new();
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        *counts
            .entry(value["case"].as_str().unwrap_or("?").to_owned())
            .or_default() += 1;
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "arguments": ARGUMENTS,
        "scope": "Unchanged Camera (updatePlayerCamera, clampCameraAngle, calculateRenderPosition) and MathUtils. Copied verbatim into adapters, because their classes cannot be compiled alone: ScriptRunner.calculateViewportBounds, projectToScreen and the follow branch of renderGameScene; SceneGraph.getTileHeight and the opcode 129 part of readTerrain; the edge arithmetic of GlRenderer.setupPerspectiveView; GameShell.calcRenderDelta and mainInputLoop; the camera lines of Protocol's drift block with Math.random() as an argument. Scene arrays are synthetic. No OpenGL matrix, input device or timing is observed."
    });
    let directory = root.join("fixtures/camera");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "camera oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("camera oracle provenance is stale; review source changes".into());
        }
        println!("{total} JVM camera observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM camera observations: {counts:?}");
    }
    Ok(())
}
