//! `cargo xtask minimap-oracle [--check]`: the minimap's drift, player dots
//! and text labels observed from the original Java; the observations become
//! `fixtures/minimap/drift-dots-labels.jsonl`.
//!
//! `MiniMap`, `Protocol`, `client`, `SceneGraph`, `Player` and `Camera` cannot
//! be compiled apart from the whole client. The statements under test are
//! therefore copied verbatim, by the anchors below, into a class generated
//! from `oracle/minimap/jvm/MiniMap.java.template`; everything those
//! statements call (`Font`, `JagString`, `Base37`, `MapElementList`,
//! `MathUtils`, `SpriteLoader` and the cache classes) is the unchanged file.
use crate::{Result, cache_assets, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

const JAVA: &str = "reference/client/client/src/main/java/rt4";
/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "Font",
    "JagString",
    "StringInterface",
    "StringNode",
    "Base37",
    "MapElementList",
    "MathUtils",
    "ClanMember",
    "SpriteLoader",
    "SecondaryNode",
    "Node",
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
    "ThreadUtils",
];
/// Files the excerpts are taken from.
const EXCERPTED: &[&str] = &[
    "MiniMap",
    "Protocol",
    "client",
    "SceneGraph",
    "Player",
    "Camera",
];
const ADAPTERS: &[&str] = &[
    "MinimapRecorder",
    "GlRenderer",
    "GlRaster",
    "SoftwareRaster",
    "BrowserControl",
    "SoftwareFont",
    "GlFont",
    "Sprite",
    "SoftwareSprite",
    "SoftwareAlphaSprite",
    "GlSprite",
    "GlAlphaSprite",
    "IndexedSprite",
    "SoftwareIndexedSprite",
    "GlIndexedSprite",
];
const OTHER: &[&str] = &[
    "reference/client/signlink/src/main/java/rt4/MonotonicClock.java",
    "reference/client/signlink/src/main/java/rt4/FileOnDisk.java",
    "oracle/minimap/jvm/MinimapOracle.java",
];
const TEMPLATE: &str = "oracle/minimap/jvm/MiniMap.java.template";
/// `-Dminimap.seed` (the static initialisers) and the scenario seed.
const SEEDS: [&str; 2] = ["20261116", "530"];
/// Cache files the observations depend on.
const INPUTS: &[&str] = &[
    "main_file_cache.dat2",
    "main_file_cache.idx8",
    "main_file_cache.idx13",
    "main_file_cache.idx23",
    "main_file_cache.idx255",
];

