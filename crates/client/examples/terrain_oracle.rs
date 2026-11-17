//! `cargo run -p rs09-client --example terrain_oracle [-- --check] [--underwater]`
//!
//! The JVM differential for the HD floor mesh, the baked tile shade and the
//! merged scenery normals. Scenes are cut from the pinned cache with the
//! client's own decoders and written to `runs/terrain-oracle/input.bin`; the
//! original code runs over them and its results become `fixtures/terrain`.
//!
//! The original code is the unchanged classes in `UNCHANGED` plus the method
//! bodies listed below, copied verbatim out of `SceneGraph`, `GlModel` and
//! `Rasteriser` into classes of the same name (those three files reference
//! most of the client and cannot be compiled whole). The adapters are in
//! `oracle/terrain`. This lives beside the client rather than in `xtask`
//! because the scenes come from the client crate's decoders.
//!
//! A second pass does the same for the underwater scene (`um`/`ul` map
//! files): `oracle/underwater/rt4/UnderwaterOracle.java` runs the original
//! loader and builder over scenes `support::underwater` cuts, and its results
//! become `fixtures/underwater`. The first pass and its fixture are unchanged
//! by it.
#[path = "../tests/terrain_support/mod.rs"]
mod support;

use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const RT4: &str = "reference/client/client/src/main/java/rt4";
/// Original classes compiled as they are.
const UNCHANGED: &[&str] = &[
    "ArrayUtils",
    "Buffer",
    "ColorUtils",
    "Entity",
    "FloType",
    "FluType",
    "GlBoundingBox",
    "GlModelRecoloring",
    "GlTile",
    "GroundDecor",
    "HashTable",
    "IntNode",
    "IntUtils",
    "Node",
    "PlainTile",
    "Scenery",
    "ShapedTile",
    "TextureProvider",
    "Tile",
    "Wall",
    "WallDecor",
];
const ADAPTERS: &[&str] = &[
    "oracle/adapters/rt4/JagString.java",
    "oracle/terrain/com/jogamp/opengl/GL2.java",
    "oracle/terrain/rt4/Stubs.java",
    "oracle/terrain/rt4/TerrainOracle.java",
];
/// Whole declarations copied from `SceneGraph`, each named by a unique prefix.
const SCENE_LINES: &[&str] = &[
    "public static final byte[][][] renderFlags =",
    "public static final int[] WALL_DECO_ROT_SIZE_Y_DIR =",
    "public static final int[] WALL_OFFSET_Y =",
    "public static final int[] WALL_OFFSET_X =",
    "public static final int[] WALL_ROTATION_TYPE1 =",
    "public static final int[][] OVERLAY_SHAPE_VERTICES =",
    "public static final int[] WALL_DECO_ROT_SIZE_X_DIR =",
    "public static final boolean[][] OVERLAY_TRIANGLE_FLAGS =",
    "public static final int[][] UNDERLAY_SHAPE_VERTICES =",
    "public static final int[] SHAPE_BLEND_VERTEX_COUNT =",
    "public static final int[] overlayEdgeLookup =",
    "public static final boolean[][] tileEdgeMasks =",
];
const SCENE_METHODS: &[&str] = &[
    "public static boolean allLevelsAreVisible(",
    "public static void setGroundDecor(",
    "public static void setWall(",
    "public static long getWallKey(",
    "public static void mergeSceneNormals(",
    "public static boolean addSceneryEntity(",
    "public static void setWallDecor(",
    "public static void addLoc(@OriginalArg(0) int currentPlane,",
    "public static void readLocs(",
    "public static void mergeGroundDecorNormals(",
    "public static void mergeEntityNormals(",
    "public static void setWallDecoration(",
    "public static GlTile[] buildGlPlainTiles(",
    "public static void addGlTileVertices(",
    "public static boolean addEntity(",
    "public static int getRenderLevel(",
    "public static GlTile[] buildGlShapedTiles(",
    "public static GlTile getOrCreateGlTile(",
    "public static int getTexturedColor(",
    "public static void addGlPlainTile(",
    "public static int computeGlTileVertex(",
    "public static int interpolateHeight(",
    "public static int getSmoothedHeight(",
    "public static void clearTerrainRegion(",
];
const MODEL_METHODS: &[&str] = &[
    "public final boolean canMerge(",
    "public final void mergeNormals(",
    "private void calculateBounds(",
    "public final void alignToTerrain(",
    "public final Entity createModel(",
    "public final int getLengthXZ(",
];

