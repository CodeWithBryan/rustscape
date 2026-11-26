//! `cargo xtask roof-oracle [--check]`: the original roof fill run over scenes
//! of the pinned cache; its observations become `fixtures/roofs`.
//!
//! The roof methods live in two very large classes that cannot be compiled
//! outside the client. The generator copies those methods, unchanged, out of
//! the pinned sources into holder classes (see `oracle/roofs/*.java.in`) and
//! compiles them with the unchanged `Tile`, `Wall`, `Scenery`, `Node` and
//! `Buffer` and a few field-only adapters.
use crate::{Result, cache_assets, normalized, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_client::roof::{self, Camera, Mode, Placement, Roofs, Scene};
use rs09_server::world::scene::{Assets, CacheAssets};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
    fs,
    path::Path,
    process::Command,
};

const RT4: &str = "reference/client/client/src/main/java/rt4";
/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &["Tile", "Wall", "Scenery", "Node", "Buffer"];
const OTHER: &[&str] = &[
    "oracle/adapters/rt4/JagString.java",
    "oracle/roofs/adapters/plugin/api/API.java",
    "oracle/roofs/adapters/rt4/Camera.java",
    "oracle/roofs/adapters/rt4/CollisionMap.java",
    "oracle/roofs/adapters/rt4/Entity.java",
    "oracle/roofs/adapters/rt4/GlRenderer.java",
    "oracle/roofs/adapters/rt4/GroundDecor.java",
    "oracle/roofs/adapters/rt4/ObjStackEntity.java",
    "oracle/roofs/adapters/rt4/PathFinder.java",
    "oracle/roofs/adapters/rt4/PlainTile.java",
    "oracle/roofs/adapters/rt4/Player.java",
    "oracle/roofs/adapters/rt4/PlayerList.java",
    "oracle/roofs/adapters/rt4/Preferences.java",
    "oracle/roofs/adapters/rt4/RoofLoc.java",
    "oracle/roofs/adapters/rt4/ShapedTile.java",
    "oracle/roofs/adapters/rt4/WallDecor.java",
    "oracle/roofs/RoofOracle.java",
];
/// Files the holders are cut from, or that the adapters and the Rust port
/// transcribe by hand.
const READ: &[&str] = &[
    "ScriptRunner",
    "SceneGraph",
    "LoginManager",
    "LocType",
    "GlTile",
];

struct Holder {
    name: &'static str,
    /// Field declarations copied from the class of the same name.
    fields: &'static [&'static str],
    /// Declarations copied from another class.
    borrowed: &'static [(&'static str, &'static str)],
    /// The start of each method's declaration line.
    methods: &'static [&'static str],
}
const HOLDERS: [Holder; 2] = [
    Holder {
        name: "ScriptRunner",
        fields: &[
            "public static int renderCycle = 0;",
            "public static boolean neverRemoveRoofs = false;",
            "public static byte[][][] roofVisibility;",
            "public static int[] roofGroupMinX = new int[8];",
            "public static int[] roofGroupMaxX = new int[8];",
            "public static int[] roofGroupMinZ = new int[8];",
            "public static int[] roofGroupMaxZ = new int[8];",
            "public static int[] roofGroupMaxHeight = new int[8];",
        ],
        borrowed: &[],
        methods: &[
            "public static void updateRoofRemovalMode() {",
            "public static int getRoofRemovalMode() {",
            "private static int getBaseRoofMode() {",
            "public static void fillRoofVisibility(",
            "public static void allocateRoofVisibilityGroupArrays(",
            "public static void buildAllRoofVisibilityGroups() {",
            "public static boolean floodFillRoofVisibilityGroup(",
            "private static boolean floodFillRoofVisibilityGroup(",
            "public static void updateRoofVisibilityGroups() {",
        ],
    },
    Holder {
        name: "SceneGraph",
        fields: &[
            "public static final byte[][][] renderFlags = new byte[4][104][104];",
            "public static final int[] WALL_ROTATION_TYPE1 = new int[]{1, 2, 4, 8};",
            "public static Tile[][][] tiles;",
            "public static int[][][] tileHeights;",
            "public static int[][][] underwaterTileHeights;",
            "public static int width;",
            "public static int length;",
            "public static int sceneryLen = 0;",
            "public static Scenery[] scenery;",
            "public static int[][] tileWaterData;",
        ],
        borrowed: &[(
            "LoginManager",
            "public static final int[] WALL_ROTATION_FLAGS = new int[]{16, 32, 64, 128};",
        )],
        methods: &[
            "public static boolean allLevelsAreVisible() {",
            "public static int getTileHeight(",
            "public static int getLocCollisionMask(",
            "public static void setWall(",
            "public static void bridgeTile(",
            "public static boolean addSceneryEntity(",
            "public static boolean addEntity(",
            "public static void readLocs(",
        ],
    },
];

