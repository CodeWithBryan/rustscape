//! `cargo xtask picking-oracle [--check]`: RT4's mouse picking and scene
//! drawing order, observed on a JVM, become `fixtures/picking`.
//!
//! `GlModel` and `SceneGraph` cannot be compiled without the whole GL client,
//! so the methods under test are copied out of the unchanged sources as text
//! (whole methods, or the stated part of one) into probe classes whose other
//! members are plain fields. Their small data classes are compiled unchanged.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const JAVA: &str = "reference/client/client/src/main/java/rt4";
/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "Tile",
    "Scenery",
    "Wall",
    "WallDecor",
    "GroundDecor",
    "ObjStackEntity",
    "GlBoundingBox",
    "MathUtils",
    "Node",
    "LinkedList",
    "GlobalConfig",
];
/// Sources methods are copied from.
const EXTRACTED: &[&str] = &["GlModel", "SceneGraph"];
const OTHER: &[&str] = &[
    "oracle/picking/adapters/rt4/Adapters.java",
    "oracle/picking/PickingOracle.java",
];
/// Seed, then the number of hover, tile, frustum and scene cases.
const ARGUMENTS: [&str; 5] = ["20261116", "6000", "3000", "120", "700"];

/// A method of `source`, from its signature to its closing brace.
pub(crate) fn method<'a>(source: &'a str, signature: &str) -> Result<&'a str> {
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

/// The statements of a method.
pub(crate) fn body(method: &str) -> &str {
    let open = method.find('{').unwrap_or(0);
    &method[open + 1..method.len() - 1]
}

/// The part of `text` from `from` up to `to`.
pub(crate) fn part<'a>(text: &'a str, from: &str, to: &str) -> Result<&'a str> {
    let start = text
        .find(from)
        .ok_or_else(|| format!("missing start {from}"))?;
    let end = text[start..]
        .find(to)
        .ok_or_else(|| format!("missing end {to}"))?;
    Ok(&text[start..start + end])
}

/// A line of `text` containing `needle`.
pub(crate) fn line<'a>(text: &'a str, needle: &str) -> Result<&'a str> {
    text.lines()
        .find(|line| line.contains(needle))
        .ok_or_else(|| format!("missing line {needle}").into())
}

pub(crate) const HEADER: &str = "package rt4;\n\nimport org.openrs2.deob.annotation.*;\n\n";

fn probe_model(model: &str) -> Result<String> {
    let render = method(model, "public final void render(@OriginalArg(0) int yaw")?;
    let hover = &render[..render
        .find("GL2 gl = GlRenderer.gl;")
        .ok_or("render: no GL part")?];
    // Drop the declaration's own annotation, left dangling by the cut.
    let hover = &hover[..hover.rfind('\n').ok_or("render: no line break")?];
    Ok(format!(
        "{HEADER}// GlModel.render up to its GL calls, calculateBounds and pointWithinTriangle,\n// copied unchanged.\npublic final class ProbeModel {{\n\tpublic static int pickScreenX = 0;\n\tpublic static int[] pickVertexScreenY = new int[1];\n\tpublic static int[] pickVertexScreenX = new int[1];\n\tpublic final GlBoundingBox bounds = new GlBoundingBox();\n\tpublic int vertexCount = 0;\n\tpublic int uniqueVertexCount = 0;\n\tpublic int triangleCount = 0;\n\tpublic int[] vertexX;\n\tpublic int[] vertexY;\n\tpublic int[] vertexZ;\n\tpublic int[] vertexOffsets;\n\tpublic short[] vertexLookup;\n\tpublic short[] triangleVertexA;\n\tpublic short[] triangleVertexB;\n\tpublic short[] triangleVertexC;\n\tpublic boolean pickable = false;\n\n\t{hover}\n\t}}\n\n\t{}\n\n\t{}\n}}\n",
        method(model, "private void calculateBounds()")?,
        method(model, "private boolean pointWithinTriangle(")?,
    ))
}

fn probe_ground(scene: &str) -> Result<String> {
    Ok(format!(
        "{HEADER}// SceneGraph.drawPlainTile and isRectVisible, copied unchanged.\npublic final class ProbeGround {{\n\tpublic static int cameraX;\n\tpublic static int cameraY;\n\tpublic static int cameraZ;\n\tpublic static int[][][] tileHeights;\n\n\t{}\n\n\t{}\n}}\n",
        method(scene, "public static void drawPlainTile(")?,
        method(scene, "public static boolean isRectVisible(")?,
    ))
}