/// Besides `SCENE_METHODS`, for the underwater pass.
const UNDERWATER_METHODS: &[&str] = &[
    "public static void load(",
    "public static void unload(",
    "public static void setUnderwater(",
    "public static void readTile(",
    "public static void setTile(",
];
/// Fields those methods and the `buildScene` fragments name.
const UNDERWATER_FIELDS: &str = "\tpublic static Tile[][][] surfaceGroundTiles;\n\tpublic static Tile[][][] underWaterGroundTiles;\n\tpublic static GlTile[][] surfaceHdTiles;\n\tpublic static GlTile[][] underWaterHdTiles;\n\tpublic static GlTile[][] underwaterHdTiles;\n\tpublic static byte[][][] tileShapes;\n\tpublic static byte[][][] tileOverlays;\n\tpublic static byte[][][] tileAngles;\n\tpublic static byte[][][] tileUnderlays;\n\tpublic static int[] rowWeightedHue;\n\tpublic static int[] rowSaturation;\n\tpublic static int[] rowChroma;\n\tpublic static int[] rowCount;\n\tpublic static int[] rowLightness;\n";
/// What the underwater pass adds to `oracle/terrain/rt4/Stubs.java`: the
/// three names the extra original code reads that the terrain pass does not.
const STUB_ADDITIONS: &[(&str, &str)] = &[
    (
        "class MaterialManager {\n",
        "class MaterialManager {\n\tstatic final int WATER = 4;\n\n",
    ),
    (
        "class ShadowManager {\n",
        "class ShadowManager {\n\tstatic void addFloorShadow(int shape, int angle, boolean overlayClear, boolean underlayClear, int x, int y, int sw, int se, int ne, int nw) {\n\t}\n\n",
    ),
];
const STUB_APPENDIX: &str = "\nclass PerlinNoise {\n\t// Only the surface branch of readTile reads the procedural height.\n\tstatic int getTileHeight(int x, int y) {\n\t\tthrow new UnsupportedOperationException();\n\t}\n}\n";

type Result<T> = std::result::Result<T, String>;

fn read(root: &Path, path: &str) -> Result<String> {
    fs::read_to_string(root.join(path))
        .map(|text| text.replace('\r', ""))
        .map_err(|e| format!("{path}: {e}"))
}

/// The whole method whose declaration starts with `signature`.
fn method(source: &str, signature: &str) -> Result<String> {
    let start = source.find(signature).ok_or(format!("no `{signature}`"))?;
    if source[start + 1..].contains(signature) {
        return Err(format!("`{signature}` is not unique"));
    }
    let open = start + source[start..].find('{').ok_or("no body")?;
    let mut depth = 0;
    for (offset, c) in source[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(source[start..=open + offset].to_owned());
                }
            }
            _ => {}
        }
    }
    Err(format!("`{signature}` does not end"))
}

fn line<'a>(source: &'a str, prefix: &str) -> Result<&'a str> {
    source
        .lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .ok_or(format!("no `{prefix}`"))
}

/// The text between two markers of `source`, exclusive.
fn between<'a>(source: &'a str, from: &str, to: &str) -> Result<&'a str> {
    let start = source.find(from).ok_or(format!("no `{from}`"))? + from.len();
    let end = source[start..].find(to).ok_or(format!("no `{to}`"))?;
    Ok(&source[start..start + end])
}

