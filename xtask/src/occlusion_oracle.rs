//! `cargo xtask occlusion-oracle [--check]`: RT4's occluders, observed on a
//! JVM, become `fixtures/occlusion`.
//!
//! `SceneGraph` cannot be compiled without the whole GL client, so the
//! methods under test are copied out of the unchanged source as text (whole
//! methods, or the stated part of one) into a holder class of the same name
//! whose other members are plain fields. `Occluder` and the tile classes are
//! compiled unchanged. This is the walk of `picking-oracle` again, with the
//! occlusion queries it answers "not hidden" replaced by the original's own.
use crate::{
    Result, normalized,
    picking_oracle::{HEADER, body, line, method, part},
    run,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const JAVA: &str = "reference/client/client/src/main/java/rt4";
/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "Occluder",
    "Tile",
    "Scenery",
    "Wall",
    "WallDecor",
    "GroundDecor",
    "ObjStackEntity",
    "MathUtils",
    "Node",
    "LinkedList",
    "GlobalConfig",
];
/// Sources text is copied from.
const EXTRACTED: &[&str] = &["SceneGraph", "LocType"];
const OTHER: &[&str] = &[
    "oracle/occlusion/adapters/rt4/Adapters.java",
    "oracle/occlusion/OcclusionOracle.java",
];
/// Seed, then the number of built scenes (each with views) and of walked
/// scenes; twenty-four staged scenes follow.
const ARGUMENTS: [&str; 3] = ["20261116", "200", "600"];

/// The shape constants of `LocType`, copied as they are declared.
fn loc_type(source: &str) -> String {
    let constants = source
        .lines()
        .filter(|line| line.contains("public static final int "))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{HEADER}// LocType's shape constants, copied unchanged.\npublic final class LocType {{\n{constants}\n}}\n"
    )
}

