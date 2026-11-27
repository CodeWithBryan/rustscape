//! `cargo xtask census`: the refusal census (docs/refusal-census.md).
//!
//! - `tours [--check] [--data DIR]` writes the scripted tours in
//!   `crates/headless/scenarios/census/` from the pinned cache and the world
//!   folder's spawn files: for each area, every option of every nearby
//!   scenery definition (the nearest object of each id), every option of the
//!   NPC types spawned there, Take on the ground spawns, the inventory items'
//!   options and a few use-with pairs, each followed by a walk through any
//!   dialogue it opens.
//! - `run [--data DIR] [--only NAME[,NAME]] [--no-build] [--output DIR]`
//!   starts a disposable world (`test-world start --log-every-refusal`), runs
//!   every tour with the kit's `agent_run`, one at a time, stamps every server
//!   log line as it appears, stops the world and writes the report.
//! - `report [--output DIR]` rebuilds the report from a run's saved files.
//!
//! The census reads what the server logs; it changes nothing in the server
//! beyond `RS09_NOT_PORTED_LOG=every`, which logs every named not-ported
//! call instead of only the first.
use crate::Result;
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "cargo xtask census <tours [--check] [--data DIR] | run [--data DIR] [--only NAME,...] [--no-build] [--output DIR] | report [--output DIR]>";
const TOURS: &str = "crates/headless/scenarios/census";
const WORLD: &str = "census";
/// The label of each tour's first step: the character stands still, so what
/// the server logs then happens every tick whatever a player does.
const IDLE: &str = "(idle baseline)";

/// One tour: an area of the world, where the character starts, and how far
/// from the centre scenery, NPC spawns and ground spawns are taken.
struct Area {
    tour: &'static str,
    area: &'static str,
    character: &'static str,
    centre: [i32; 3],
    radius: i32,
    /// Also press the tab buttons (once, in the first tour).
    tabs: bool,
}

const AREAS: &[Area] = &[
    Area {
        tour: "lumbridge_castle",
        area: "Lumbridge",
        character: "cs_lumcastle",
        centre: [3215, 3220, 0],
        radius: 14,
        tabs: true,
    },
    Area {
        tour: "lumbridge_store",
        area: "Lumbridge",
        character: "cs_lumstore",
        centre: [3212, 3247, 0],
        radius: 9,
        tabs: false,
    },
    Area {
        tour: "lumbridge_bank",
        area: "Lumbridge",
        character: "cs_lumbank",
        centre: [3208, 3220, 2],
        radius: 10,
        tabs: false,
    },
    Area {
        tour: "draynor",
        area: "Draynor",
        character: "cs_draynor",
        centre: [3093, 3247, 0],
        radius: 14,
        tabs: false,
    },
    Area {
        tour: "varrock_square",
        area: "Varrock",
        character: "cs_vsquare",
        centre: [3212, 3428, 0],
        radius: 12,
        tabs: false,
    },
    Area {
        tour: "varrock_west_bank",
        area: "Varrock",
        character: "cs_vwbank",
        centre: [3185, 3438, 0],
        radius: 8,
        tabs: false,
    },
    Area {
        tour: "varrock_shops",
        area: "Varrock",
        character: "cs_vshops",
        centre: [3213, 3410, 0],
        radius: 11,
        tabs: false,
    },
    Area {
        tour: "varrock_east",
        area: "Varrock",
        character: "cs_veast",
        centre: [3253, 3415, 0],
        radius: 12,
        tabs: false,
    },
    Area {
        tour: "al_kharid_bank",
        area: "Al Kharid",
        character: "cs_akbank",
        centre: [3271, 3168, 0],
        radius: 10,
        tabs: false,
    },
    Area {
        tour: "al_kharid_town",
        area: "Al Kharid",
        character: "cs_aktown",
        centre: [3298, 3186, 0],
        radius: 14,
        tabs: false,
    },
    Area {
        tour: "falador_square",
        area: "Falador",
        character: "cs_fsquare",
        centre: [2965, 3380, 0],
        radius: 12,
        tabs: false,
    },
    Area {
        tour: "falador_banks",
        area: "Falador",
        character: "cs_fbanks",
        centre: [2946, 3370, 0],
        radius: 9,
        tabs: false,
    },
];

/// What every tour character carries: tools, food, and things to use.
const INVENTORY: &[(i32, i32)] = &[
    (1351, 1),   // bronze axe
    (1265, 1),   // bronze pickaxe
    (590, 1),    // tinderbox
    (303, 1),    // small fishing net
    (1511, 3),   // logs
    (2309, 1),   // bread
    (315, 1),    // shrimps
    (317, 1),    // raw shrimps
    (1925, 1),   // bucket
    (1931, 1),   // pot
    (946, 1),    // knife
    (1755, 1),   // chisel
    (1733, 1),   // needle
    (1734, 5),   // thread
    (1205, 1),   // bronze dagger
    (1171, 1),   // wooden shield
    (995, 5000), // coins
    (526, 1),    // bones
    (556, 50),   // air runes
    (558, 50),   // mind runes
];
/// Use one held item on another.
const USE_PAIRS: &[(i32, i32)] = &[(946, 1511), (590, 1511), (1755, 1511), (1733, 1734)];
/// Tab buttons pressed once (interface, child): combat styles, music, emotes,
/// quests, skills, prayers, settings.
const TAB_BUTTONS: &[(i32, i32)] = &[
    (92, 3),
    (92, 4),
    (92, 5),
    (187, 11),
    (464, 2),
    (464, 3),
    (464, 4),
    (464, 5),
    (464, 6),
    (464, 7),
    (274, 10),
    (274, 11),
    (274, 12),
    (320, 1),
    (320, 2),
    (320, 125),
    (271, 5),
    (271, 7),
    (261, 3),
    (261, 4),
];
/// Scenery options that usually move the character away (stairs, ladders,
/// portals): sent last in each tour.
const MOVING: &[&str] = &[
    "climb",
    "enter",
    "cross",
    "squeeze",
    "jump",
    "go-through",
    "exit",
];

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    match args.get(1).map(String::as_str) {
        Some("tours") => tours(root, &args[2..]),
        Some("run") => census(root, &args[2..]),
        Some("report") => {
            let mut output = root.join("runs/census");
            let mut options = args[2..].iter();
            while let Some(option) = options.next() {
                match option.as_str() {
                    "--output" => {
                        output = absolute(root, options.next().ok_or("missing --output value")?)
                    }
                    other => return Err(format!("unknown option {other}\n{USAGE}").into()),
                }
            }
            report(&output)
        }
        _ => Err(USAGE.into()),
    }
}

