//! `cargo xtask title-oracle [--check]`: the title screen's 3D scene as RT4
//! sets it up and runs it, observed on a JVM, becomes `fixtures/title`.
//!
//! `LoginManager`, `client` and `PathingEntity` cannot be compiled without
//! the whole client, so the methods under test are copied out of the
//! unchanged sources as text into classes of the same names whose other
//! members are plain fields. `Camera`, `MathUtils` and `CollisionMap` are
//! compiled unchanged.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const JAVA: &str = "reference/client/client/src/main/java/rt4";
/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &["Camera", "MathUtils", "CollisionMap"];
/// Sources text is copied from.
const EXTRACTED: &[&str] = &["LoginManager", "client", "PathingEntity"];
const OTHER: &[&str] = &[
    "oracle/title/scene/adapters/rt4/Adapters.java",
    "oracle/title/scene/TitleSceneOracle.java",
];
/// Seed, then the number of region, spawn, wander and camera cases.
const ARGUMENTS: [&str; 5] = ["20261116", "300", "300", "400", "60"];

/// A method of `source`, from its signature to its closing brace.
fn method<'a>(source: &'a str, signature: &str) -> Result<&'a str> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("missing method {signature}"))?;
    let mut depth = 0usize;
    for (offset, byte) in source.as_bytes()[start..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&source[start..=start + offset]);
                }
            }
            _ => {}
        }
    }
    Err(format!("unterminated method {signature}").into())
}

/// The part of `text` from `from` up to `to`.
fn part<'a>(text: &'a str, from: &str, to: &str) -> Result<&'a str> {
    let start = text
        .find(from)
        .ok_or_else(|| format!("missing start {from}"))?;
    let end = text[start..]
        .find(to)
        .ok_or_else(|| format!("missing end {to}"))?;
    Ok(&text[start..start + end])
}

/// A line of `text` containing `needle`.
fn line<'a>(text: &'a str, needle: &str) -> Result<&'a str> {
    text.lines()
        .find(|line| line.contains(needle))
        .ok_or_else(|| format!("missing line {needle}").into())
}

const HEADER: &str = "package rt4;\n\nimport org.openrs2.deob.annotation.*;\n\n";

fn login_manager(source: &str) -> Result<String> {
    let constants = [
        "JagString UNDERWATER_LOC_PREFIX = ",
        "JagString MAP_PREFIX = ",
        "JagString LOC_PREFIX = ",
        "JagString NPC_PREFIX = ",
        "JagString UNDERWATER_MAP_PREFIX = ",
        "JagString UNDERSCORE = ",
        "int[] decodedNpcRegions = ",
        "int decodedNpcRegionCount = ",
    ]
    .iter()
    .map(|needle| line(source, needle))
    .collect::<Result<Vec<_>>>()?
    .join("\n");
    Ok(format!(
        "{HEADER}// LoginManager.setupLoadingScreenRegion, loadRegion and decodeNpcFiles with\n// the constants they name, copied unchanged. The other fields are declared\n// here as the original declares them.\npublic final class LoginManager {{\n{constants}\n\tpublic static byte[][] locationMapFilesBuffer;\n\tpublic static int[] underWaterLocationsMapFileIds;\n\tpublic static byte[][] npcSpawnsFilesBuffer;\n\tpublic static int[] mapFileIds;\n\tpublic static int[][] regionsXteaKeys;\n\tpublic static byte[][] underWaterMapFilesBuffer;\n\tpublic static int[] regionBitPacked;\n\tpublic static byte[][] mapFilesBuffer;\n\tpublic static int[] npcSpawnsFileIds;\n\tpublic static int[] underWaterMapFileIds;\n\tpublic static int[] locationsMapFileIds;\n\tpublic static byte[][] underWaterLocationsMapFilesBuffer;\n\tpublic static Map map;\n\tpublic static MapElementList mapElementList;\n\tpublic static int mapFlagX;\n\tpublic static int mapFlagY;\n\tpublic static int step;\n\n\t{}\n\n\t{}\n\n\t{}\n}}\n",
        method(source, "public static void setupLoadingScreenRegion()")?,
        method(source, "public static void loadRegion(")?,
        method(source, "public static void decodeNpcFiles()")?,
    ))
}