/// The scenes, by the south-west tile of their 104 by 104 window.
const SCENES: [(&str, [i32; 2]); 3] = [
    // Lumbridge: the castle, its upper floors and the bridge over the Lum.
    ("lumbridge", [3168, 3168]),
    // Varrock: the palace and a town of two- and three-storey houses.
    ("varrock", [3160, 3376]),
    // Falador: the White Knights' castle, with walkways between its towers.
    ("falador", [2912, 3328]),
];

fn source(root: &Path, class: &str) -> Result<String> {
    Ok(String::from_utf8(normalized(&fs::read(
        root.join(format!("{RT4}/{class}.java")),
    )?))?)
}
fn declared(text: &str, class: &str, line: &str) -> Result<()> {
    if text.lines().filter(|l| l.trim() == line).count() == 1 {
        Ok(())
    } else {
        Err(format!("{class}.java no longer declares `{line}` exactly once").into())
    }
}
/// The whole method whose declaration line starts with `start`, as written.
fn method(text: &str, class: &str, start: &str) -> Result<String> {
    let lines: Vec<&str> = text.lines().collect();
    let found: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim_start().starts_with(start))
        .collect();
    let [first] = found[..] else {
        return Err(format!("{class}.java: `{start}` matches {} lines", found.len()).into());
    };
    let mut depth = 0i32;
    let mut out = String::new();
    for line in &lines[first..] {
        out.push_str(line);
        out.push('\n');
        if line.contains('"') || line.contains('\'') {
            return Err(format!("{class}.java: `{start}` has a literal; braces unsafe").into());
        }
        depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
        if depth == 0 {
            return Ok(out);
        }
    }
    Err(format!("{class}.java: `{start}` does not close").into())
}
fn holder(root: &Path, holder: &Holder) -> Result<(String, Vec<String>)> {
    let text = source(root, holder.name)?;
    let mut fields = String::new();
    for line in holder.fields {
        declared(&text, holder.name, line)?;
        writeln!(fields, "\t{line}")?;
    }
    for (class, line) in holder.borrowed {
        declared(&source(root, class)?, class, line)?;
        writeln!(fields, "\t{line}")?;
    }
    let mut methods = String::new();
    let mut copied = Vec::new();
    for start in holder.methods {
        let body = method(&text, holder.name, start)?;
        copied.push(format!("{:x}", Sha256::digest(body.as_bytes())));
        methods.push_str(&body);
        methods.push('\n');
    }
    let template = String::from_utf8(normalized(&fs::read(
        root.join(format!("oracle/roofs/{}.java.in", holder.name)),
    )?))?;
    if !template.contains("// @FIELDS@\n") || !template.contains("// @METHODS@\n") {
        return Err("holder template lost a marker".into());
    }
    Ok((
        template
            .replace("// @FIELDS@\n", &fields)
            .replace("// @METHODS@\n", &methods),
        copied,
    ))
}

struct Facts {
    size: [i32; 2],
    undrawn: bool,
    animated: bool,
    models: usize,
    shapes: Option<Vec<i32>>,
}
struct Prepared {
    name: &'static str,
    base: [i32; 2],
    flags: Vec<u8>,
    heights: Vec<i32>,
    /// `[plane, x, y, shape, rotation, width, length, entity]` in file order.
    locs: Vec<[i32; 8]>,
    scene: Scene,
    /// Roofed tiles of each plane.
    roofed: [Vec<[i32; 2]>; 4],
    input: String,
}

/// `SceneGraph.readLocs`' stream, without its bounds: `[id, x, y, plane,
/// shape, rotation]` with region-local tiles.
fn placements(bytes: &[u8]) -> Result<Vec<[i32; 6]>> {
    let mut r = rs09_protocol::buffer::Reader::new(bytes);
    let mut out = Vec::new();
    let mut id = -1i32;
    loop {
        let offset = r.g_var_smart()?;
        if offset == 0 {
            return Ok(out);
        }
        id += offset;
        let mut position = 0;
        loop {
            let offset = r.gsmarts()?;
            if offset == 0 {
                break;
            }
            position += offset - 1;
            let flags = r.g1()?;
            out.push([
                id,
                position >> 6 & 63,
                position & 63,
                position >> 12,
                flags >> 2,
                flags & 3,
            ]);
        }
    }
}
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}