fn holder(scene: &str) -> Result<String> {
    let tables = scene
        .lines()
        .filter(|line| line.contains("public static final int[] WALL_"))
        .collect::<Vec<_>>()
        .join("\n");
    let camera = method(scene, "public static void setPlainTile(")?;
    let prefix = part(body(camera), "if (camX < 0)", "@Pc(99) short viewDistance;")?;
    let sweep = method(scene, "public static void setShapedTile(")?;
    let signature = &sweep[..sweep.find('{').ok_or("setShapedTile: no body")?];
    let marking = part(body(sweep), "drawCycle++;", "@Pc(240) boolean isUnderwater")?;
    let sweeps = &body(sweep)[body(sweep)
        .find("@Pc(434) int nearY;")
        .ok_or("setShapedTile: no sweeps")?..];

    // The end of buildScene: runs of marks become occluders.
    let build = body(method(scene, "public static void buildScene(")?);
    let inner = build
        .find("for (col = 0; col <= 104; col++) {")
        .ok_or("buildScene: no occluder loop")?;
    let start = build[..inner]
        .rfind("for (currentLevel = 0; currentLevel < 4; currentLevel++) {")
        .ok_or("buildScene: no occluder level loop")?;
    let runs = &build[start..];
    if !runs.contains("Occluder.add(4,") {
        return Err("buildScene: the occluder loop is not the last statement".into());
    }
    // buildScene's floor rule, from reading the tile to the mark.
    let floor = part(
        build,
        "k = tileUnderlays[currentLevel][i][j] & 0xFF;",
        "if (k <= 0) {",
    )?;
    if !floor.contains("occludeFlags[currentLevel][i][j] |= 0x4;") {
        return Err("buildScene: the floor rule moved".into());
    }

    // addLoc's marks, each inside the branch of the shape chain it has in
    // the original; the branch conditions are the original's lines.
    let add = method(
        scene,
        "public static void addLoc(@OriginalArg(0) int currentPlane",
    )?;
    let branches = [
        "if (locType == LocType.GROUNDDECOR) {",
        "} else if (locType == LocType.CENTREPIECE_STRAIGHT || locType == LocType.CENTREPIECE_DIAGONAL) {",
        "} else if (locType >= LocType.ROOF_STRAIGHT) {",
        "} else if (locType == LocType.WALL_STRAIGHT) {",
        "} else if (locType == LocType.WALL_DIAGONALCORNER) {",
        "} else if (locType == LocType.WALL_L) {",
    ];
    let mut last = 0;
    for branch in branches {
        let at = add
            .find(branch)
            .ok_or_else(|| format!("addLoc: missing branch {branch}"))?;
        if at < last {
            return Err(format!("addLoc: branch out of order: {branch}").into());
        }
        last = at;
    }
    let hidden = line(add, "if (GlRenderer.enabled && loc.render) {")?;
    let roof = method(add, "if (lowmem && locType <= LocType.ROOF_FLAT")?;
    let straight = method(add, "if (lowmem) {")?;
    let corner = method(add, "if (loc.occlude && lowmem) {")?;
    for (name, text) in [("straight", straight), ("corner", corner), ("roof", roof)] {
        if !text.contains("occludeFlags[plane]") {
            return Err(format!("addLoc: the {name} wall marks moved").into());
        }
    }
    let marks = format!(
        "{hidden}\n\t\t\treturn;\n\t\t}}\n\t\t{}\n\t\t{}\n\t\t{}\n\t\t\t{roof}\n\t\t{}\n\t\t\t{straight}\n\t\t{}\n\t\t{}\n\t\t\t{corner}\n\t\t}}",
        branches[0], branches[1], branches[2], branches[3], branches[4], branches[5]
    );

    let methods = [
        "public static boolean isOccluded(",
        "public static boolean isAreaVisible(",
        "public static boolean isTileVisible(",
        "public static boolean isWallVisible(",
        "public static boolean isTileOccluded(",
        "public static void buildActiveOccluders(",
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
    ]
    .iter()
    .map(|signature| method(scene, signature))
    .collect::<Result<Vec<_>>>()?
    .join("\n\n\t");
    Ok(format!(
        "{HEADER}// SceneGraph's occluders and tile walk, copied unchanged. Ground drawing\n// only records the tile.\npublic final class SceneGraph {{\n{tables}\n\tpublic static java.util.ArrayList<String> out = new java.util.ArrayList<String>();\n\tpublic static Tile[][][] tiles;\n\tpublic static int width = 104;\n\tpublic static int length = 104;\n\tpublic static int levels = 4;\n\tpublic static int startLevel = 0;\n\tpublic static int[][][] tileHeights;\n\tpublic static int[][][] surfaceTileHeights;\n\tpublic static int[][][] underwaterTileHeights;\n\tpublic static int[][] tileWaterData;\n\tpublic static Scenery[] scenery;\n\tpublic static int sceneryLen;\n\tpublic static final Scenery[] sceneryBuffer = new Scenery[100];\n\tpublic static LinkedList tileQueue = new LinkedList();\n\tpublic static int drawCycle;\n\tpublic static int visibleTileCount;\n\tpublic static int cameraX;\n\tpublic static int cameraY;\n\tpublic static int cameraZ;\n\tpublic static int cameraTileX;\n\tpublic static int cameraTileY;\n\tpublic static int sinPitch;\n\tpublic static int cosPitch;\n\tpublic static int sinYaw;\n\tpublic static int cosYaw;\n\tpublic static int visibility;\n\tpublic static int lastUnderwaterColor;\n\tpublic static int lastUnderwaterFogRange;\n\tpublic static boolean[][] visibleTiles;\n\tpublic static int[][][] occludeFlags;\n\tpublic static byte[][][] shadowmap;\n\tpublic static byte[][][] tileShapes;\n\tpublic static byte[][][] tileUnderlays;\n\tpublic static byte[][][] tileOverlays;\n\tpublic static Occluder[] occluders;\n\tpublic static int occluderCount;\n\tpublic static Occluder[] activeOccluders;\n\tpublic static int activeOccluderCount = 0;\n\tpublic static int[][][] occlusionCache;\n\tpublic static int[] roofGroupMaxHeight;\n\tpublic static int[] roofGroupMinX;\n\tpublic static int[] roofGroupMaxX;\n\tpublic static int[] roofGroupMaxZ;\n\tpublic static int[] roofGroupMinZ;\n\n\tpublic static void drawPlainTile(PlainTile tile, int level, int sinPitch, int cosPitch, int cosYaw, int sinYaw, int tileX, int tileY, boolean occluded) {{\n\t\tout.add(\"g\" + level + \",\" + tileX + \",\" + tileY);\n\t}}\n\n\tpublic static void drawShapedTile(ShapedTile tile, int sinPitch, int cosPitch, int cosYaw, int sinYaw, int tileX, int tileY, int level, boolean occluded) {{\n\t\tout.add(\"g\" + level + \",\" + tileX + \",\" + tileY);\n\t}}\n\n\t// The start of setPlainTile.\n\tpublic static void camera(int camX, int camZ, int camY, int pitch, int yaw) {{\n\t\t{prefix}\n\t}}\n\n\t// setShapedTile without its GL block between marking and sweeping.\n\t{signature}{{\n\t\t{marking}boolean isUnderwater = false;\n\t\t{sweeps}\n\t}}\n\n\t// The end of buildScene.\n\tpublic static void buildOccluders() {{\n\t\tint currentLevel;\n\t\tint col;\n\t\tint i;\n\t\tint j;\n\t\tint k;\n\t\tint m;\n\t\tint overlay;\n\t\tint n;\n\t\tint z;\n\t\tint q;\n\t\tint r;\n\t\tint s;\n\t\tint p;\n\t\t{runs}\n\t}}\n\n\t// buildScene for one tile, up to its occlusion mark.\n\tpublic static void floorMark(int currentLevel, int i, int j) {{\n\t\tint k;\n\t\tint overlay;\n\t\tint m;\n\t\tint n;\n\t\tint z;\n\t\tint p;\n\t\t{floor}}}\n\t}}\n\n\t// addLoc's occlusion marks in its chain of shapes.\n\tpublic static void locMarks(boolean lowmem, int plane, int x, int y, int locType, int orientation, LocInfo loc) {{\n\t\t{marks}\n\t}}\n\n\t{methods}\n}}\n"
    ))
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let read = |name: &str| -> Result<String> {
        Ok(String::from_utf8(normalized(&fs::read(
            root.join(format!("{JAVA}/{name}.java")),
        )?))?)
    };
    let build = root.join("runs/occlusion-oracle");
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
        ("SceneGraph", holder(&read("SceneGraph")?)?),
        ("LocType", loc_type(&read("LocType")?)),
    ] {
        fs::write(generated.join(format!("{name}.java")), text)?;
        sources.push(format!("runs/occlusion-oracle/generated/rt4/{name}.java"));
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
        .arg("rt4.OcclusionOracle")
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
        "scope": "Text copied unchanged from SceneGraph (isOccluded, isTileOccluded, isTileVisible, isWallVisible, isAreaVisible, buildActiveOccluders, the occluder loop that ends buildScene, buildScene's statements for one tile from reading its floor ids to its occlusion mark, addLoc's occlusion marks under the original's own shape-chain conditions, renderTile, setShapedTile without its GL block, the camera part of setPlainTile, addSceneryEntity, add, setWall, setWallDecor, setGroundDecor, setObjStack, removeScenery, bridgeTile, setBaseLevel) and LocType (its shape constants), compiled with the unchanged Occluder, Tile, Scenery, Wall, WallDecor, GroundDecor, ObjStackEntity, MathUtils, Node, LinkedList and GlobalConfig. Models are stand-ins that record being drawn and report a given getMinY; GL, lighting and raster calls are empty; ground kinds, heights, floor ids, the visibility map and roof-group bounds are set directly. Not observed: how addLoc reaches its marks from a map file (readLocs, the opcode-82 return's place in the whole method), FloType decoding, drawPlainTile's own tests, the client's wiring of these into its frame."
    });
    let directory = root.join("fixtures/occlusion");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.txt"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "occlusion oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("occlusion oracle provenance is stale; review source changes".into());
        }
        println!("{total} JVM occlusion observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM occlusion observations: {counts:?}");
    }
    Ok(())
}