struct Source {
    name: &'static str,
    lines: Vec<String>,
}
impl Source {
    fn read(root: &Path, name: &'static str) -> Result<Self> {
        let text = String::from_utf8(normalized(&fs::read(
            root.join(format!("{JAVA}/{name}.java")),
        )?))?;
        Ok(Self {
            name,
            lines: text.lines().map(str::to_owned).collect(),
        })
    }
    fn find(&self, from: usize, trimmed: &str) -> Result<usize> {
        self.lines[from..]
            .iter()
            .position(|line| line.trim() == trimmed)
            .map(|at| at + from)
            .ok_or_else(|| format!("{}.java: no line `{trimmed}`", self.name).into())
    }
    /// Lines `first..last` (zero based, exclusive end) with their provenance.
    fn block(&self, first: usize, last: usize) -> String {
        format!(
            "// BEGIN {}.java lines {}-{}\n{}\n// END {}.java",
            self.name,
            first + 1,
            last,
            self.lines[first..last].join("\n"),
            self.name
        )
    }
    /// From the line `start` up to, not including, the next line `end`.
    fn between(&self, start: &str, end: &str) -> Result<String> {
        let first = self.find(0, start)?;
        Ok(self.block(first, self.find(first, end)?))
    }
    /// The declarations of these static fields, in source order.
    fn fields(&self, names: &[&str]) -> Result<String> {
        let mut found = Vec::new();
        for name in names {
            let matches: Vec<usize> = self
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| {
                    ["int", "double"].iter().any(|kind| {
                        line.trim()
                            .strip_prefix(&format!("public static {kind} {name}"))
                            .is_some_and(|rest| rest.starts_with(';') || rest.starts_with(" ="))
                    })
                })
                .map(|(at, _)| at)
                .collect();
            let [at] = matches[..] else {
                return Err(
                    format!("{}.java: field {name} is not declared once", self.name).into(),
                );
            };
            found.push(at);
        }
        found.sort_unstable();
        Ok(found
            .into_iter()
            .map(|at| self.block(at, at + 1))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// The generated `rt4.MiniMap`.
fn excerpts(root: &Path) -> Result<String> {
    let minimap = Source::read(root, "MiniMap")?;
    let protocol = Source::read(root, "Protocol")?;
    let client = Source::read(root, "client")?;
    let scene = Source::read(root, "SceneGraph")?;
    let player = Source::read(root, "Player")?;
    let camera = Source::read(root, "Camera")?;
    let mut parts = BTreeMap::new();
    parts.insert(
        "CAMERA_FIELDS",
        camera.fields(&[
            "yawTarget",
            "originY",
            "originX",
            "cameraOffsetX",
            "cameraOffsetY",
            "yawDrift",
            "offsetXDelta",
        ])?,
    );
    parts.insert(
        "MINIMAP_FIELDS",
        minimap.fields(&[
            "zoomOffset",
            "compassAngleOffset",
            "angleJitterStep",
            "zoomJitterStep",
            "jitterTimer",
        ])?,
    );
    parts.insert(
        "PROTOCOL_FIELDS",
        protocol.fields(&["cameraYOffsetDelta", "cameraJitterTimer", "yawDriftDelta"])?,
    );
    parts.insert(
        "SCENE_FIELDS",
        scene.fields(&["lightnessJitter", "hueJitter"])?,
    );
    // The Math.random() assignments of client.resetGameSessionState, in order.
    let first = client.find(0, "public static void resetGameSessionState() {")?;
    let last = first
        + client.lines[first..]
            .iter()
            .position(|line| line.starts_with("\t}"))
            .ok_or("client.java: resetGameSessionState has no end")?;
    let draws: Vec<usize> = (first..last)
        .filter(|&at| client.lines[at].contains("Math.random()"))
        .collect();
    if draws.len() != 6 {
        return Err("client.java: expected six random assignments in resetGameSessionState".into());
    }
    parts.insert(
        "RESET",
        draws
            .into_iter()
            .map(|at| client.block(at, at + 1))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    parts.insert(
        "CYCLE",
        protocol.between("MiniMap.jitterTimer++;", "if (ticksSinceWrite > 50) {")?,
    );
    let first = scene.find(0, "hueJitter += (int) (Math.random() * 5.0D) - 2;")?;
    parts.insert(
        "FLOOR_STEP",
        scene.block(first, scene.find(first, "lightnessJitter = 8;")? + 2),
    );
    let angle = minimap.find(
        0,
        "@Pc(48) int totalAngle = compassAngleOffset + (int) Camera.yawTarget & 0x7FF;",
    )?;
    parts.insert("TOTAL_ANGLE", minimap.block(angle, angle + 1));
    let first = minimap.find(angle, "@Pc(146) int i;")?;
    parts.insert(
        "LOCALS",
        minimap.block(first, minimap.find(first, "@Pc(271) int labelColor;")? + 1),
    );
    parts.insert(
        "LABELS",
        minimap.between(
            "if (LoginManager.mapElementList != null) {",
            "for (i = 0; i < locs; i++) {",
        )?,
    );
    parts.insert(
        "DOTS",
        minimap.between(
            "for (i = 0; i < PlayerList.size; i++) {",
            "@Pc(756) MapMarker[] markers = hintMapMarkers;",
        )?,
    );
    let name = player.find(0, "this.username = Base37.decode37(name37).toTitleCase();")?;
    parts.insert("USERNAME", player.block(name, name + 1));
    let template = String::from_utf8(normalized(&fs::read(root.join(TEMPLATE))?))?;
    let mut out = String::new();
    for line in template.lines() {
        match line
            .strip_prefix("//@@")
            .and_then(|rest| rest.strip_suffix("@@"))
        {
            Some(key) => out.push_str(
                parts
                    .get(key)
                    .ok_or_else(|| format!("template names unknown excerpt {key}"))?,
            ),
            None => out.push_str(line),
        }
        out.push('\n');
    }
    Ok(out)
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    cache_assets::verify(root)?;
    let upstream = crate::verify_reference(root)?;
    let adapters = ADAPTERS
        .iter()
        .map(|name| format!("oracle/minimap/jvm/adapters/rt4/{name}.java"));
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{JAVA}/{name}.java"))
        .chain(adapters)
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    let hashed = sources
        .iter()
        .cloned()
        .chain(EXCERPTED.iter().map(|name| format!("{JAVA}/{name}.java")))
        .chain([TEMPLATE.to_owned()]);
    for source in hashed {
        let hash = format!(
            "{:x}",
            Sha256::digest(normalized(&fs::read(root.join(&source))?))
        );
        hashes.insert(source, hash);
    }
    let build = root.join("runs/minimap-oracle");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes)?;
    }
    fs::create_dir_all(&classes)?;
    let generated = build.join("generated/rt4");
    fs::create_dir_all(&generated)?;
    let excerpt_source = excerpts(root)?;
    fs::write(generated.join("MiniMap.java"), &excerpt_source)?;
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .arg(generated.join("MiniMap.java"))
        .args(&sources))?;
    let cache = cache_assets::directory(root);
    let output = run(Command::new("java")
        .current_dir(root)
        .arg(format!("-Dminimap.seed={}", SEEDS[0]))
        .arg("-cp")
        .arg(&classes)
        .arg("MinimapOracle")
        .arg(&cache)
        .arg(SEEDS[1]))?;
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
        "generatedSha256": format!("{:x}", Sha256::digest(excerpt_source.as_bytes())),
        "inputs": inputs,
        "seeds": SEEDS,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "Statement excerpts copied verbatim into a generated rt4.MiniMap: Protocol's camera and minimap drift update, the six random assignments of client.resetGameSessionState, SceneGraph's floor jitter initialisers and per-rebuild step, Player.decodeAppearance's username assignment, and MiniMap.render's label loop and player dot loop. Math.random() is a seeded java.util.Random through a nested Math. Font, JagString, Base37, MapElementList, MathUtils, SpriteLoader, ClanMember and the cache classes are unchanged and read the pinned cache. SoftwareFont records each glyph call instead of rasterising; the raster, sprite and browser classes are empty adapters, and the statics the excerpts read are nested stand-ins. No pixels, masks or clipping are observed."
    });
    let directory = root.join("fixtures/minimap");
    let (fixture_path, provenance_path) = (
        directory.join("drift-dots-labels.jsonl"),
        directory.join("drift-dots-labels.provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "minimap oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err(
                "minimap oracle provenance is stale; review source or input changes".into(),
            );
        }
        println!("{total} JVM minimap observations reproduced with matching sources and inputs.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM minimap observations: {counts:?}");
    }
    Ok(())
}