fn generate(root: &Path, directory: &Path, underwater: bool) -> Result<Vec<String>> {
    let scene = read(root, &format!("{RT4}/SceneGraph.java"))?;
    let build = method(&scene, "public static void buildScene(")?;
    let mut text = String::from(
        "package rt4;\n\nimport org.openrs2.deob.annotation.OriginalArg;\nimport org.openrs2.deob.annotation.Pc;\n\n// Generated by examples/terrain_oracle.rs: verbatim declarations and methods of the original SceneGraph.\npublic final class SceneGraph {\n",
    );
    for prefix in SCENE_LINES {
        text += line(&scene, prefix)?;
        text.push('\n');
    }
    if underwater {
        text += UNDERWATER_FIELDS;
    }
    text += "\tpublic static Tile[][][] tiles;\n\tpublic static int[][][] tileHeights;\n\tpublic static int[][][] surfaceTileHeights;\n\tpublic static int[][][] underwaterTileHeights;\n\tpublic static int[][][] occludeFlags;\n\tpublic static byte[][][] shadowmap;\n\tpublic static int[][] tileWaterData;\n\tpublic static Scenery[] scenery;\n\tpublic static int sceneryLen = 0;\n\tpublic static int firstVisibleLevel = 99;\n\tpublic static int centralPlane = 0;\n\tpublic static int width;\n\tpublic static int length;\n\tpublic static int levels;\n";
    let extra: &[&str] = if underwater { UNDERWATER_METHODS } else { &[] };
    for signature in SCENE_METHODS.iter().chain(extra) {
        text += "\n\t";
        text += &method(&scene, signature)?;
        text.push('\n');
    }
    // Two fragments of buildScene, which as a whole needs the entire client:
    // the HD tileLightness loop and the per-corner slope normals.
    text += "\n\tstatic void oracleTileLightness(byte[][] shadowData, int[][] tileLightness) {\n\t\tint i;\n\t\tint j;\n\t\tint overlay;\n";
    text += between(
        &build,
        "} else if (Preferences.highDetailLighting) {\n",
        "\t\t\t} else {\n",
    )?;
    // The loop buildScene runs instead without high detail lighting: slope
    // against FogManager.light0Position, less 1.7 times the shade.
    let standard = "\t\t\t\ti = (int) FogManager.light0Position[0];";
    text += "\t}\n\n\tstatic void oracleTileLightnessSd(int currentLevel, byte[][] shadowData, int[][] tileLightness) {\n\t\tint i;\n\t\tint j;\n\t\tint k;\n\t\tint overlay;\n\t\tint m;\n\t\tint n;\n\t\tint p;\n\t\tint q;\n\t\tint r;\n\t\tint s;\n\t\tint t;\n\t\tint u;\n\t\tint v;\n\t\tint w;\n\t\tint z;\n";
    text += standard;
    text += between(
        &build,
        &format!("\t\t\t}} else {{\n{standard}"),
        "\t\t\t}\n\t\t\tfor (i = 0; i < 104; i++) {\n\t\t\t\trowWeightedHue[i] = 0;",
    )?;
    text += "\t}\n\n\tstatic void oracleNormals(int[][] levelHeights, float[][] normalsX, float[][] normalsY, float[][] normalsZ) {\n\t\tint m;\n\t\tint n;\n\t\tint z;\n\t\tint p;\n\t\tfor (m = 1; m <= 103; m++) {\n\t\t\t\t\t";
    let normals = between(
        &build,
        "\t\t\t\t\tfor (n = 1; n <= 103; n++) {",
        "\t\t\t\t\tm++;",
    )?;
    text += "for (n = 1; n <= 103; n++) {";
    text += normals;
    text += "\t\t}\n\t}\n";
    if underwater {
        // Two more fragments of buildScene: the blended underlay colours of
        // a level, and the tile loop, which fills tileWaterData on the
        // surface and makes the tile objects.
        let blend = "\t\t\tfor (i = 0; i < 104; i++) {\n\t\t\t\trowWeightedHue[i] = 0;";
        let tiles = "\t\t\tfor (i = 1; i < 103; i++) {\n\t\t\t\ttileLoop:";
        text += "\n\tstatic void oracleUnderlayColors(int currentLevel, int[][] underlayColors) {\n\t\tint i;\n\t\tint j;\n\t\tint k;\n\t\tint overlay;\n\t\tint m;\n\t\tint n;\n\t\tint p;\n\t\tint z;\n\t\tint q;\n";
        text += blend;
        text += between(&build, blend, tiles)?;
        text += "\t}\n\n\tstatic void oracleTiles(boolean underwater, int currentLevel, int[][] underlayColors, int[][] tileLightness, int x, int level) {\n\t\tint i;\n\t\tint j;\n\t\tint k;\n\t\tint overlay;\n\t\tint m;\n\t\tint n;\n\t\tint p;\n\t\tint z;\n\t\tint q;\n\t\tint r;\n\t\tint s;\n\t\tint t;\n\t\tint v;\n\t\tint w;\n\t\tint u;\n";
        text += tiles;
        text += between(
            &build,
            tiles,
            "\t\t\tif (GlRenderer.enabled) {\n\t\t\t\t@Pc(1888) float[][] normalsY",
        )?;
        text += "\t}\n";
    }
    text += "}\n";
    fs::create_dir_all(directory.join("rt4")).map_err(|e| e.to_string())?;
    fs::write(directory.join("rt4/SceneGraph.java"), text).map_err(|e| e.to_string())?;

    let model = read(root, &format!("{RT4}/GlModel.java"))?;
    let mut text = String::from(
        "package rt4;\n\nimport org.openrs2.deob.annotation.OriginalArg;\nimport org.openrs2.deob.annotation.Pc;\n\n// Generated by examples/terrain_oracle.rs: verbatim methods of the original GlModel over the fields they read.\npublic final class GlModel extends Model {\n\tpublic int vertexCount;\n\tpublic int[] vertexX;\n\tpublic int[] vertexY;\n\tpublic int[] vertexZ;\n\tpublic int triangleCount;\n\tpublic int uniqueVertexCount;\n\tpublic short[] normalX;\n\tpublic short[] normalY;\n\tpublic short[] normalZ;\n\tpublic short[] normalMagnitude;\n\tpublic GlModelRecoloring recoloring;\n\tpublic int[] vertexOffsets;\n\tpublic short[] vertexLookup;\n\tpublic boolean mergeable;\n\tpublic final GlBoundingBox bounds = new GlBoundingBox();\n\tpublic final GlBuffer vertexBuffer = new GlBuffer();\n\n\tpublic void updateModel(int pitch, int yaw, int roll, int x, int z) {\n\t}\n\n\tpublic void render(int yaw, int sinCameraPitch, int cosCameraPitch, int sinCameraYaw, int cosCameraYaw, int sceneX, int sceneZ, int sceneY, long key, int plane, ParticleSystem particleSystem) {\n\t}\n\n\tpublic int getMinY() {\n\t\treturn 0;\n\t}\n",
    );
    for signature in MODEL_METHODS {
        text += "\n\t";
        text += &method(&model, signature)?;
        text.push('\n');
    }
    text += "}\n";
    fs::write(directory.join("rt4/GlModel.java"), text).map_err(|e| e.to_string())?;

    let raster = read(root, &format!("{RT4}/Rasteriser.java"))?;
    let text = format!(
        "package rt4;\n\nimport org.openrs2.deob.annotation.Pc;\n\n// Generated by examples/terrain_oracle.rs: the original palette at the HD brightness (DisplayMode sets 0.7).\npublic final class Rasteriser {{\n\tpublic static TextureProvider textureProvider;\n\tpublic static final int[] palette = new int[65536];\n\tprivate static float brightness = 0.7F;\n\n\tstatic void oracleInit() {{\n\t\tcalculateBrightness();\n\t}}\n\n\t// Rasteriser.setBrightness without randBrightness's random offset.\n\tstatic void oracleBrightness(float value) {{\n\t\tbrightness = value;\n\t\tcalculateBrightness();\n\t}}\n\n\t{}\n}}\n",
        method(&raster, "private static void calculateBrightness(")?
    );
    fs::write(directory.join("rt4/Rasteriser.java"), text).map_err(|e| e.to_string())?;
    Ok(["SceneGraph", "GlModel", "Rasteriser"]
        .map(|name| format!("{}/rt4/{name}.java", directory.display()))
        .to_vec())
}