fn prepare(
    assets: &CacheAssets,
    definitions: &Archive,
    name: &'static str,
    base: [i32; 2],
) -> Result<Prepared> {
    use rs09_client::terrain::{Region, Tile};
    let mut regions = Vec::new();
    let mut lands = Vec::new();
    // LoginManager's region order: x outside, y inside.
    for rx in base[0] >> 6..=(base[0] + 103) >> 6 {
        for ry in base[1] >> 6..=(base[1] + 103) >> 6 {
            let map = assets.archive_id(&format!("m{rx}_{ry}"))?;
            let land = assets.archive_id(&format!("l{rx}_{ry}"))?;
            if map < 0 {
                continue;
            }
            let bytes = assets.mapscape(map)?.ok_or("terrain file missing")?;
            regions.push(Region::decode(rx, ry, &bytes)?);
            if land >= 0 {
                let bytes = assets
                    .landscape(land, assets.keys(rx << 8 | ry)?)?
                    .ok_or_else(|| format!("no location file for region {rx}_{ry}"))?;
                lands.push(([rx, ry], bytes));
            }
        }
    }
    let tile = |plane: usize, x: usize, y: usize| {
        let (wx, wy) = (base[0] + x as i32, base[1] + y as i32);
        regions
            .iter()
            .find(|r| r.x == wx >> 6 && r.z == wy >> 6)
            .map_or(Tile::default(), |r| {
                r.tile(plane, (wx & 63) as usize, (wy & 63) as usize)
            })
    };
    let scene = Scene::new(
        |p, x, y| tile(p, x, y).flags,
        |p, x, y| tile(p, x, y).height,
    );
    assemble(name, base, scene, lands, |id| {
        let raw = definitions.file(id as u32 >> 8, id as u32 & 255, [0; 4])?;
        Ok(match raw {
            Some(raw) => {
                let (kind, visual, _) =
                    rs09_client::interaction::LocationType::decode_with_visual(&raw)?;
                Facts {
                    size: [visual.width, visual.length],
                    undrawn: kind.placement.hidden_in_hd,
                    animated: ![-1, 65535].contains(&visual.animation)
                        || kind.children.is_some()
                        || kind.placement.dynamic,
                    models: visual.models.len(),
                    shapes: visual.shapes,
                }
            }
            // LocTypeList.get of an absent file: an undecoded type.
            None => Facts {
                size: [1, 1],
                undrawn: false,
                animated: false,
                models: 0,
                shapes: None,
            },
        })
    })
}

/// The oracle's input for a scene, and the same scene built by the Rust port.
fn assemble(
    name: &'static str,
    base: [i32; 2],
    mut scene: Scene,
    lands: Vec<([i32; 2], Vec<u8>)>,
    mut describe: impl FnMut(i32) -> Result<Facts>,
) -> Result<Prepared> {
    let mut flags = Vec::new();
    let mut heights = Vec::new();
    let mut roofed: [Vec<[i32; 2]>; 4] = Default::default();
    #[allow(clippy::needless_range_loop)]
    for plane in 0..4 {
        for x in 0..104 {
            for y in 0..104 {
                flags.push(scene.flag(plane, x, y));
                if scene.flag(plane, x, y) & 4 != 0 {
                    roofed[plane].push([x as i32, y as i32]);
                }
            }
        }
        for x in 0..=104 {
            for y in 0..=104 {
                heights.push(scene.height(plane, x, y));
            }
        }
    }
    let mut input = format!("scene {name}\nflags {}\nheights", hex(&flags));
    for height in &heights {
        write!(input, " {height}")?;
    }
    input.push('\n');
    let mut facts = BTreeMap::<i32, Facts>::new();
    let mut locs = Vec::new();
    for (region, bytes) in &lands {
        for [id, x, y, plane, shape, rotation] in placements(bytes)? {
            if let std::collections::btree_map::Entry::Vacant(entry) = facts.entry(id) {
                entry.insert(describe(id)?);
            }
            let f = &facts[&id];
            let entity = roof::has_entity(
                shape as u8,
                f.shapes.as_deref(),
                f.models,
                f.animated,
                f.undrawn,
            );
            let tile = [region[0] * 64 + x - base[0], region[1] * 64 + y - base[1]];
            if plane > 3 {
                // readLocs would index past the scene's four planes.
                return Err("location above plane 3".into());
            }
            locs.push([
                plane,
                tile[0],
                tile[1],
                shape,
                rotation,
                f.size[0],
                f.size[1],
                i32::from(entity),
            ]);
            scene.add(Placement {
                plane: plane as usize,
                tile,
                shape: shape as u8,
                rotation: rotation as u8,
                size: f.size,
                entity,
            });
        }
    }
    scene.bridge();
    for (id, f) in &facts {
        write!(
            input,
            "loc {id} {} {} {} {} {}",
            f.size[0],
            f.size[1],
            u8::from(f.undrawn),
            u8::from(f.animated),
            f.models
        )?;
        match &f.shapes {
            None => input.push_str(" -1"),
            Some(shapes) => {
                write!(input, " {}", shapes.len())?;
                for shape in shapes {
                    write!(input, " {shape}")?;
                }
            }
        }
        input.push('\n');
    }
    for (region, bytes) in &lands {
        writeln!(
            input,
            "land {} {} {}",
            region[0] * 64 - base[0],
            region[1] * 64 - base[1],
            hex(bytes)
        )?;
    }
    input.push_str("build\n");
    Ok(Prepared {
        name,
        base,
        flags,
        heights,
        locs,
        scene,
        roofed,
        input,
    })
}