fn probe_key(scene: &str) -> Result<String> {
    let add = method(
        scene,
        "public static void addLoc(@OriginalArg(0) int currentPlane",
    )?;
    let statements = [
        line(add, "long bitset = x | 0x40000000")?.to_owned(),
        method(add, "if (loc.interactable == 0 || highmem)")?.to_owned(),
        method(add, "if (loc.supportitems == 1)")?.to_owned(),
        method(add, "if (loc.hasanimation)")?.to_owned(),
        line(add, "bitset |= (long) locIndex << 32;")?.to_owned(),
    ]
    .join("\n\t\t");
    Ok(format!(
        "{HEADER}// The statements of SceneGraph.addLoc that build a location's scene key.\npublic final class ProbeKey {{\n\tpublic static long key(int x, int y, int locType, int orientation, int locIndex, int interactable, int supportitems, boolean hasanimation, boolean highmem) {{\n\t\tLocInfo loc = new LocInfo();\n\t\tloc.interactable = interactable;\n\t\tloc.supportitems = supportitems;\n\t\tloc.hasanimation = hasanimation;\n\t\t{statements}\n\t\treturn bitset;\n\t}}\n}}\n"
    ))
}

fn probe_scene(scene: &str) -> Result<String> {
    let tables = scene
        .lines()
        .filter(|line| line.contains("public static final int[] WALL_"))
        .collect::<Vec<_>>()
        .join("\n");
    let camera = method(scene, "public static void setPlainTile(")?;
    let prefix = part(body(camera), "if (camX < 0)", "@Pc(99) short viewDistance;")?;
    let frustum = part(
        body(camera),
        "for (vx = 0; vx < visibility + visibility + 2; vx++)",
        "roofGroupMaxHeight = roofMaxHeight;",
    )?;
    let sweep = method(scene, "public static void setShapedTile(")?;
    let signature = &sweep[..sweep.find('{').ok_or("setShapedTile: no body")?];
    let marking = part(body(sweep), "drawCycle++;", "@Pc(240) boolean isUnderwater")?;
    let sweeps = &body(sweep)[body(sweep)
        .find("@Pc(434) int nearY;")
        .ok_or("setShapedTile: no sweeps")?..];
    let methods = [
        "public static void renderTile(",
        "public static boolean addSceneryEntity(",
        "public static boolean add(@OriginalArg(0) int level, @OriginalArg(1) int xFine",
        "public static void setWall(",
        "public static void setWallDecor(",
        "public static void setGroundDecor(",
        "public static void setObjStack(",
        "public static void removeScenery(@OriginalArg(0) Scenery scenery)",
        "public static void bridgeTile(",
        "public static void setBaseLevel(",
        "public static boolean projectToScreen(",
    ]
    .iter()
    .map(|signature| method(scene, signature))
    .collect::<Result<Vec<_>>>()?
    .join("\n\n\t");
    Ok(format!(
        "{HEADER}// SceneGraph's tile walk and the calls that place things on tiles, copied\n// unchanged. Occlusion always answers \"not hidden\" and ground drawing only\n// records the tile.\npublic final class ProbeScene {{\n{tables}\n\tpublic static java.util.ArrayList<String> out = new java.util.ArrayList<String>();\n\tpublic static Tile[][][] tiles;\n\tpublic static int width = 104;\n\tpublic static int length = 104;\n\tpublic static int levels = 4;\n\tpublic static int startLevel = 0;\n\tpublic static int[][][] tileHeights;\n\tpublic static int[][][] surfaceTileHeights;\n\tpublic static int[][][] underwaterTileHeights;\n\tpublic static int[][] tileWaterData;\n\tpublic static Scenery[] scenery;\n\tpublic static int sceneryLen;\n\tpublic static final Scenery[] sceneryBuffer = new Scenery[100];\n\tpublic static LinkedList tileQueue = new LinkedList();\n\tpublic static int drawCycle;\n\tpublic static int visibleTileCount;\n\tpublic static int cameraX;\n\tpublic static int cameraY;\n\tpublic static int cameraZ;\n\tpublic static int cameraTileX;\n\tpublic static int cameraTileY;\n\tpublic static int sinPitch;\n\tpublic static int cosPitch;\n\tpublic static int sinYaw;\n\tpublic static int cosYaw;\n\tpublic static int visibility;\n\tpublic static int lastUnderwaterColor;\n\tpublic static int lastUnderwaterFogRange;\n\tpublic static boolean[][] visibleTiles;\n\tpublic static boolean[][] visibilityPoints;\n\n\tpublic static boolean isTileOccluded(int level, int x, int y) {{\n\t\treturn false;\n\t}}\n\n\tpublic static boolean isWallVisible(int level, int x, int y, int flags) {{\n\t\treturn false;\n\t}}\n\n\tpublic static boolean isTileVisible(int level, int x, int y, int height) {{\n\t\treturn false;\n\t}}\n\n\tpublic static boolean isAreaVisible(int level, int xMin, int xMax, int yMin, int yMax, int height) {{\n\t\treturn false;\n\t}}\n\n\tpublic static void drawPlainTile(PlainTile tile, int level, int sinPitch, int cosPitch, int cosYaw, int sinYaw, int tileX, int tileY, boolean occluded) {{\n\t\tout.add(\"g\" + level + \",\" + tileX + \",\" + tileY);\n\t}}\n\n\tpublic static void drawShapedTile(ShapedTile tile, int sinPitch, int cosPitch, int cosYaw, int sinYaw, int tileX, int tileY, int level, boolean occluded) {{\n\t\tout.add(\"g\" + level + \",\" + tileX + \",\" + tileY);\n\t}}\n\n\t// The start of setPlainTile.\n\tpublic static void camera(int camX, int camZ, int camY, int pitch, int yaw) {{\n\t\t{prefix}\n\t}}\n\n\t// setPlainTile's visibility map.\n\tpublic static void frustum(short viewDistance) {{\n\t\tint vx;\n\t\tint vy;\n\t\t{frustum}\n\t}}\n\n\t// setShapedTile without its GL block between marking and sweeping.\n\t{signature}{{\n\t\t{marking}boolean isUnderwater = false;\n\t\t{sweeps}\n\t}}\n\n\t{methods}\n}}\n"
    ))
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let read = |name: &str| -> Result<String> {
        Ok(String::from_utf8(normalized(&fs::read(
            root.join(format!("{JAVA}/{name}.java")),
        )?))?)
    };
    let (model, scene) = (read("GlModel")?, read("SceneGraph")?);
    let build = root.join("runs/picking-oracle");
    let (generated, classes) = (build.join("generated/rt4"), build.join("classes"));
    for directory in [&generated, &classes] {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
        fs::create_dir_all(directory)?;
    }
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{JAVA}/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    for (name, text) in [
        ("ProbeModel", probe_model(&model)?),
        ("ProbeGround", probe_ground(&scene)?),
        ("ProbeKey", probe_key(&scene)?),
        ("ProbeScene", probe_scene(&scene)?),
    ] {
        fs::write(generated.join(format!("{name}.java")), text)?;
        sources.push(format!("runs/picking-oracle/generated/rt4/{name}.java"));
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
        .arg("rt4.PickingOracle")
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
        "scope": "Text copied unchanged from GlModel (render up to its GL calls, calculateBounds, pointWithinTriangle) and SceneGraph (drawPlainTile, isRectVisible, the scene-key statements of addLoc, renderTile, setShapedTile without its GL block, the camera and visibility-map parts of setPlainTile, projectToScreen, addSceneryEntity, add, setWall, setWallDecor, setGroundDecor, setObjStack, removeScenery, bridgeTile, setBaseLevel), compiled with the unchanged Tile, Scenery, Wall, WallDecor, GroundDecor, ObjStackEntity, GlBoundingBox, MathUtils, Node, LinkedList and GlobalConfig. Models are recording stand-ins; GL, lighting and raster calls are empty; every occlusion query answers 'not hidden'; ground kinds are set directly on tiles as setTile does. 'lost' lines are scenes on which the original threw. drawShapedTile, occluders, roof removal and SceneGraph.addLoc's shape rules are not observed."
    });
    let directory = root.join("fixtures/picking");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.txt"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "picking oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("picking oracle provenance is stale; review source changes".into());
        }
        println!("{total} JVM picking observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM picking observations: {counts:?}");
    }
    Ok(())
}