fn run(command: &mut Command) -> Result<()> {
    let output = command.output().map_err(|e| format!("{command:?}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command:?} failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() {
    // `--underwater` runs the second pass alone.
    let result = if std::env::args().any(|arg| arg == "--underwater") {
        underwater()
    } else {
        oracle().and_then(|()| underwater())
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn oracle() -> Result<()> {
    let check = std::env::args().any(|arg| arg == "--check");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = support::open(&root)?;
    let scenes = support::SCENES
        .iter()
        .map(|(name, base)| source.scene(name, *base))
        .collect::<Result<Vec<_>>>()?;
    let build = root.join("runs/terrain-oracle");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&classes).map_err(|e| e.to_string())?;
    let input = support::input(&source, &scenes);
    fs::write(build.join("input.bin"), &input).map_err(|e| e.to_string())?;

    let generated = generate(&root, &build.join("gen"), false)?;
    let mut sources: Vec<String> = UNCHANGED
        .iter()
        .map(|name| format!("{RT4}/{name}.java"))
        .chain(ADAPTERS.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for path in sources
        .iter()
        .cloned()
        .chain(["SceneGraph", "GlModel", "Rasteriser"].map(|name| format!("{RT4}/{name}.java")))
    {
        hashes.insert(path.clone(), hex(read(&root, &path)?.as_bytes()));
    }
    run(Command::new("javac")
        .current_dir(&root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .args(&generated))?;
    run(Command::new("java")
        .current_dir(&root)
        .arg("-cp")
        .arg(&classes)
        .arg("rt4.TerrainOracle")
        .arg(build.join("input.bin"))
        .arg(build.join("output.bin")))?;
    let output = fs::read(build.join("output.bin")).map_err(|e| e.to_string())?;
    let original = support::read_sections(&output);

    // The Rust ports over the same scenes, section by section.
    let mut ported = vec![("clear/shadow".to_owned(), support::clear_section())];
    ported.extend(support::palette_sections());
    for scene in &scenes {
        let (sections, stats) = support::sections(&source, scene);
        println!("{}: {stats:?}", scene.name);
        for (name, bytes) in sections {
            ported.push((format!("{}/{name}", scene.name), bytes));
        }
    }
    let mut differences = 0;
    for ((name, java), (rust_name, rust)) in original.iter().zip(&ported) {
        if name != rust_name {
            return Err(format!("section order differs: {name} / {rust_name}"));
        }
        if java != rust {
            differences += 1;
            let at = java
                .iter()
                .zip(rust)
                .position(|(a, b)| a != b)
                .unwrap_or(java.len().min(rust.len()));
            eprintln!(
                "{name}: differs at byte {at} of {} (Rust has {})",
                java.len(),
                rust.len()
            );
            fs::write(
                build.join(format!("{}.java.bin", name.replace('/', "-"))),
                java,
            )
            .map_err(|e| e.to_string())?;
            fs::write(
                build.join(format!("{}.rust.bin", name.replace('/', "-"))),
                rust,
            )
            .map_err(|e| e.to_string())?;
        }
    }
    if original.len() != ported.len() {
        return Err(format!(
            "{} original sections, {} ported",
            original.len(),
            ported.len()
        ));
    }

    let sections: BTreeMap<_, _> = original
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                serde_json::json!({"bytes": bytes.len(), "sha256": hex(bytes)}),
            )
        })
        .collect();
    let fixture = serde_json::json!({
        "format": 1,
        "scenes": support::SCENES.iter().map(|(name, base)| serde_json::json!({"name": name, "base": base})).collect::<Vec<_>>(),
        "inputSha256": hex(&input),
        "sections": sections,
    });
    let provenance = serde_json::json!({
        "format": 1,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "extracted": {
            "SceneGraph": SCENE_METHODS.iter().chain(SCENE_LINES).collect::<Vec<_>>(),
            "SceneGraph.buildScene fragments": ["the HD tileLightness loop", "the tileLightness loop without high detail lighting", "the corner slope normals"],
            "GlModel": MODEL_METHODS,
            "Rasteriser": ["private static void calculateBrightness("],
        },
        "scope": "Original buildGlPlainTiles, addGlTileVertices, buildGlShapedTiles, addGlPlainTile, computeGlTileVertex, getTexturedColor, GlTile.addVertex/addFace/uploadVertexData, readLocs, addLoc (tile slots and shadow map), clearTerrainRegion, the HD tileLightness loop, mergeSceneNormals, mergeEntityNormals, mergeGroundDecorNormals, GlModel.mergeNormals/createModel/calculateBounds/alignToTerrain over 104x104 scenes cut from the pinned cache. For high detail lighting off: calculateBrightness at the palette brightness of preferences 1, 2 and 4 (set directly; setBrightness's random offset is not run), the other tileLightness loop of buildScene under two values of FogManager.light0Position, and under the first the floor builders again with that lightness and the palette of preference 4, over fresh copies of the tile angles. The tile arrays, heights, blended underlay colours, location type facts, landscape streams and placed location models (vertices, packed-vertex normals and lookup slots) are produced by the Rust decoders and are inputs, not observations. Rotations 4..7 of location models lack the extra 45 degree turn. No rendering, GL state or draw order is observed.",
    });
    let directory = root.join("fixtures/terrain");
    let fixture_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&fixture).map_err(|e| e.to_string())?
    );
    let provenance_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&provenance).map_err(|e| e.to_string())?
    );
    if check {
        let same = |name: &str, text: &str| {
            fs::read_to_string(directory.join(name))
                .map(|old| old.replace('\r', "") == text)
                .unwrap_or(false)
        };
        if !same("oracle.json", &fixture_text) {
            return Err("terrain oracle output differs from the committed fixture".into());
        }
        if !same("provenance.json", &provenance_text) {
            return Err("terrain oracle provenance is stale".into());
        }
        println!("{} JVM terrain sections reproduced.", original.len());
    } else {
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        fs::write(directory.join("oracle.json"), fixture_text).map_err(|e| e.to_string())?;
        fs::write(directory.join("provenance.json"), provenance_text).map_err(|e| e.to_string())?;
        println!("Wrote {} JVM terrain sections.", original.len());
    }
    if differences != 0 {
        return Err(format!(
            "{differences} sections differ between the original and the port"
        ));
    }
    println!("The Rust ports reproduce every section.");
    Ok(())
}