fn absolute(root: &Path, path: impl Into<PathBuf>) -> PathBuf {
    let path = path.into();
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn default_data(root: &Path) -> PathBuf {
    for candidate in ["runs/world-trace-oracle/template", "runs/play/world-data"] {
        let dir = root.join(candidate);
        if dir.join("data/configs").is_dir() {
            return dir;
        }
    }
    root.join("runs/world-trace-oracle/template")
}

// ---------------------------------------------------------------- tours

fn archive(root: &Path, index: u8) -> Result<Archive> {
    let limits = Limits::default();
    Ok(Archive::open(
        Store::open(&root.join("assets/cache"), StorageMode::Resident, limits)?,
        index,
        None,
        limits,
    )?)
}

/// The `{a,b,c,...}-{...}-` rows of the spawn files.
fn rows(text: &str) -> Vec<Vec<i32>> {
    text.split('-')
        .filter_map(|row| {
            let row = row.trim().strip_prefix('{')?.strip_suffix('}')?;
            row.split(',').map(|n| n.trim().parse().ok()).collect()
        })
        .collect()
}

fn smart(b: &[u8], at: &mut usize) -> Option<i32> {
    let first = *b.get(*at)? as i32;
    if first < 128 {
        *at += 1;
        Some(first)
    } else {
        let v = (first << 8 | *b.get(*at + 1)? as i32) - 32768;
        *at += 2;
        Some(v)
    }
}

fn big_smart(b: &[u8], at: &mut usize) -> Option<i32> {
    let mut value = 0i32;
    let mut current = smart(b, at)?;
    while current == 32767 {
        current = smart(b, at)?;
        value = value.wrapping_add(32767);
    }
    Some(value.wrapping_add(current))
}

type TileFlags = [[[u8; 64]; 64]; 4];

/// The terrain file's tile flags (`m` files), for the bridge plane rule.
fn tile_flags(b: &[u8]) -> Box<TileFlags> {
    let mut flags = Box::new([[[0u8; 64]; 64]; 4]);
    let mut at = 0usize;
    for plane in flags.iter_mut() {
        for column in plane.iter_mut() {
            for tile in column.iter_mut() {
                loop {
                    let Some(&op) = b.get(at) else {
                        return flags;
                    };
                    at += 1;
                    match op {
                        0 => break,
                        1 => {
                            at += 1;
                            break;
                        }
                        2..=49 => at += 1,
                        50..=81 => *tile = op - 49,
                        _ => {}
                    }
                }
            }
        }
    }
    flags
}

/// (id, x, y, plane) of every object in a region's `l` file, with the
/// landscape parser's bridge rule (a tile flagged 2 on plane 1 lowers it).
fn objects(landscape: &[u8], flags: &TileFlags, rx: i32, ry: i32) -> Vec<[i32; 4]> {
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut id = -1i32;
    loop {
        let Some(offset) = big_smart(landscape, &mut at) else {
            break;
        };
        if offset == 0 {
            break;
        }
        id = id.wrapping_add(offset);
        let mut location = 0i32;
        loop {
            let Some(offset) = smart(landscape, &mut at) else {
                return out;
            };
            if offset == 0 {
                break;
            }
            location = location.wrapping_add(offset - 1);
            let (x, y) = ((location >> 6) & 63, location & 63);
            at += 1; // shape and rotation
            let mut z = location >> 12;
            if flags[1][x as usize][y as usize] & 2 == 2 {
                z -= 1;
            }
            if (0..=3).contains(&z) {
                out.push([id, rx * 64 + x, ry * 64 + y, z]);
            }
        }
    }
    out
}

type Options = Option<rs09_server::definitions::item::Shared<Vec<Option<String>>>>;

fn options_of(options: &Options) -> Vec<(usize, String)> {
    options
        .as_ref()
        .map(|o| {
            o.read()
                .iter()
                .enumerate()
                .filter_map(|(i, o)| {
                    let o = o.as_ref()?;
                    // "hidden" marks an option the client does not show.
                    let shown = !o.is_empty()
                        && !o.eq_ignore_ascii_case("null")
                        && !o.eq_ignore_ascii_case("hidden");
                    shown.then(|| (i, o.clone()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn distance(a: [i32; 2], b: [i32; 2]) -> i32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs())
}

struct Definitions {
    scenery: rs09_server::definitions::scenery::Registry,
    npcs: rs09_server::definitions::npc::Registry,
    npc_archive: Archive,
    items: rs09_server::definitions::item::Registry,
}

struct NoLog;
impl rs09_server::definitions::scenery::LogContext for NoLog {
    fn log(
        &mut self,
        _: &mut rs09_server::definitions::scenery::Registry,
        _: String,
    ) -> rs09_server::definitions::scenery::Result<()> {
        Ok(())
    }
}
struct NpcFiles<'a>(&'a Archive);
impl rs09_server::definitions::npc::Files for NpcFiles<'_> {
    fn file(
        &mut self,
        _: &mut rs09_server::definitions::npc::Registry,
        id: i32,
    ) -> rs09_server::definitions::npc::Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .file((id as u32) >> 7, (id & 127) as u32, [0; 4])
            .ok()
            .flatten()
            .map(|d| d.as_ref().clone()))
    }
}

/// A definition's name and its (slot, option) pairs.
type Named = (String, Vec<(usize, String)>);

impl Definitions {
    fn load(root: &Path) -> Result<Self> {
        let mut scenery = rs09_server::definitions::scenery::Registry::default();
        scenery.load_archive(&mut NoLog, &archive(root, 16)?, 1_000_000)?;
        let npc_archive = archive(root, 18)?;
        let mut npcs = rs09_server::definitions::npc::Registry::default();
        npcs.load_archive(&npc_archive, 1_000_000)?;
        let mut items = rs09_server::definitions::item::Registry::default();
        items.load_archive(&archive(root, 19)?, 1_000_000)?;
        Ok(Self {
            scenery,
            npcs,
            npc_archive,
            items,
        })
    }
    /// The scenery's name and options; a definition with no options of its
    /// own takes its first child's that has some (varbit-morphed objects).
    fn scenery(&mut self, id: i32) -> Option<Named> {
        let def = self.scenery.for_id(id);
        let (name, mut options, children) = {
            let d = def.read();
            (
                d.name.clone(),
                options_of(&d.options),
                d.children_ids
                    .as_ref()
                    .map(|c| c.read().clone())
                    .unwrap_or_default(),
            )
        };
        let mut name = name.filter(|n| n != "null");
        if options.is_empty() {
            for child in children.into_iter().filter(|&c| c >= 0) {
                let def = self.scenery.for_id(child);
                let d = def.read();
                let child_options = options_of(&d.options);
                if !child_options.is_empty() {
                    options = child_options;
                    name = name.or(d.name.clone()).filter(|n| n != "null");
                    break;
                }
            }
        }
        (!options.is_empty()).then(|| (name.unwrap_or_else(|| format!("scenery {id}")), options))
    }
    fn npc(&mut self, id: i32) -> Option<Named> {
        let def = self
            .npcs
            .for_id(&mut NpcFiles(&self.npc_archive), id)
            .ok()?;
        let d = def.read();
        Some((
            d.name.clone().unwrap_or_else(|| format!("npc {id}")),
            options_of(&d.options),
        ))
    }
    fn item(&self, id: i32) -> Named {
        let def = self.items.for_id(id);
        let d = def.read();
        (
            d.name.clone().unwrap_or_else(|| format!("item {id}")),
            options_of(&d.options),
        )
    }
}

struct Maps {
    archive: Archive,
    keys: BTreeMap<i32, [u32; 4]>,
}
impl Maps {
    fn load(root: &Path, data: &Path) -> Result<Self> {
        let text = fs::read_to_string(data.join("data/configs/xteas.json"))?;
        let value: Value = serde_json::from_str(&text)?;
        let mut keys = BTreeMap::new();
        for row in value["xteas"].as_array().into_iter().flatten() {
            let id: i32 = row["regionId"].as_str().unwrap_or("").parse()?;
            let parts: Vec<i32> = row["keys"]
                .as_str()
                .unwrap_or("")
                .split(',')
                .map(str::parse)
                .collect::<std::result::Result<_, _>>()?;
            if let [a, b, c, d] = parts[..] {
                keys.insert(id, [a as u32, b as u32, c as u32, d as u32]);
            }
        }
        Ok(Self {
            archive: archive(root, 5)?,
            keys,
        })
    }
    fn region(&self, rx: i32, ry: i32) -> Result<Vec<[i32; 4]>> {
        let group = |name: String| self.archive.index.group_by_ascii_name(&name);
        let (Some(m), Some(l)) = (group(format!("m{rx}_{ry}")), group(format!("l{rx}_{ry}")))
        else {
            return Ok(Vec::new());
        };
        let terrain = self.archive.file(m, 0, [0; 4])?.map(|d| d.as_ref().clone());
        let key = self.keys.get(&(rx << 8 | ry)).copied().unwrap_or([0; 4]);
        let landscape = self.archive.file(l, 0, key)?.map(|d| d.as_ref().clone());
        let (Some(terrain), Some(landscape)) = (terrain, landscape) else {
            return Ok(Vec::new());
        };
        Ok(objects(&landscape, &tile_flags(&terrain), rx, ry))
    }
}

fn step(mut map: Value, label: &str) -> Value {
    map["label"] = Value::String(label.to_owned());
    map
}

/// After an action: let the walk and the arrival happen, walk any dialogue
/// it opened to its end, close whatever interface it opened.
fn settle(steps: &mut Vec<Value>, label: &str, near: Option<[i32; 2]>, seconds: f64) {
    if let Some(tile) = near {
        steps.push(step(
            json!({"do": "wait", "until": {"near": {"tile": tile, "within": 3}}, "timeout": 8}),
            label,
        ));
    }
    steps.push(step(json!({"do": "pump", "seconds": seconds}), label));
    steps.push(step(json!({"do": "dialogue", "timeout": 1.0}), label));
    steps.push(step(json!({"do": "close"}), label));
}

fn tour(area: &Area, defs: &mut Definitions, maps: &Maps, data: &Path) -> Result<Value> {
    let [cx, cy, cz] = area.centre;
    let r = area.radius;
    let centre = [cx, cy];
    let mut steps = vec![step(json!({"do": "pump", "seconds": 12.0}), IDLE)];

    if area.tabs {
        for &(interface, child) in TAB_BUTTONS {
            let label = format!("tab button {interface}:{child}");
            steps.push(step(
                json!({"do": "button", "interface": interface, "child": child}),
                &label,
            ));
            steps.push(step(json!({"do": "pump", "seconds": 0.8}), &label));
            steps.push(step(json!({"do": "dialogue", "timeout": 0.5}), &label));
            steps.push(step(json!({"do": "close"}), &label));
        }
    }

    // NPCs spawned in the area: every option of each type.
    let spawns: Value = serde_json::from_str(&fs::read_to_string(
        data.join("data/configs/npc_spawns.json"),
    )?)?;
    let mut npc_types = BTreeSet::new();
    for row in spawns.as_array().into_iter().flatten() {
        let id: i32 = row["npc_id"].as_str().unwrap_or("-1").parse().unwrap_or(-1);
        for loc in rows(row["loc_data"].as_str().unwrap_or("")) {
            if loc.len() >= 3 && loc[2] == cz && distance([loc[0], loc[1]], centre) <= r {
                npc_types.insert(id);
            }
        }
    }
    let mut attacks = Vec::new();
    for id in npc_types {
        // A nameless NPC is not clickable in the client.
        let Some((name, options)) = defs.npc(id).filter(|(n, _)| n != "null") else {
            continue;
        };
        for (slot, option) in options {
            if slot > 4 || option.eq_ignore_ascii_case("examine") {
                continue;
            }
            let label = format!("NPC {option} {name} ({id})");
            let action = json!({"do": "npcOption", "option": slot, "npc": {"type": id}});
            if option.eq_ignore_ascii_case("attack") {
                attacks.push((action, label));
                continue;
            }
            steps.push(step(action, &label));
            settle(&mut steps, &label, None, 3.0);
        }
        let label = format!("NPC use Bread on {name} ({id})");
        steps.push(step(
            json!({"do": "useOnNpc", "item": 2309, "npc": {"type": id}}),
            &label,
        ));
        settle(&mut steps, &label, None, 2.5);
    }

    // Ground spawns in the area: Take.
    let ground: Value = serde_json::from_str(&fs::read_to_string(
        data.join("data/configs/ground_spawns.json"),
    )?)?;
    for row in ground.as_array().into_iter().flatten() {
        let id: i32 = row["item_id"]
            .as_str()
            .unwrap_or("-1")
            .parse()
            .unwrap_or(-1);
        for loc in rows(row["loc_data"].as_str().unwrap_or("")) {
            if loc.len() >= 4 && loc[3] == cz && distance([loc[1], loc[2]], centre) <= r {
                let (name, _) = defs.item(id);
                let label = format!("ground Take {name} ({id})");
                steps.push(step(
                    json!({"do": "groundItem", "option": 2, "item": id, "tile": [loc[1], loc[2]]}),
                    &label,
                ));
                settle(&mut steps, &label, Some([loc[1], loc[2]]), 1.5);
            }
        }
    }

    // Scenery: the nearest object of each id with options, every option.
    let mut nearest: BTreeMap<i32, [i32; 2]> = BTreeMap::new();
    let (rx0, rx1, ry0, ry1) = ((cx - r) >> 6, (cx + r) >> 6, (cy - r) >> 6, (cy + r) >> 6);
    for rx in rx0..=rx1 {
        for ry in ry0..=ry1 {
            for [id, x, y, z] in maps.region(rx, ry)? {
                if z != cz || distance([x, y], centre) > r {
                    continue;
                }
                let closer = nearest
                    .get(&id)
                    .is_none_or(|t| distance([x, y], centre) < distance(*t, centre));
                if closer {
                    nearest.insert(id, [x, y]);
                }
            }
        }
    }
    let mut scenery = Vec::new();
    for (id, tile) in nearest {
        let Some((name, options)) = defs.scenery(id) else {
            continue;
        };
        for (slot, option) in options {
            if slot > 4 || option.eq_ignore_ascii_case("examine") {
                continue;
            }
            let lower = option.to_ascii_lowercase();
            let moving = MOVING.iter().any(|m| lower.contains(m));
            scenery.push((
                moving,
                distance(tile, centre),
                id,
                slot,
                option,
                name.clone(),
                tile,
            ));
        }
    }
    scenery.sort();
    let water = ["fountain", "sink", "well", "pump", "water"];
    let cooking = ["range", "fire", "stove", "oven"];
    for (_, _, id, slot, option, name, tile) in &scenery {
        let label = format!("scenery {option} {name} ({id})");
        steps.push(step(
            json!({"do": "scenery", "option": slot, "scenery": id, "tile": tile}),
            &label,
        ));
        settle(&mut steps, &label, Some(*tile), 1.5);
        let lower = name.to_ascii_lowercase();
        let used = if water.iter().any(|w| lower.contains(w)) {
            Some((1925, "Bucket"))
        } else if cooking.iter().any(|w| lower.contains(w)) {
            Some((317, "Raw shrimps"))
        } else {
            None
        };
        if let (Some((item, item_name)), 0) = (used, *slot) {
            let label = format!("scenery use {item_name} on {name} ({id})");
            steps.push(step(
                json!({"do": "useOnScenery", "item": item, "scenery": id, "tile": tile}),
                &label,
            ));
            settle(&mut steps, &label, Some(*tile), 2.0);
        }
    }
    // Inventory: every item's options (drop later), then the use-with pairs.
    let mut drops = Vec::new();
    for &(id, _) in INVENTORY {
        let (name, options) = defs.item(id);
        for (slot, option) in options {
            if slot > 4 {
                continue;
            }
            let label = format!("item {option} {name} ({id})");
            let action = json!({"do": "itemOption", "option": slot, "item": id});
            if ["drop", "destroy"]
                .iter()
                .any(|d| option.eq_ignore_ascii_case(d))
            {
                drops.push((action, label));
                continue;
            }
            steps.push(step(action, &label));
            settle(&mut steps, &label, None, 1.5);
        }
    }
    for &(used, target) in USE_PAIRS {
        let label = format!("item use {} on {}", defs.item(used).0, defs.item(target).0);
        steps.push(step(
            json!({"do": "useOnItem", "item": used, "target": target}),
            &label,
        ));
        settle(&mut steps, &label, None, 2.0);
    }
    // Attacks after everything that needs the NPCs alive.
    for (action, label) in attacks {
        steps.push(step(action, &label));
        steps.push(step(json!({"do": "pump", "seconds": 6.0}), &label));
        steps.push(step(
            json!({"do": "walk", "to": centre, "run": true}),
            &label,
        ));
        steps.push(step(json!({"do": "pump", "seconds": 2.0}), &label));
    }
    // One drop (the pot: nothing else uses it).
    if let Some((action, label)) = drops.into_iter().find(|(a, _)| a["item"] == 1931) {
        steps.push(step(action, &label));
        settle(&mut steps, &label, None, 1.0);
    }

    let inventory: Vec<Value> = INVENTORY
        .iter()
        .map(|&(id, amount)| json!({"id": id, "amount": amount}))
        .collect();
    let skills: Map<String, Value> = [
        "attack",
        "defence",
        "strength",
        "hitpoints",
        "ranged",
        "prayer",
        "magic",
        "cooking",
        "woodcutting",
        "fletching",
        "fishing",
        "firemaking",
        "crafting",
        "smithing",
        "mining",
        "herblore",
        "agility",
        "thieving",
        "slayer",
        "farming",
        "runecrafting",
        "hunter",
        "construction",
        "summoning",
    ]
    .into_iter()
    .map(|s| (s.to_owned(), json!(60)))
    .collect();
    Ok(json!({
        "census": {"tour": area.tour, "area": area.area, "centre": area.centre, "radius": r},
        "keepGoing": true,
        "character": {
            "name": area.character,
            "location": area.centre,
            "inventory": inventory,
            "skills": skills,
        },
        "steps": steps,
    }))
}

fn tours(root: &Path, args: &[String]) -> Result<()> {
    let mut check = false;
    let mut data = default_data(root);
    let mut options = args.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--check" => check = true,
            "--data" => data = absolute(root, options.next().ok_or("missing --data value")?),
            other => return Err(format!("unknown option {other}\n{USAGE}").into()),
        }
    }
    let mut defs = Definitions::load(root)?;
    let maps = Maps::load(root, &data)?;
    let dir = root.join(TOURS);
    fs::create_dir_all(&dir)?;
    let mut stale = Vec::new();
    for area in AREAS {
        let tour = tour(area, &mut defs, &maps, &data)?;
        let text = format!("{}\n", serde_json::to_string_pretty(&tour)?);
        let path = dir.join(format!("{}.json", area.tour));
        let actions = tour["steps"]
            .as_array()
            .map(|s| {
                s.iter()
                    .filter_map(|s| s["label"].as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
            })
            .unwrap_or(0);
        if check {
            if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                stale.push(path.display().to_string());
            }
        } else {
            fs::write(&path, text)?;
            println!("{}: {actions} actions -> {}", area.tour, path.display());
        }
    }
    if !stale.is_empty() {
        return Err(format!(
            "stale tours (run cargo xtask census tours): {}",
            stale.join(", ")
        )
        .into());
    }
    if check {
        println!("{} census tours are current.", AREAS.len());
    }
    Ok(())
}

