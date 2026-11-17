//! `cargo xtask lighting-oracle [--check]`: the unchanged RT4 `LightingManager`,
//! `LightMesh` and `ShadowManager`, with the light-mesh methods of `SceneGraph`
//! copied verbatim into a class of their own, run over seeded inputs; their
//! observations become `fixtures/lighting`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const CLIENT_DIRECTORY: &str = "reference/client/client/src/main/java/rt4";
/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "LightingManager",
    "LightMesh",
    "ShadowManager",
    "HashTable",
    "Node",
    "IntNode",
    "IntUtils",
];
/// `SceneGraph` members copied verbatim, each found by this text.
const SCENE_GRAPH: &[&str] = &[
    "public static Tile[][][] tiles;",
    "public static final int[][] OVERLAY_SHAPE_VERTICES =",
    "public static final int[][] UNDERLAY_SHAPE_VERTICES =",
    "public static Scenery getScenery(",
    "public static void buildGlLighting(",
    "public static void applyLightToTile(",
    "public static int interpolateHeight(",
];
const ADAPTERS: &[&str] = &[
    "oracle/lighting/adapters/com/jogamp/opengl/GL2.java",
    "oracle/lighting/adapters/rt4/Buffer.java",
    "oracle/lighting/adapters/rt4/FloType.java",
    "oracle/lighting/adapters/rt4/FloTypeList.java",
    "oracle/lighting/adapters/rt4/FogManager.java",
    "oracle/lighting/adapters/rt4/GlRenderer.java",
    "oracle/lighting/adapters/rt4/GlVertexBufferObject.java",
    "oracle/lighting/adapters/rt4/Light.java",
    "oracle/lighting/adapters/rt4/MaterialManager.java",
    "oracle/lighting/adapters/rt4/Preferences.java",
    "oracle/lighting/adapters/rt4/Scenery.java",
    "oracle/lighting/adapters/rt4/Shadow.java",
    "oracle/lighting/adapters/rt4/SoftwareIndexedSprite.java",
    "oracle/lighting/adapters/rt4/Sprites.java",
    "oracle/lighting/adapters/rt4/Tile.java",
    "oracle/lighting/LightingOracle.java",
];

/// One member of a Java class, from its annotation line to its closing brace
/// (or its semicolon, for a field), exactly as the source has it.
fn member(source: &str, marker: &str) -> Result<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(marker));
    let (at, line) = found
        .next()
        .ok_or_else(|| format!("SceneGraph.java has no `{marker}`"))?;
    if found.next().is_some() {
        return Err(format!("SceneGraph.java has `{marker}` more than once").into());
    }
    let start = if at > 0 && lines[at - 1].trim_start().starts_with("@OriginalMember") {
        at - 1
    } else {
        at
    };
    let mut end = at;
    if !line.trim_end().ends_with(';') {
        let mut depth = 0i32;
        loop {
            let line = lines
                .get(end)
                .ok_or_else(|| format!("`{marker}` is not closed"))?;
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            if depth == 0 && line.contains('}') {
                break;
            }
            end += 1;
        }
    }
    Ok(lines[start..=end].join("\n"))
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let scene_graph_path = format!("{CLIENT_DIRECTORY}/SceneGraph.java");
    let scene_graph = String::from_utf8(normalized(&fs::read(root.join(&scene_graph_path))?))?;
    let mut extracted = String::from(
        "package rt4;\n\nimport org.openrs2.deob.annotation.OriginalArg;\nimport org.openrs2.deob.annotation.OriginalMember;\nimport org.openrs2.deob.annotation.Pc;\n\n/** Members of the pinned SceneGraph.java, copied verbatim by `cargo xtask lighting-oracle`. */\npublic class SceneGraph {\n",
    );
    for marker in SCENE_GRAPH {
        extracted.push_str(&member(&scene_graph, marker)?);
        extracted.push_str("\n\n");
    }
    extracted.push_str("}\n");
    let build = root.join("runs/lighting-oracle");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes)?;
    }
    fs::create_dir_all(&classes)?;
    let generated = build.join("generated/rt4");
    fs::create_dir_all(&generated)?;
    fs::write(generated.join("SceneGraph.java"), &extracted)?;

    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{CLIENT_DIRECTORY}/{name}.java"))
        .chain(ADAPTERS.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for source in sources.iter().chain([&scene_graph_path]) {
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
        .args(&sources)
        .arg(generated.join("SceneGraph.java")))?;
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("LightingOracle"))?;
    let fixture = normalized(&output.stdout);
    let mut counts = BTreeMap::<String, usize>::new();
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let case = line.split(|&b| b == b' ').next().unwrap_or_default();
        *counts
            .entry(String::from_utf8_lossy(case).into_owned())
            .or_default() += 1;
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "sceneGraphMembers": SCENE_GRAPH,
        "extractedSha256": format!("{:x}", Sha256::digest(extracted.as_bytes())),
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "Unchanged LightingManager (light grid, updateLightsForTile/Region/WallPiece, pruneInactiveLights, resetActiveLights, the cache reset of updateAllLightAnimations), LightMesh (addVertex, addFan, upload) and ShadowManager (addFloorShadow, isObjectInShadow), with HashTable, Node, IntNode and IntUtils. SceneGraph.buildGlLighting, applyLightToTile, interpolateHeight, getScenery and the two shape vertex tables are copied verbatim into a class of their own; the corner normals passed in repeat buildScene lines 1109-1115 by hand in LightingOracle.java. Light, Tile, Scenery, FloType, FogManager, the sprite classes, Buffer and the GL classes are adapters holding only the members read. Inputs are seeded, not cache data: light footprints, tile floors, diagonal walls, heights, floor-shadow masks. No GL call is executed; light changes are recorded as issued. renderTile's choice of which query to make for which object is not run.",
    });
    let directory = root.join("fixtures/lighting");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.txt"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "lighting oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err(
                "lighting oracle provenance is stale; review source or input changes".into(),
            );
        }
        println!("{total} JVM lighting observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM lighting observations: {counts:?}");
    }
    Ok(())
}