/// The underwater pass: its own input, classes, output and fixture.
fn underwater() -> Result<()> {
    use support::underwater as under;
    let check = std::env::args().any(|arg| arg == "--check");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = support::open(&root)?;
    let scenes = under::SCENES
        .iter()
        .map(|(name, base)| under::scene(&source, name, *base))
        .collect::<Result<Vec<_>>>()?;
    let build = root.join("runs/terrain-oracle/underwater");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&classes).map_err(|e| e.to_string())?;
    let input = under::input(&source, &scenes);
    fs::write(build.join("input.bin"), &input).map_err(|e| e.to_string())?;

    let mut generated = generate(&root, &build.join("gen"), true)?;
    let mut stubs = read(&root, "oracle/terrain/rt4/Stubs.java")?;
    for (from, to) in STUB_ADDITIONS {
        if stubs.matches(from).count() != 1 {
            return Err(format!(
                "the terrain stubs have no single `{}`",
                from.trim()
            ));
        }
        stubs = stubs.replace(from, to);
    }
    stubs += STUB_APPENDIX;
    let stubs_path = build.join("gen/rt4/Stubs.java");
    fs::write(&stubs_path, stubs).map_err(|e| e.to_string())?;
    generated.push(stubs_path.display().to_string());
    let adapters = [
        "oracle/adapters/rt4/JagString.java",
        "oracle/terrain/com/jogamp/opengl/GL2.java",
        "oracle/underwater/rt4/UnderwaterOracle.java",
    ];
    let mut sources: Vec<String> = UNCHANGED
        .iter()
        .map(|name| format!("{RT4}/{name}.java"))
        .chain(adapters.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for path in sources.iter().cloned().chain(
        ["SceneGraph", "GlModel", "Rasteriser"]
            .map(|name| format!("{RT4}/{name}.java"))
            .into_iter()
            .chain(["oracle/terrain/rt4/Stubs.java".to_owned()]),
    ) {
        hashes.insert(path.clone(), hex(read(&root, &path)?.as_bytes()));
    }
    run(Command::new("javac")
        .current_dir(&root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .args(&generated))?;
    run(Command::new("java")
        .current_dir(&root)
        .arg("-cp")
        .arg(&classes)
        .arg("rt4.UnderwaterOracle")
        .arg(build.join("input.bin"))
        .arg(build.join("output.bin")))?;
    let output = fs::read(build.join("output.bin")).map_err(|e| e.to_string())?;
    let original = support::read_sections(&output);

    let mut ported = Vec::new();
    for scene in &scenes {
        let (sections, stats) = under::sections(&source, scene);
        println!("{}: {stats:?}", scene.name);
        for (name, bytes) in sections {
            ported.push((format!("{}/{name}", scene.name), bytes));
        }
    }
    if original.len() != ported.len() {
        return Err(format!(
            "{} original underwater sections, {} ported",
            original.len(),
            ported.len()
        ));
    }
    let mut differences = 0;
    for ((name, java), (rust_name, rust)) in original.iter().zip(&ported) {
        if name != rust_name {
            return Err(format!("section order differs: {name} / {rust_name}"));
        }
        if java != rust {
            differences += 1;
            let at = java
                .iter()
                .zip(rust)
                .position(|(a, b)| a != b)
                .unwrap_or(java.len().min(rust.len()));
            eprintln!(
                "{name}: differs at byte {at} of {} (Rust has {})",
                java.len(),
                rust.len()
            );
            for (side, bytes) in [("java", java), ("rust", rust)] {
                fs::write(
                    build.join(format!("{}.{side}.bin", name.replace('/', "-"))),
                    bytes,
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    let sections: BTreeMap<_, _> = original
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                serde_json::json!({"bytes": bytes.len(), "sha256": hex(bytes)}),
            )
        })
        .collect();
    let regions = under::regions(&source);
    let fixture = serde_json::json!({
        "format": 1,
        "scenes": under::SCENES.iter().map(|(name, base)| serde_json::json!({"name": name, "base": base})).collect::<Vec<_>>(),
        "inputSha256": hex(&input),
        "sections": sections,
    });
    let provenance = serde_json::json!({
        "format": 1,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "extracted": {
            "SceneGraph": SCENE_METHODS.iter().chain(UNDERWATER_METHODS).chain(SCENE_LINES).collect::<Vec<_>>(),
            "SceneGraph.buildScene fragments": ["the blended underlay colours", "the tile loop (tileWaterData, setTile)", "the HD tileLightness loop", "the corner slope normals"],
            "GlModel": MODEL_METHODS,
            "Rasteriser": ["private static void calculateBrightness("],
        },
        "stubAdditions": STUB_ADDITIONS.iter().map(|(_, to)| *to).chain([STUB_APPENDIX]).collect::<Vec<_>>(),
        "scope": "Original SceneGraph.load, setUnderwater, readTile (underwater branch), clearTerrainRegion, readLocs and addLoc with highmem set, setWall, setWallDecor, setGroundDecor, addSceneryEntity (with its tileWaterData spread), setTile, mergeSceneNormals, buildGlPlainTiles with tileWaterData and the surface heights, addGlTileVertices, computeGlTileVertex, GlTile.addVertex/addFace/uploadVertexData (40-byte vertices with the underwater coordinate), GlModel.alignToTerrain, and four buildScene fragments (the blended underlay colours, the tile loop, the HD tileLightness loop, the corner normals), over 104x104 scenes cut from the pinned cache. The um and ul files are passed raw. The surface's ground-level tile arrays, render flags and corner heights, the floor definitions, each texture's brightness, speed and material type, and each location type's facts and models (as getGlModel and the diagonal turn leave them) are produced by the Rust decoders and are inputs, not observations. The adapter repeats, in their order, the calls of LoginManager.rebuildMap, SceneGraph.init, readStaticTerrain, readStaticLocs and the 64x64 tile loop of SceneGraph.readTerrain, whose tail (occlusion records, environment, lights) is not compiled. No rendering, GL state or draw order is observed.",
        "underwaterRegions": {
            "count": regions.len(),
            "withTerrain": regions.iter().filter(|r| r.2).count(),
            "withLocations": regions.iter().filter(|r| r.3).count(),
            "regions": regions.iter().map(|(x, z, _, _)| format!("{x}_{z}")).collect::<Vec<_>>(),
        },
    });
    let directory = root.join("fixtures/underwater");
    let fixture_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&fixture).map_err(|e| e.to_string())?
    );
    let provenance_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&provenance).map_err(|e| e.to_string())?
    );
    if check {
        let same = |name: &str, text: &str| {
            fs::read_to_string(directory.join(name))
                .map(|old| old.replace('\r', "") == text)
                .unwrap_or(false)
        };
        if !same("oracle.json", &fixture_text) {
            return Err("underwater oracle output differs from the committed fixture".into());
        }
        if !same("provenance.json", &provenance_text) {
            return Err("underwater oracle provenance is stale".into());
        }
        println!("{} JVM underwater sections reproduced.", original.len());
    } else {
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        fs::write(directory.join("oracle.json"), fixture_text).map_err(|e| e.to_string())?;
        fs::write(directory.join("provenance.json"), provenance_text).map_err(|e| e.to_string())?;
        println!("Wrote {} JVM underwater sections.", original.len());
    }
    if differences != 0 {
        return Err(format!(
            "{differences} underwater sections differ between the original and the port"
        ));
    }
    println!("The Rust port reproduces every underwater section.");
    Ok(())
}