// ---------------------------------------------------------------- run

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Every server log line with the time it was read. Texts are interned with
/// the every-call count (` [#n]`) removed: logging every not-ported call
/// gives hundreds of thousands of lines per tour, but few distinct texts.
#[derive(Default)]
struct Lines {
    texts: Vec<String>,
    index: std::collections::HashMap<String, usize>,
    /// (unix ms, text index, 0 stderr / 1 stdout)
    events: Vec<(u64, usize, u8)>,
}
impl Lines {
    fn push(&mut self, ms: u64, source: &str, line: &str) {
        let text = strip_count(line);
        let id = match self.index.get(text) {
            Some(&id) => id,
            None => {
                self.texts.push(text.to_owned());
                self.index.insert(text.to_owned(), self.texts.len() - 1);
                self.texts.len() - 1
            }
        };
        self.events.push((ms, id, u8::from(source == "stdout")));
    }
    fn json(&self) -> Value {
        json!({
            "texts": self.texts,
            "events": self.events.iter().map(|&(ms, id, src)| json!([ms, id, src])).collect::<Vec<_>>(),
        })
    }
    fn from_json(value: &Value) -> Self {
        let texts: Vec<String> = value["texts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| t.as_str().unwrap_or("").to_owned())
            .collect();
        let events = value["events"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| {
                (
                    e[0].as_u64().unwrap_or(0),
                    e[1].as_u64().unwrap_or(0) as usize,
                    e[2].as_u64().unwrap_or(0) as u8,
                )
            })
            .collect();
        Self {
            texts,
            events,
            ..Self::default()
        }
    }
}

/// A not-ported line without its every-call count (` [#n]`).
fn strip_count(line: &str) -> &str {
    match line.rfind(" [#") {
        Some(i)
            if line.ends_with(']')
                && line[i + 3..line.len() - 1]
                    .bytes()
                    .all(|b| b.is_ascii_digit()) =>
        {
            &line[..i]
        }
        _ => line,
    }
}

/// Follows a growing log file, stamping each complete line when it is read.
struct Follow {
    path: PathBuf,
    offset: u64,
    partial: String,
    source: &'static str,
}
impl Follow {
    fn new(path: PathBuf, source: &'static str) -> Self {
        Self {
            path,
            offset: 0,
            partial: String::new(),
            source,
        }
    }
    fn poll(&mut self, out: &mut Lines) {
        let Ok(mut file) = fs::File::open(&self.path) else {
            return;
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() || bytes.is_empty() {
            return;
        }
        self.offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));
        let now = unix_ms();
        while let Some(end) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=end).collect();
            let line = line.trim_end().to_owned();
            if !line.is_empty() {
                out.push(now, self.source, &line);
            }
        }
    }
}