/// A made-up scene for what the map data seldom or never holds: roof edges of
/// every shape and rotation beside roofed tiles, corner walls on upper planes,
/// tiles offered more than five pieces of scenery, large scenery across the
/// scene's edge and across bridge columns, forced-ground flags above roofs,
/// location types without a model for the placed shape, animated and
/// undrawn types, and shapes outside the usual 0 to 22. Flags, heights and
/// types are invented; the locations travel through a location file written
/// here and decoded by the original `readLocs`.
fn synthetic() -> Result<Prepared> {
    let mut random = Random(530);
    let at = |plane: usize, x: usize, y: usize| plane * 10816 + x * 104 + y;
    let mut flags = vec![0u8; 4 * 10816];
    for (plane, count) in [(0, 34), (1, 20), (2, 12)] {
        for _ in 0..count {
            let (width, length) = (2 + random.next(9), 2 + random.next(9));
            let (x0, y0) = (random.next(104 - width), random.next(104 - length));
            for x in x0..x0 + width {
                for y in y0..y0 + length {
                    flags[at(plane, x as usize, y as usize)] |= 4;
                }
            }
        }
    }
    for _ in 0..7 {
        let (x0, y) = (random.next(96), random.next(104));
        for x in x0..x0 + 8 {
            flags[at(1, x as usize, y as usize)] |= 2;
        }
    }
    for plane in 0..4 {
        for x in 0..104 {
            for y in 0..104 {
                let flag = &mut flags[at(plane, x, y)];
                if plane > 0 && random.next(30) == 0 {
                    *flag |= 8;
                }
                if random.next(20) == 0 {
                    *flag |= 1;
                }
                if random.next(25) == 0 {
                    *flag |= 16;
                }
            }
        }
    }
    let heights: Vec<i32> = (0..4 * 105 * 105)
        .map(|i| -240 * (i / 11025) - random.next(64))
        .collect();
    let scene = Scene::new(
        |plane, x, y| flags[at(plane, x, y)],
        |plane, x, y| heights[plane * 11025 + x * 105 + y],
    );
    let types: Vec<Facts> = (0..80)
        .map(|_| Facts {
            size: [1 + random.next(4), 1 + random.next(4)],
            undrawn: random.next(12) == 0,
            animated: random.next(7) == 0,
            models: random.next(3) as usize,
            shapes: (random.next(3) != 0)
                .then(|| (0..1 + random.next(12)).map(|_| random.next(23)).collect()),
        })
        .collect();
    const SHAPES: [i32; 36] = [
        0, 0, 1, 2, 2, 3, 9, 10, 10, 10, 11, 11, 12, 13, 14, 15, 16, 17, 18, 18, 18, 19, 19, 19,
        20, 21, 21, 21, 4, 5, 6, 7, 8, 22, 23, 30,
    ];
    // [region, id, plane << 12 | x << 6 | y, shape << 2 | rotation]
    let mut placed = Vec::new();
    let mut place = |random: &mut Random, plane: i32, x: i32, y: i32, shape: i32| {
        placed.push([
            (x >> 6) * 2 + (y >> 6),
            random.next(80),
            plane << 12 | (x & 63) << 6 | (y & 63),
            shape << 2 | random.next(4),
        ]);
    };
    for _ in 0..9000 {
        let shape = SHAPES[random.next(36) as usize];
        let (plane, x, y) = (random.next(4), random.next(104), random.next(104));
        place(&mut random, plane, x, y, shape);
    }
    for _ in 0..60 {
        let (plane, x, y) = (random.next(4), random.next(104), random.next(104));
        for _ in 0..8 {
            let shape = [10, 11, 12, 18, 19, 21, 9][random.next(7) as usize];
            place(&mut random, plane, x, y, shape);
        }
    }
    placed.sort();
    let smart = |out: &mut Vec<u8>, value: i32| {
        if value < 128 {
            out.push(value as u8);
        } else {
            out.extend(((value + 32768) as u16).to_be_bytes());
        }
    };
    let mut lands = Vec::new();
    for region in 0..4 {
        let mut bytes = Vec::new();
        let mut previous = -1;
        let mut position = 0;
        let mut open = false;
        for &[_, id, packed, kind] in placed.iter().filter(|p| p[0] == region) {
            if id != previous {
                if open {
                    smart(&mut bytes, 0);
                }
                smart(&mut bytes, id - previous);
                (previous, position, open) = (id, 0, true);
            }
            smart(&mut bytes, packed - position + 1);
            bytes.push(kind as u8);
            position = packed;
        }
        if open {
            smart(&mut bytes, 0);
        }
        smart(&mut bytes, 0);
        lands.push(([region / 2, region % 2], bytes));
    }
    assemble("synthetic", [0, 0], scene, lands, |id| {
        let f = &types[id as usize];
        Ok(Facts {
            size: f.size,
            undrawn: f.undrawn,
            animated: f.animated,
            models: f.models,
            shapes: f.shapes.clone(),
        })
    })
}