fn probe_client(source: &str) -> Result<String> {
    let update = method(source, "private void mainUpdate()")?;
    // From the NPC pass to the end of the camera part; the interface walk
    // before it and the hook queues after it are left out.
    let scene = part(update, "if (GlRenderer.enabled) {", "while (true) {")?;
    Ok(format!(
        "{HEADER}// The title scene's part of client.mainUpdate (the NPC pass and the camera),\n// copied unchanged.\npublic final class ProbeClient {{\n\tpublic static void mainUpdate() {{\n\t\t{scene}\n\t}}\n}}\n"
    ))
}

fn pathing_entity(source: &str) -> Result<String> {
    let fields = [
        "int[] ANGLES = ",
        "int[] movementQueueX = ",
        "int[] movementQueueY = ",
        "byte[] movementQueueSpeed = ",
    ]
    .iter()
    .map(|needle| line(source, needle))
    .collect::<Result<Vec<_>>>()?
    .join("\n");
    Ok(format!(
        "{HEADER}// PathingEntity.teleport, setSize and getSize with the queue arrays and the\n// angle table, copied unchanged; the other fields are plain.\npublic class PathingEntity {{\n{fields}\n\tpublic int xFine;\n\tpublic int yFine;\n\tpublic int movementQueueSize;\n\tpublic int movementCatchupTicks;\n\tpublic int seqMovementSteps;\n\tpublic int seqId = -1;\n\tpublic int lastSeenLoop;\n\tpublic int targetAngle;\n\tpublic int currentAngle;\n\tpublic int turnSpeed;\n\tpublic int basTypeId;\n\tprivate int size = 1;\n\n\t{}\n\n\t{}\n\n\t{}\n}}\n",
        method(source, "public final void teleport(")?,
        method(source, "public final void setSize(")?,
        method(source, "public int getSize()")?,
    ))
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let read = |name: &str| -> Result<String> {
        Ok(String::from_utf8(normalized(&fs::read(
            root.join(format!("{JAVA}/{name}.java")),
        )?))?)
    };
    let build = root.join("runs/title-oracle");
    let (generated, classes) = (build.join("generated/rt4"), build.join("classes"));
    for directory in [&generated, &classes] {
        fs::create_dir_all(directory)?;
    }
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{JAVA}/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    for (name, text) in [
        ("LoginManager", login_manager(&read("LoginManager")?)?),
        ("ProbeClient", probe_client(&read("client")?)?),
        ("PathingEntity", pathing_entity(&read("PathingEntity")?)?),
    ] {
        fs::write(generated.join(format!("{name}.java")), text)?;
        sources.push(format!("runs/title-oracle/generated/rt4/{name}.java"));
    }
    let mut hashes = BTreeMap::new();
    for source in sources
        .iter()
        .filter(|source| !source.starts_with("runs/"))
        .cloned()
        .chain(EXTRACTED.iter().map(|name| format!("{JAVA}/{name}.java")))
    {
        let digest = Sha256::digest(normalized(&fs::read(root.join(&source))?));
        hashes.insert(source, format!("{digest:x}"));
    }
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources))?;
    let output = build.join("oracle.txt");
    run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("rt4.TitleSceneOracle")
        .args(ARGUMENTS)
        .arg(&output))?;
    let fixture = normalized(&fs::read(&output)?);
    let mut counts = BTreeMap::<String, usize>::new();
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let kind = line.split(|&b| b == b' ').next().unwrap_or_default();
        *counts
            .entry(String::from_utf8_lossy(kind).into_owned())
            .or_default() += 1;
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "arguments": ARGUMENTS,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "The unchanged Camera (updateLoginScreenCamera, updateLockedCamera, setLockedPosition, setLockedLookAt, resetCameraEffects), MathUtils and CollisionMap (isPathClear, flagScenery, unflagScenery), with text copied unchanged from LoginManager (setupLoadingScreenRegion, loadRegion, decodeNpcFiles), client (mainUpdate from its NPC pass to the end of its camera part) and PathingEntity (teleport, setSize, getSize). Math.random() is a scripted sequence; the map archive is a name table holding the 30 NPC files of the pinned cache and terrain and scenery for every region; NPC types are a table; the ground is a fixed slope; NpcList.processMovement takes a queued step at once and facing and animation are empty; the camera path tables and the spline start are written as CS2 5405, 5406 and 5502 write them, by hand. Not observed: LoginManager.rebuildMap, the scene build, NPC movement, rendering, the interface scripts that choose the paths."
    });
    let directory = root.join("fixtures/title");
    let (fixture_path, provenance_path) = (
        directory.join("scene.txt"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "title oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("title oracle provenance is stale; review source changes".into());
        }
        println!("{total} JVM title-scene observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM title-scene observations: {counts:?}");
    }
    Ok(())
}