fn census(root: &Path, args: &[String]) -> Result<()> {
    let mut data = default_data(root);
    let mut only: Option<Vec<String>> = None;
    let mut build = true;
    let mut output = root.join("runs/census");
    let mut options = args.iter();
    while let Some(option) = options.next() {
        let mut value = || options.next().ok_or(format!("missing value for {option}"));
        match option.as_str() {
            "--data" => data = absolute(root, value()?),
            "--only" => only = Some(value()?.split(',').map(str::to_owned).collect()),
            "--no-build" => build = false,
            "--output" => output = absolute(root, value()?),
            other => return Err(format!("unknown option {other}\n{USAGE}").into()),
        }
    }
    let mut tours = Vec::new();
    for entry in fs::read_dir(root.join(TOURS))? {
        let path = entry?.path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        if path.extension().is_some_and(|e| e == "json")
            && only.as_ref().is_none_or(|o| o.contains(&name))
        {
            tours.push((name, path));
        }
    }
    // The order of AREAS (the first tour also presses the tab buttons).
    tours.sort_by_key(|(name, _)| {
        AREAS
            .iter()
            .position(|a| a.tour == name)
            .unwrap_or(usize::MAX)
    });
    if tours.is_empty() {
        return Err("no census tours (cargo xtask census tours)".into());
    }
    if build {
        // `--example` limits a build to that target, so two builds.
        for args in [
            &["build", "--release", "-p", "rs09-server"][..],
            &[
                "build",
                "--release",
                "-p",
                "rs09-headless",
                "--example",
                "agent_run",
            ],
        ] {
            let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .current_dir(root)
                .args(args)
                .status()?;
            if !status.success() {
                return Err(format!("cargo {} failed", args.join(" ")).into());
            }
        }
    }
    let agent = root.join(format!(
        "target/release/examples/agent_run{}",
        std::env::consts::EXE_SUFFIX
    ));
    if output.exists() {
        // Only a folder a census made (it holds census.json) is replaced.
        if !output.join("census.json").is_file() && fs::read_dir(&output)?.next().is_some() {
            return Err(format!("{} exists and is not a census folder", output.display()).into());
        }
        fs::remove_dir_all(&output)?;
    }
    let agents = output.join("agents");
    fs::create_dir_all(&agents)?;
    // Marks the folder as the census's before anything else is written.
    fs::write(output.join("census.json"), "{}")?;

    crate::test_world::run(
        root,
        &[
            "test-world".into(),
            "start".into(),
            "--name".into(),
            WORLD.into(),
            "--data".into(),
            data.display().to_string(),
            "--no-build".into(),
            "--log-every-refusal".into(),
        ],
    )?;
    let world_dir = root.join("runs/test-world").join(WORLD);
    let world: Value = serde_json::from_str(&fs::read_to_string(world_dir.join("world.json"))?)?;
    let follows = vec![
        Follow::new(world_dir.join("server.err.log"), "stderr"),
        Follow::new(world_dir.join("server.log"), "stdout"),
    ];
    let lines = Arc::new(Mutex::new(Lines::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let follower = {
        let (lines, stop) = (lines.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut follows = follows;
            loop {
                let last = stop.load(Ordering::SeqCst);
                {
                    let mut lines = lines.lock().unwrap();
                    for f in &mut follows {
                        f.poll(&mut lines);
                    }
                }
                if last {
                    break;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        })
    };

    let mut runs = Vec::new();
    let result = (|| -> Result<()> {
        for (name, path) in &tours {
            let script: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
            let report = agents.join(format!("{name}.json"));
            let log = agents.join(format!("{name}.log"));
            eprintln!("census: tour {name} ...");
            let started = unix_ms();
            let began = Instant::now();
            let status = Command::new(&agent)
                .current_dir(root)
                .arg("--world")
                .arg(&world_dir)
                .arg("--output")
                .arg(&report)
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(fs::File::create(&log)?)
                .status()?;
            // Let the logout and the last tick's lines arrive.
            std::thread::sleep(Duration::from_secs(2));
            let ended = unix_ms();
            eprintln!(
                "census: tour {name} finished ({status}) in {:.0} s",
                began.elapsed().as_secs_f64()
            );
            runs.push(json!({
                "tour": name,
                "area": script["census"]["area"],
                "script": path.strip_prefix(root).unwrap_or(path).display().to_string().replace('\\', "/"),
                "startedMs": started,
                "endedMs": ended,
                "exit": status.code(),
                "report": format!("agents/{name}.json"),
                "log": format!("agents/{name}.log"),
            }));
        }
        Ok(())
    })();
    stop.store(true, Ordering::SeqCst);
    let _ = follower.join();
    let stopped = crate::test_world::run(
        root,
        &[
            "test-world".into(),
            "stop".into(),
            "--name".into(),
            WORLD.into(),
        ],
    );
    let lines = std::mem::take(&mut *lines.lock().unwrap());
    fs::write(
        output.join("server-lines.json"),
        serde_json::to_string(&lines.json())?,
    )?;
    fs::write(
        output.join("census.json"),
        serde_json::to_string_pretty(&json!({
            "world": world,
            "data": data,
            "tours": runs,
            "when": unix_ms(),
        }))?,
    )?;
    result?;
    stopped?;
    report(&output)
}

// ---------------------------------------------------------------- report

/// Numbers in errors (indices, ids, coordinates) vary per hit: one `#` each.
fn numbers_hidden(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_digit = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !last_digit {
                out.push('#');
            }
            last_digit = true;
        } else {
            out.push(c);
            last_digit = false;
        }
    }
    out
}

/// The text of a refusal carried inside an error's debug form: from
/// "not ported in the live world" to the first quote or backslash.
fn refusal_text(line: &str, at: usize) -> String {
    let what = &line[at..];
    let end = what.find(['"', '\\']).unwrap_or(what.len());
    numbers_hidden(what[..end].trim_end())
}

/// The kind and name of a server log line the census counts, if it is one.
fn classify(line: &str) -> Option<(&'static str, String)> {
    let line = strip_count(line);
    if let Some(rest) = line.strip_prefix("[live] not ported, skipped: ") {
        return Some(("skipped", rest.to_owned()));
    }
    if let Some(rest) = line.strip_prefix("[live] not ported, original's empty answer: ") {
        return Some(("empty answer", rest.to_owned()));
    }
    // The start-up summary ("... steps partly ported or refused") is not a hit.
    if line.starts_with("World booted") || line.contains("walk debug:") {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("panicked") {
        return Some(("panic", numbers_hidden(line)));
    }
    // `PacketProcessor Err Error Processing IfAction: ...`: the packet class.
    let class = line.find("Error Processing ").map(|j| {
        let rest = &line[j + "Error Processing ".len()..];
        rest.split(':').next().unwrap_or("").to_owned()
    });
    // A service that failed outside a packet: "deferred walk service failed: ...".
    let context = class.filter(|c| !c.is_empty()).or_else(|| {
        let i = line.find(" failed:")?;
        let start = line[..i].rfind(']').map_or(0, |j| j + 1);
        Some(line[start..i].trim().to_owned())
    });
    if let Some(i) = line.find("not ported in the live world") {
        let what = refusal_text(line, i);
        return Some((
            "refused",
            match context {
                Some(c) => format!("{c}: {what}"),
                None => what,
            },
        ));
    }
    if line.contains("Error Processing")
        || lower.contains(" err ")
        || lower.contains("error")
        || lower.contains(" failed:")
    {
        let at = line.find("Error Processing").unwrap_or(0);
        return Some((
            "error",
            numbers_hidden(&line[at..]).chars().take(240).collect(),
        ));
    }
    if lower.contains("not ported") {
        return Some(("refused", numbers_hidden(line).chars().take(240).collect()));
    }
    None
}

#[derive(Default)]
struct Entry {
    kind: &'static str,
    hits: u64,
    /// Tours whose idle baseline hit it. Hit in at least half the tours'
    /// baselines, it happens every tick whatever the player does (a single
    /// baseline can also catch the previous character's delayed logout).
    idle: BTreeSet<String>,
    actions: BTreeSet<(String, String)>,
    tours: BTreeSet<String>,
    areas: BTreeSet<String>,
    examples: Vec<String>,
}

/// A finished tour's steps with their start times, to attribute a line to.
struct TourRun {
    name: String,
    area: String,
    started: u64,
    ended: u64,
    steps: Vec<(u64, String)>,
    ok: usize,
    failed: usize,
}

fn report(output: &Path) -> Result<()> {
    let census: Value = serde_json::from_str(&fs::read_to_string(output.join("census.json"))?)?;
    let lines = Lines::from_json(&serde_json::from_str(&fs::read_to_string(
        output.join("server-lines.json"),
    )?)?);
    // Each text is classified once.
    let classes: Vec<Option<(&'static str, String)>> =
        lines.texts.iter().map(|t| classify(t)).collect();
    let mut tours = Vec::new();
    for run in census["tours"].as_array().into_iter().flatten() {
        let report: Value = fs::read_to_string(output.join(run["report"].as_str().unwrap_or("")))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let mut steps = Vec::new();
        let (mut ok, mut failed) = (0, 0);
        for s in report["steps"].as_array().into_iter().flatten() {
            let label = s["spec"]["label"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| s["spec"]["do"].as_str().unwrap_or("?").to_owned());
            steps.push((s["startedMs"].as_u64().unwrap_or(0), label));
            if s["ok"] == true {
                ok += 1;
            } else {
                failed += 1;
            }
        }
        tours.push(TourRun {
            name: run["tour"].as_str().unwrap_or("").to_owned(),
            area: run["area"].as_str().unwrap_or("").to_owned(),
            started: run["startedMs"].as_u64().unwrap_or(0),
            ended: run["endedMs"].as_u64().unwrap_or(0),
            steps,
            ok,
            failed,
        });
    }
    let mut entries: BTreeMap<(&'static str, String), Entry> = BTreeMap::new();
    for t in &mut tours {
        t.steps.sort_by_key(|(started, _)| *started);
    }
    let mut counted = 0u64;
    for &(ms, id, _) in &lines.events {
        let Some((kind, name)) = &classes[id] else {
            continue;
        };
        counted += 1;
        let (tour, area, action) = match tours.iter().find(|t| t.started <= ms && ms <= t.ended) {
            Some(t) => {
                let after = t.steps.partition_point(|(started, _)| *started <= ms);
                let action = match after {
                    0 => "(login)".to_owned(),
                    n => t.steps[n - 1].1.clone(),
                };
                (t.name.as_str(), t.area.as_str(), action)
            }
            None => ("(outside tours)", "", "(start-up)".to_owned()),
        };
        let e = entries
            .entry((kind, name.clone()))
            .or_insert_with(|| Entry {
                kind,
                ..Entry::default()
            });
        e.hits += 1;
        if action == IDLE && !e.idle.contains(tour) {
            e.idle.insert(tour.to_owned());
        }
        if !e.tours.contains(tour) {
            e.tours.insert(tour.to_owned());
        }
        if !area.is_empty() && !e.areas.contains(area) {
            e.areas.insert(area.to_owned());
        }
        e.actions.insert((tour.to_owned(), action));
        let text = &lines.texts[id];
        if e.examples.len() < 3 && !e.examples.iter().any(|x| x == text) {
            e.examples.push(text.chars().take(400).collect());
        }
    }
    let mut ranked: Vec<(String, Entry)> = entries
        .into_iter()
        .map(|((_, name), e)| (name, e))
        .collect();
    ranked.sort_by(|(an, a), (bn, b)| {
        b.actions
            .len()
            .cmp(&a.actions.len())
            .then(b.hits.cmp(&a.hits))
            .then(an.cmp(bn))
    });
    let (ambient, ranked): (Vec<_>, Vec<_>) = ranked.into_iter().partition(|(_, e)| e.idle.len() * 2 >= tours.len().max(1));
    let seconds = |t: &TourRun| t.ended.saturating_sub(t.started) as f64 / 1000.0;
    let minutes = tours.iter().map(seconds).sum::<f64>().max(1.0) / 60.0;
    let entry_json = |i: usize, name: &str, e: &Entry| {
        json!({
            "rank": i + 1,
            "kind": e.kind,
            "name": name,
            "hits": e.hits,
            "hitsPerMinute": e.hits as f64 / minutes,
            "actions": e.actions.len(),
            "tours": e.tours,
            "areas": e.areas,
            "hitBy": e.actions.iter().map(|(t, a)| json!({"tour": t, "action": a})).collect::<Vec<_>>(),
            "examples": e.examples,
        })
    };
    let tour_rows: Vec<Value> = tours
        .iter()
        .map(|t| json!({"tour": t.name, "area": t.area, "stepsOk": t.ok, "stepsFailed": t.failed, "seconds": seconds(t)}))
        .collect();
    fs::write(
        output.join("report.json"),
        serde_json::to_string_pretty(&json!({
            "serverLines": lines.events.len(),
            "countedLines": counted,
            "tourMinutes": minutes,
            "tours": tour_rows,
            "entries": ranked.iter().enumerate().map(|(i, (n, e))| entry_json(i, n, e)).collect::<Vec<_>>(),
            "everyTick": ambient.iter().enumerate().map(|(i, (n, e))| entry_json(i, n, e)).collect::<Vec<_>>(),
        }))?,
    )?;

    let mut md = String::new();
    md.push_str("# Refusal census\n\n");
    md.push_str("Generated by `cargo xtask census run` ([docs/refusal-census.md](../../docs/refusal-census.md)). Each refusal or error the server logged while the tours ran, ranked by the number of distinct tour actions that hit it, then by hits. *Actions* counts each (tour, action) once; *hits* counts every log line. Refusals logged during the idle baseline (the character standing still) of at least half the tours happen every tick whatever a player does; they are listed apart, by hits per minute.\n\n");
    md.push_str(&format!(
        "{} server log lines over {:.1} tour minutes; {} counted; {} distinct refusals and errors from actions, {} every tick.\n\n",
        lines.events.len(),
        minutes,
        counted,
        ranked.len(),
        ambient.len()
    ));
    md.push_str("## Tours\n\n| tour | area | steps ok | steps failed | seconds |\n| --- | --- | ---: | ---: | ---: |\n");
    for t in &tours {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.0} |\n",
            t.name,
            t.area,
            t.ok,
            t.failed,
            seconds(t)
        ));
    }
    let cell = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    md.push_str("\n## Ranking (hit by actions)\n\n| # | kind | refusal or error | actions | hits | areas | e.g. hit by |\n| ---: | --- | --- | ---: | ---: | --- | --- |\n");
    for (i, (name, e)) in ranked.iter().enumerate() {
        let examples: Vec<String> = e
            .actions
            .iter()
            .take(3)
            .map(|(t, a)| format!("{t}: {a}"))
            .collect();
        md.push_str(&format!(
            "| {} | {} | `{}` | {} | {} | {} | {} |\n",
            i + 1,
            e.kind,
            cell(name),
            e.actions.len(),
            e.hits,
            e.areas.iter().cloned().collect::<Vec<_>>().join(", "),
            cell(&examples.join("; "))
        ));
    }
    md.push_str("\n## Every tick (hit while standing still)\n\n| # | kind | refusal or error | hits | hits per minute |\n| ---: | --- | --- | ---: | ---: |\n");
    let mut ambient = ambient;
    ambient.sort_by(|(an, a), (bn, b)| b.hits.cmp(&a.hits).then(an.cmp(bn)));
    for (i, (name, e)) in ambient.iter().enumerate() {
        md.push_str(&format!(
            "| {} | {} | `{}` | {} | {:.0} |\n",
            i + 1,
            e.kind,
            cell(name),
            e.hits,
            e.hits as f64 / minutes
        ));
    }
    fs::write(output.join("report.md"), md)?;
    println!(
        "Census report: {} ({} ranked, {} every tick); JSON: {}",
        output.join("report.md").display(),
        ranked.len(),
        ambient.len(),
        output.join("report.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_server_lines() {
        assert_eq!(
            classify("[live] not ported, skipped: Shops.openShop (content) [#3]"),
            Some(("skipped", "Shops.openShop (content)".to_owned()))
        );
        assert_eq!(
            classify("[live] not ported, original's empty answer: quest requirements"),
            Some(("empty answer", "quest requirements".to_owned()))
        );
        assert_eq!(
            classify(
                "PacketProcessor Err Error Processing IfAction: x 12 not ported in the live world: global handler service open_interface"
            ),
            Some((
                "refused",
                "IfAction: not ported in the live world: global handler service open_interface"
                    .to_owned()
            ))
        );
        assert_eq!(classify("Native world listening on TCP 127.0.0.1:1"), None);
        assert_eq!(
            classify(
                r#"[live] deferred walk service failed: Binding("Binding(\"Binding(Some(\\\"\\\\\\\"not ported in the live world: openGrandExchangeCollectionBox (the Grand Exchange is not installed)\\\\\\\"\\\"))\")")"#
            ),
            Some((
                "refused",
                "deferred walk service: not ported in the live world: openGrandExchangeCollectionBox (the Grand Exchange is not installed)".to_owned()
            ))
        );
        assert_eq!(
            classify(
                "World booted in 12 ms: start-up complete = false, 3 steps partly ported or refused"
            ),
            None
        );
        assert_eq!(strip_count("x [#12]"), "x");
        assert_eq!(strip_count("x [#a]"), "x [#a]");
    }

    #[test]
    fn reads_spawn_rows() {
        assert_eq!(
            rows("{3221,3218,0,1,0}-{1,2,3,4,5}-"),
            vec![vec![3221, 3218, 0, 1, 0], vec![1, 2, 3, 4, 5]]
        );
    }
}