/// One call of the original's per-frame entry, with the state it reads.
#[derive(Clone, Copy)]
struct Step {
    update: bool,
    never: bool,
    selective: bool,
    plane: i32,
    player: [i32; 2],
    camera: Camera,
    cycle: i32,
}
struct Random(u64);
impl Random {
    fn next(&mut self, bound: i32) -> i32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % bound as u64) as i32
    }
}
/// A follow camera somewhere around the player, inside the scene, and never
/// on the player's own tile below pitch 310 (the original divides by zero).
fn orbit(scene: &Scene, random: &mut Random, plane: i32, player: [i32; 2]) -> Camera {
    let mut pitch = 128 + random.next(256);
    let distance = if random.next(3) == 0 {
        f64::from(200 + random.next(4000))
    } else {
        // Camera.ZOOM + pitch * 3 at the default zoom.
        f64::from(600 + pitch * 3)
    };
    let yaw = f64::from(random.next(2048)) * std::f64::consts::TAU / 2048.0;
    let tilt = f64::from(pitch) * std::f64::consts::TAU / 2048.0;
    let limit = 104 * 128 - 1;
    let x = (f64::from(player[0]) + yaw.sin() * tilt.cos() * distance) as i32;
    let y = (f64::from(player[1]) - yaw.cos() * tilt.cos() * distance) as i32;
    let (x, y) = (x.clamp(0, limit), y.clamp(0, limit));
    let z = scene.tile_height(plane, player) - 50 - (tilt.sin() * distance) as i32;
    if x >> 7 == player[0] >> 7 && y >> 7 == player[1] >> 7 && pitch < 310 {
        pitch = 310 + random.next(74);
    }
    Camera {
        follow: true,
        position: [x, y, z],
        pitch,
    }
}
fn steps(p: &Prepared) -> Vec<Step> {
    let mut random = Random(
        p.name
            .bytes()
            .fold(20261116u64, |n, b| n.wrapping_mul(131) + u64::from(b)),
    );
    let mut out = Vec::new();
    let mut cycle = 250;
    let fine = |random: &mut Random, tile: [i32; 2]| {
        [
            tile[0].clamp(1, 102) * 128 + random.next(128),
            tile[1].clamp(1, 102) * 128 + random.next(128),
        ]
    };
    let place = |random: &mut Random, plane: usize| -> [i32; 2] {
        let roofed = &p.roofed[plane];
        match random.next(4) {
            0 => [8 + random.next(88), 8 + random.next(88)],
            // Beside or just outside a roofed tile.
            1 if !roofed.is_empty() => {
                let [x, y] = roofed[random.next(roofed.len() as i32) as usize];
                [x + random.next(5) - 2, y + random.next(5) - 2]
            }
            _ if !roofed.is_empty() => roofed[random.next(roofed.len() as i32) as usize],
            _ => [8 + random.next(88), 8 + random.next(88)],
        }
    };
    // The default mode from every plane; a rebuild or plane change now and then.
    for (plane, count) in [(0, 160), (1, 90), (2, 60), (3, 8)] {
        for i in 0..count {
            let tile = place(&mut random, plane as usize);
            let player = fine(&mut random, tile);
            cycle += 1;
            out.push(Step {
                update: i % 37 == 0,
                never: false,
                selective: true,
                plane,
                player,
                camera: orbit(&p.scene, &mut random, plane, player),
                cycle,
            });
        }
    }
    // Walking: consecutive frames one fine step apart, through a roofed area.
    for plane in 0..2usize {
        if let Some(&start) = p.roofed[plane].get(p.roofed[plane].len() / 2) {
            let mut player = [(start[0] - 12).max(1) * 128, start[1] * 128 + 64];
            let camera_offset = [900 - random.next(1800), -1200];
            for i in 0..70 {
                player[0] += 48;
                cycle += 1;
                let limit = 104 * 128 - 1;
                out.push(Step {
                    update: i == 0,
                    never: false,
                    selective: true,
                    plane: plane as i32,
                    player,
                    camera: Camera {
                        follow: true,
                        position: [
                            (player[0] + camera_offset[0]).clamp(0, limit),
                            (player[1] + camera_offset[1]).clamp(0, limit),
                            p.scene.tile_height(plane as i32, player) - 900,
                        ],
                        pitch: 200 + i % 120,
                    },
                    cycle,
                });
            }
        }
    }
    // A camera that does not follow the player, either side of the 800 limit.
    for i in 0..90 {
        let plane = (i % 3) as usize;
        let tile = place(&mut random, plane);
        let position = fine(&mut random, tile);
        let other = place(&mut random, plane);
        cycle += 1;
        out.push(Step {
            update: i == 0,
            never: false,
            selective: true,
            plane: plane as i32,
            player: fine(&mut random, other),
            camera: Camera {
                follow: false,
                position: [
                    position[0],
                    position[1],
                    p.scene.tile_height(plane as i32, position)
                        - match i % 6 {
                            0 => 799,
                            1 => 800,
                            2 => 801,
                            _ => random.next(1600),
                        },
                ],
                pitch: 128 + random.next(256),
            },
            cycle,
        });
    }
    // Marks 256 frames old alias this frame's value except in the one column
    // each frame resets: no update in between.
    for i in 0..12 {
        let tile = place(&mut random, 0);
        let player = fine(&mut random, tile);
        cycle += if i % 2 == 1 { 256 } else { 1 };
        out.push(Step {
            update: i == 0,
            never: false,
            selective: true,
            plane: 0,
            player,
            camera: orbit(&p.scene, &mut random, 0, player),
            cycle,
        });
    }
    // "Remove roofs: all": the groups are built on the update, with the plane
    // of that moment, and later frames and plane changes leave them alone.
    for plane in [0, 1, 2, 3, 0] {
        for i in 0..4 {
            let tile = place(&mut random, 0);
            let player = fine(&mut random, tile);
            cycle += 1;
            out.push(Step {
                update: i == 0,
                never: false,
                selective: false,
                plane: if i == 3 { (plane + 1) % 4 } else { plane },
                player,
                camera: orbit(&p.scene, &mut random, plane, player),
                cycle,
            });
        }
    }
    // Preference changes, each followed by the update the original makes.
    let mut previous = (false, false);
    for i in 0..60 {
        let mode = match random.next(5) {
            0 => (true, random.next(2) == 0),
            1 => (false, false),
            _ => (false, true),
        };
        let plane = random.next(3);
        let tile = place(&mut random, plane as usize);
        let player = fine(&mut random, tile);
        cycle += 1 + random.next(3);
        out.push(Step {
            update: i == 0 || mode != previous || random.next(6) == 0,
            never: mode.0,
            selective: mode.1,
            plane,
            player,
            camera: orbit(&p.scene, &mut random, plane, player),
            cycle,
        });
        previous = mode;
    }
    out
}
fn fnv(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:x}")
}

pub fn generate(root: &Path, check: bool) -> Result<()> {
    cache_assets::verify(root)?;
    let upstream = crate::verify_reference(root)?;
    let build = root.join("runs/roof-oracle");
    let classes = build.join("classes");
    let generated = build.join("generated/rt4");
    for directory in [&classes, &generated] {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
        fs::create_dir_all(directory)?;
    }
    let mut hashes = BTreeMap::new();
    let mut copied = BTreeMap::new();
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("{RT4}/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    for path in sources
        .iter()
        .cloned()
        .chain(READ.iter().map(|name| format!("{RT4}/{name}.java")))
        .chain(
            HOLDERS
                .iter()
                .map(|h| format!("oracle/roofs/{}.java.in", h.name)),
        )
    {
        let digest = Sha256::digest(normalized(&fs::read(root.join(&path))?));
        hashes.insert(path, format!("{digest:x}"));
    }
    for h in &HOLDERS {
        let (text, methods) = holder(root, h)?;
        fs::write(generated.join(format!("{}.java", h.name)), text)?;
        sources.push(format!("runs/roof-oracle/generated/rt4/{}.java", h.name));
        for (start, digest) in h.methods.iter().zip(methods) {
            copied.insert(format!("{}: {start}", h.name), digest);
        }
    }
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources))?;

    let keys_source = crate::git(
        &root.join("reference/server"),
        &[
            "show",
            &format!(
                "{}:Server/data/configs/xteas.json",
                upstream["server"]["commit"]
                    .as_str()
                    .ok_or("missing server pin")?
            ),
        ],
    )?;
    let rows: Value = serde_json::from_str(&keys_source)?;
    let mut keys = HashMap::new();
    for row in rows["xteas"].as_array().ok_or("missing pinned XTEA rows")? {
        let id: i32 = row["regionId"]
            .as_str()
            .ok_or("missing XTEA region")?
            .parse()?;
        let values: Vec<_> = row["keys"]
            .as_str()
            .ok_or("missing XTEA keys")?
            .split(',')
            .map(str::parse::<i32>)
            .collect::<std::result::Result<_, _>>()?;
        let values: [i32; 4] = values.try_into().map_err(|_| "XTEA key length differs")?;
        keys.insert(id, values.map(|n| n as u32));
    }
    let limits = Limits::default();
    let store = Store::open(
        &cache_assets::directory(root),
        StorageMode::Resident,
        limits,
    )?;
    let definitions = Archive::open(store.clone(), 16, None, limits)?;
    let assets = CacheAssets {
        archive: Archive::open(store, 5, None, limits)?,
        keys,
    };

    let mut input = String::new();
    let mut scenes = Vec::new();
    let mut prepared = SCENES
        .iter()
        .map(|&(name, base)| prepare(&assets, &definitions, name, base))
        .collect::<Result<Vec<_>>>()?;
    prepared.push(synthetic()?);
    for prepared in prepared {
        let list = steps(&prepared);
        input.push_str(&prepared.input);
        for s in &list {
            let [x, y, z] = s.camera.position;
            writeln!(
                input,
                "step {} {} {} {} {} {} {} {x} {y} {z} {} {}",
                u8::from(s.update),
                u8::from(s.never),
                u8::from(s.selective),
                s.plane,
                s.player[0],
                s.player[1],
                u8::from(s.camera.follow),
                s.camera.pitch,
                s.cycle
            )?;
        }
        scenes.push((prepared, list));
    }
    let input_path = build.join("input.txt");
    fs::write(&input_path, &input)?;
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("RoofOracle")
        .arg(&input_path))?;
    let output = String::from_utf8(normalized(&output.stdout))?;
    fs::write(build.join("output.txt"), &output)?;

    let mut lines = output.lines();
    let mut fixture = String::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for (p, list) in &scenes {
        if lines.next() != Some(&format!("scene {}", p.name)) {
            return Err("oracle output lost a scene".into());
        }
        let mut original = String::new();
        let total = loop {
            let line = lines.next().ok_or("oracle output ended in a tile dump")?;
            if let Some(count) = line.strip_prefix("tiles ") {
                break count.parse::<usize>()?;
            }
            original.push_str(line);
            original.push('\n');
        };
        let native = p.scene.dump();
        if native != original {
            fs::write(build.join(format!("{}-native-tiles.txt", p.name)), &native)?;
            fs::write(
                build.join(format!("{}-original-tiles.txt", p.name)),
                &original,
            )?;
            return Err(format!(
                "{}: the Rust tile array differs from the original's; see runs/roof-oracle",
                p.name
            )
            .into());
        }
        *counts.entry("scene".into()).or_default() += 1;
        writeln!(
            fixture,
            "{}",
            json!({
                "case": "scene", "name": p.name, "base": p.base,
                "flags": hex(&p.flags), "heights": p.heights, "locs": p.locs,
                "tiles": total,
                "tilesSha256": format!("{:x}", Sha256::digest(original.as_bytes())),
            })
        )?;
        let mut roofs = Roofs::default();
        for (number, s) in list.iter().enumerate() {
            let line = lines.next().ok_or("oracle output ended early")?;
            let mut t = line.split(' ');
            if t.next() != Some("step") {
                return Err(format!("unexpected oracle line: {line}").into());
            }
            let marks = t.next().ok_or("marks")?;
            if t.next() != Some("g") {
                return Err("oracle step lost its groups".into());
            }
            let length: usize = t.next().ok_or("groups")?.parse()?;
            let mut groups = Vec::new();
            for _ in 0..5 {
                let mut values = Vec::new();
                for _ in 0..length {
                    values.push(t.next().ok_or("group value")?.parse::<i32>()?);
                }
                groups.push(values);
            }
            if t.next() != Some("h") {
                return Err("oracle step lost its hidden tiles".into());
            }
            let count: usize = t.next().ok_or("hidden")?.parse()?;
            let hidden: Vec<u32> = t.map(str::parse).collect::<std::result::Result<_, _>>()?;
            if hidden.len() != count {
                return Err("oracle hidden tile count differs".into());
            }

            let mode = Mode::new(s.never, s.selective);
            roofs.cycle = s.cycle;
            if s.update {
                roofs.update_mode(mode, &p.scene, s.plane);
            }
            roofs.frame(mode, &p.scene, s.plane, s.player, &s.camera);
            let native_marks = roofs
                .marks()
                .map_or("null".to_owned(), |m| fnv(m.iter().copied()));
            let native_hidden: Vec<u32> = roofs
                .mask(mode, s.plane)
                .tiles()
                .into_iter()
                .map(|[level, x, y]| (level * 10816 + x * 104 + y) as u32)
                .collect();
            let g = &roofs.groups;
            let native_groups = vec![
                g.max_height.clone(),
                g.min_x.clone(),
                g.max_x.clone(),
                g.min_z.clone(),
                g.max_z.clone(),
            ];
            if native_hidden != hidden || native_marks != marks || native_groups != groups {
                return Err(format!(
                    "{} step {number}: Rust differs from the original.\n hidden {} vs {}\n marks {native_marks} vs {marks}\n only Rust: {:?}\n only original: {:?}\n groups {native_groups:?}\n     vs {groups:?}",
                    p.name,
                    native_hidden.len(),
                    hidden.len(),
                    native_hidden.iter().filter(|n| !hidden.contains(n)).take(20).collect::<Vec<_>>(),
                    hidden.iter().filter(|n| !native_hidden.contains(n)).take(20).collect::<Vec<_>>(),
                )
                .into());
            }
            *counts.entry("step".into()).or_default() += 1;
            let key = match (mode, s.camera.follow, hidden.is_empty()) {
                (Mode::Never, ..) => "stepNever",
                (Mode::All, ..) => "stepAll",
                (Mode::Selective, false, _) => "stepFreeCamera",
                (Mode::Selective, true, true) => "stepNothingHidden",
                (Mode::Selective, true, false) => "stepRoofsHidden",
            };
            *counts.entry(key.into()).or_default() += 1;
            writeln!(
                fixture,
                "{}",
                json!({
                    "case": "step", "scene": p.name, "update": s.update,
                    "never": s.never, "selective": s.selective, "plane": s.plane,
                    "player": s.player, "follow": s.camera.follow,
                    "camera": s.camera.position, "pitch": s.camera.pitch, "cycle": s.cycle,
                    "marks": marks, "groups": groups, "hidden": hidden.len(),
                    "hiddenHash": fnv(hidden.iter().flat_map(|n| n.to_le_bytes())),
                })
            )?;
        }
    }
    if lines.next().is_some() {
        return Err("oracle printed more than was asked".into());
    }
    let fixture = fixture.into_bytes();
    let mut inputs = BTreeMap::new();
    for name in [
        "main_file_cache.dat2",
        "main_file_cache.idx5",
        "main_file_cache.idx16",
        "main_file_cache.idx255",
    ] {
        let mut hasher = Sha256::new();
        std::io::copy(
            &mut fs::File::open(cache_assets::directory(root).join(name))?,
            &mut hasher,
        )?;
        inputs.insert(name, format!("{:x}", hasher.finalize()));
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "copiedMethodsSha256": copied,
        "inputs": inputs,
        "keySourceSha256": format!("{:x}", Sha256::digest(normalized(keys_source.as_bytes()))),
        "inputSha256": format!("{:x}", Sha256::digest(input.as_bytes())),
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "ScriptRunner.updateRoofRemovalMode, getRoofRemovalMode, fillRoofVisibility, allocateRoofVisibilityGroupArrays, buildAllRoofVisibilityGroups, floodFillRoofVisibilityGroup and updateRoofVisibilityGroups, and SceneGraph.getTileHeight, getLocCollisionMask, setWall, addEntity, addSceneryEntity, bridgeTile and readLocs, copied unchanged out of the pinned sources into holder classes and run with the unchanged Tile, Wall, Scenery, Node and Buffer over three scenes of the pinned cache and one invented scene (random roofed blocks, bridge strips, forced-ground flags and 9480 locations of every shape on every plane, written into location files that the original readLocs decodes). Observed per step: the whole roofVisibility array (as a hash), the five roof group arrays and the tiles setShapedTile would hide; per scene: every wall and scenery link of the tile array. NOT the original: render flags, tile heights and each location type's size, shapes, model count, animation, alternatives and opcodes 82 and 98 come from the Rust decoders; SceneGraph.addLoc is replaced by a hand transcription of its setWall/addEntity decisions (SceneGraph.java.in, RoofLoc.java); the hidden test of setShapedTile and the roofCycle of the scene draw are transcribed in RoofOracle.java; the plugin API is its no-handler state. Inputs on which the original throws (a camera outside the scene, or on the player's tile below pitch 310) are not generated. Dynamic location changes, instanced scenes and the renderer's use of the result are not observed."
    });
    let directory = root.join("fixtures/roofs");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts["scene"] + counts["step"];
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "roof oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("roof oracle provenance is stale; review source or input changes".into());
        }
        println!("{total} JVM roof observations reproduced with matching sources and inputs.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM roof observations: {counts:?}");
    }
    Ok(())
}
