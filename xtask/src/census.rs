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
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "cargo xtask census <tours [--check] [--data DIR] | run [--data DIR] [--only NAME,...] [--no-build] [--output DIR] [--tour-timeout SECS] | report [--output DIR]>";
const TOURS: &str = "crates/headless/scenarios/census";
const WORLD: &str = "census";
/// The label of each tour's first step: the character stands still, so what
/// the server logs then happens every tick whatever a player does.
const IDLE: &str = "(idle baseline)";
/// How long the idle baseline lasts.
const IDLE_SECONDS: f64 = 20.0;
/// Lines before a tour's first step (its login).
const LOGIN: &str = "(login)";
/// Lines outside every tour's window.
const OUTSIDE: &str = "(outside tours)";

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
    while let Some(offset) = big_smart(landscape, &mut at) {
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
    let mut steps = vec![step(json!({"do": "pump", "seconds": IDLE_SECONDS}), IDLE)];

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
    index: HashMap<String, u32>,
    /// (unix ms, text index, 0 stderr / 1 stdout)
    events: Vec<(u64, u32, u8)>,
}
impl Lines {
    fn push(&mut self, ms: u64, source: &str, line: &str) {
        let text = strip_count(line);
        let id = match self.index.get(text) {
            Some(&id) => id,
            None => {
                let id = self.texts.len() as u32;
                self.texts.push(text.to_owned());
                self.index.insert(text.to_owned(), id);
                id
            }
        };
        self.events.push((ms, id, u8::from(source == "stdout")));
    }
    /// `server-texts.json` (the distinct texts) and `server-events.tsv`
    /// (`ms<TAB>text index<TAB>0 stderr / 1 stdout`, one line per event).
    fn save(&self, output: &Path) -> Result<()> {
        fs::write(output.join(TEXTS_FILE), serde_json::to_string(&self.texts)?)?;
        let mut out = BufWriter::new(fs::File::create(output.join(EVENTS_FILE))?);
        for &(ms, id, source) in &self.events {
            writeln!(out, "{ms}\t{id}\t{source}")?;
        }
        out.flush()?;
        Ok(())
    }
    fn load(output: &Path) -> Result<Self> {
        let texts: Vec<String> =
            serde_json::from_str(&fs::read_to_string(output.join(TEXTS_FILE))?)?;
        let mut events = Vec::new();
        for line in BufReader::new(fs::File::open(output.join(EVENTS_FILE))?).lines() {
            let line = line?;
            let mut parts = line.split('\t');
            let (Some(ms), Some(id), Some(source)) = (parts.next(), parts.next(), parts.next())
            else {
                return Err(format!("bad {EVENTS_FILE} line: {line:?}").into());
            };
            let id: u32 = id.parse()?;
            if id as usize >= texts.len() {
                return Err(format!("{EVENTS_FILE} names text {id} of {}", texts.len()).into());
            }
            events.push((ms.parse()?, id, source.parse()?));
        }
        Ok(Self {
            texts,
            events,
            ..Self::default()
        })
    }
}

const TEXTS_FILE: &str = "server-texts.json";
const EVENTS_FILE: &str = "server-events.tsv";

/// A not-ported line without its every-call count (` [#n]`).
fn strip_count(line: &str) -> &str {
    match line.rfind(" [#") {
        Some(i)
            if line.ends_with(']')
                && i + 3 < line.len() - 1
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
            let line = line.trim_end();
            if !line.is_empty() {
                out.push(now, self.source, line);
            }
        }
    }
}

/// Stops the census world however the run ends (an error, a panic).
struct WorldGuard<'a> {
    root: &'a Path,
}
impl Drop for WorldGuard<'_> {
    fn drop(&mut self) {
        let stop = [
            "test-world".to_owned(),
            "stop".to_owned(),
            "--name".to_owned(),
            WORLD.to_owned(),
        ];
        if let Err(e) = crate::test_world::run(self.root, &stop) {
            eprintln!("census: could not stop the census world: {e}");
        }
    }
}

fn census(root: &Path, args: &[String]) -> Result<()> {
    let mut data = default_data(root);
    let mut only: Option<Vec<String>> = None;
    let mut build = true;
    let mut output = root.join("runs/census");
    let mut tour_limit = Duration::from_secs(1500);
    let mut options = args.iter();
    while let Some(option) = options.next() {
        let mut value = || options.next().ok_or(format!("missing value for {option}"));
        match option.as_str() {
            "--data" => data = absolute(root, value()?),
            "--only" => only = Some(value()?.split(',').map(str::to_owned).collect()),
            "--no-build" => build = false,
            "--output" => output = absolute(root, value()?),
            "--tour-timeout" => tour_limit = Duration::from_secs(value()?.parse()?),
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
    let guard = WorldGuard { root };
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
            let mut child = Command::new(&agent)
                .current_dir(root)
                .arg("--world")
                .arg(&world_dir)
                .arg("--output")
                .arg(&report)
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(fs::File::create(&log)?)
                .spawn()?;
            // A wall-clock limit per tour: a stuck agent is killed.
            let (status, timed_out) = loop {
                if let Some(status) = child.try_wait()? {
                    break (Some(status), false);
                }
                if began.elapsed() > tour_limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    break (None, true);
                }
                std::thread::sleep(Duration::from_millis(200));
            };
            // Let the logout and the last tick's lines arrive.
            std::thread::sleep(Duration::from_secs(2));
            let ended = unix_ms();
            eprintln!(
                "census: tour {name} {} in {:.0} s",
                match (status, timed_out) {
                    (_, true) => "killed at the tour time limit".to_owned(),
                    (Some(s), _) => format!("finished ({s})"),
                    (None, _) => "ended".to_owned(),
                },
                began.elapsed().as_secs_f64()
            );
            runs.push(json!({
                "tour": name,
                "area": script["census"]["area"],
                "script": path.strip_prefix(root).unwrap_or(path).display().to_string().replace('\\', "/"),
                "startedMs": started,
                "endedMs": ended,
                "exit": status.and_then(|s| s.code()),
                "timedOut": timed_out,
                "report": format!("agents/{name}.json"),
                "log": format!("agents/{name}.log"),
            }));
        }
        Ok(())
    })();
    stop.store(true, Ordering::SeqCst);
    let _ = follower.join();
    drop(guard);
    let lines = std::mem::take(&mut *lines.lock().unwrap());
    lines.save(&output)?;
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

/// What a counted line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// `not_ported::Kind::Skipped`.
    Skipped,
    /// `not_ported::Kind::Default`.
    EmptyAnswer,
    /// An error naming something "not ported in the live world".
    Refused,
    /// A packet or service error, or a panic: listed apart.
    Error,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Skipped => "skipped",
            Kind::EmptyAnswer => "empty answer",
            Kind::Refused => "refused",
            Kind::Error => "error",
        }
    }
}

/// The kind and name of a server log line the census counts, if it is one.
/// The not-ported prefixes are the server's own (`not_ported::line`).
fn classify(line: &str) -> Option<(Kind, String)> {
    use rs09_server::world::live::not_ported::{DEFAULT_PREFIX, SKIPPED_PREFIX};
    let line = strip_count(line);
    if let Some(rest) = line.strip_prefix(SKIPPED_PREFIX) {
        return Some((Kind::Skipped, rest.to_owned()));
    }
    if let Some(rest) = line.strip_prefix(DEFAULT_PREFIX) {
        return Some((Kind::EmptyAnswer, rest.to_owned()));
    }
    // The start-up summary ("... steps partly ported or refused") is not a hit.
    if line.starts_with("World booted") || line.contains("walk debug:") {
        return None;
    }
    if line.contains("panicked") {
        return Some((Kind::Error, format!("panic: {}", numbers_hidden(line))));
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
            Kind::Refused,
            match context {
                Some(c) => format!("{c}: {what}"),
                None => what,
            },
        ));
    }
    // Only the server's two error forms; other lines that merely mention
    // an error are not counted.
    if let Some(at) = line.find("Error Processing") {
        return Some((
            Kind::Error,
            numbers_hidden(&line[at..]).chars().take(240).collect(),
        ));
    }
    if let Some(context) = context.filter(|_| line.contains(" failed:")) {
        let at = line.find(" failed:").unwrap_or(0);
        let rest: String = numbers_hidden(&line[at + " failed:".len()..])
            .chars()
            .take(200)
            .collect();
        return Some((Kind::Error, format!("{context} failed:{rest}")));
    }
    None
}

#[derive(Default)]
struct Entry {
    hits: u64,
    /// Hits per (tour, action).
    by_action: BTreeMap<(String, String), u64>,
    /// Hits during the idle baselines, and the tours whose baseline hit it.
    baseline_hits: u64,
    baseline_tours: BTreeSet<String>,
    examples: Vec<String>,
}

/// A finished tour's steps with their start times, to attribute a line to.
struct TourRun {
    name: String,
    area: String,
    started: u64,
    ended: u64,
    /// (start ms, action label), by start.
    steps: Vec<(u64, String)>,
    ok: usize,
    failed: usize,
    exit: Option<i64>,
    timed_out: bool,
    has_report: bool,
}
impl TourRun {
    /// Milliseconds each action ran: a step runs until the next starts.
    fn action_ms(&self) -> BTreeMap<String, u64> {
        let mut out = BTreeMap::new();
        for (i, (start, label)) in self.steps.iter().enumerate() {
            let end = self.steps.get(i + 1).map_or(self.ended, |(s, _)| *s);
            *out.entry(label.clone()).or_insert(0) += end.saturating_sub(*start);
        }
        out
    }
}

/// Whether `hits` in an action are more than the idle baseline explains:
/// more than 1.5 times its expectation `e` plus a Poisson-like margin, or
/// any hit when the baseline never saw it. The factor covers rates that
/// follow how many NPCs are near (the entity tick hooks), which vary more
/// than a Poisson count.
fn credited(hits: u64, expected: f64) -> bool {
    if expected <= 0.0 {
        return hits >= 1;
    }
    hits as f64 > 1.5 * expected + 2.0 * expected.sqrt() + 1.0
}

fn report(output: &Path) -> Result<()> {
    let census: Value = serde_json::from_str(&fs::read_to_string(output.join("census.json"))?)?;
    let lines = Lines::load(output)?;
    let classes: Vec<Option<(Kind, String)>> = lines.texts.iter().map(|t| classify(t)).collect();
    let mut tours = Vec::new();
    for run in census["tours"].as_array().into_iter().flatten() {
        let text = fs::read_to_string(output.join(run["report"].as_str().unwrap_or(""))).ok();
        let report: Value = text
            .as_deref()
            .and_then(|t| serde_json::from_str(t).ok())
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
        steps.sort_by_key(|(start, _)| *start);
        tours.push(TourRun {
            name: run["tour"].as_str().unwrap_or("").to_owned(),
            area: run["area"].as_str().unwrap_or("").to_owned(),
            started: run["startedMs"].as_u64().unwrap_or(0),
            ended: run["endedMs"].as_u64().unwrap_or(0),
            steps,
            ok,
            failed,
            exit: run["exit"].as_i64(),
            timed_out: run["timedOut"] == true,
            has_report: report.is_object(),
        });
    }

    // Attribute every counted line to the tour action running when it was read.
    let mut entries: BTreeMap<(Kind, String), Entry> = BTreeMap::new();
    let mut counted = 0u64;
    let mut not_ported_lines = 0u64;
    for &(ms, id, _) in &lines.events {
        let Some((kind, name)) = &classes[id as usize] else {
            continue;
        };
        counted += 1;
        if matches!(kind, Kind::Skipped | Kind::EmptyAnswer) {
            not_ported_lines += 1;
        }
        let (tour, action) = match tours.iter().find(|t| t.started <= ms && ms <= t.ended) {
            Some(t) => {
                let after = t.steps.partition_point(|(started, _)| *started <= ms);
                let action = match after {
                    0 => LOGIN,
                    n => t.steps[n - 1].1.as_str(),
                };
                (t.name.as_str(), action)
            }
            None => (OUTSIDE, "(start-up or between tours)"),
        };
        let e = entries.entry((*kind, name.clone())).or_default();
        e.hits += 1;
        if action == IDLE {
            e.baseline_hits += 1;
            if !e.baseline_tours.contains(tour) {
                e.baseline_tours.insert(tour.to_owned());
            }
        }
        *e.by_action
            .entry((tour.to_owned(), action.to_owned()))
            .or_insert(0) += 1;
        let text = &lines.texts[id as usize];
        if e.examples.len() < 3 && !e.examples.iter().any(|x| x == text) {
            e.examples.push(text.chars().take(400).collect());
        }
    }

    // The idle baseline's length over all tours, and each action's length.
    let action_ms: BTreeMap<&str, BTreeMap<String, u64>> = tours
        .iter()
        .map(|t| (t.name.as_str(), t.action_ms()))
        .collect();
    let baseline_seconds: f64 = action_ms
        .values()
        .filter_map(|m| m.get(IDLE))
        .map(|&ms| ms as f64 / 1000.0)
        .sum();
    let tour_minutes = tours
        .iter()
        .map(|t| t.ended.saturating_sub(t.started) as f64 / 60_000.0)
        .sum::<f64>();

    struct Row {
        kind: Kind,
        name: String,
        hits: u64,
        seen: usize,
        credited: Vec<(String, String)>,
        baseline_per_minute: f64,
        baseline_tours: usize,
        areas: BTreeSet<String>,
        entry: Entry,
    }
    let area_of: BTreeMap<&str, &str> = tours
        .iter()
        .map(|t| (t.name.as_str(), t.area.as_str()))
        .collect();
    let mut rows: Vec<Row> = entries
        .into_iter()
        .map(|((kind, name), entry)| {
            let rate = if baseline_seconds > 0.0 {
                entry.baseline_hits as f64 / baseline_seconds
            } else {
                0.0
            };
            let credited: Vec<(String, String)> = entry
                .by_action
                .iter()
                .filter(|&((tour, action), &hits)| {
                    let lengths = action_ms.get(tour.as_str());
                    let ms = lengths.and_then(|m| m.get(action)).copied().unwrap_or(0);
                    // This tour's own baseline rate when it is higher (the
                    // area's NPCs), else the rate over all baselines.
                    let idle_ms = lengths.and_then(|m| m.get(IDLE)).copied().unwrap_or(0);
                    let idle_hits = entry
                        .by_action
                        .get(&(tour.clone(), IDLE.to_owned()))
                        .copied()
                        .unwrap_or(0);
                    let own = if idle_ms > 0 {
                        idle_hits as f64 * 1000.0 / idle_ms as f64
                    } else {
                        0.0
                    };
                    action != IDLE
                        && tour != OUTSIDE
                        && credited(hits, rate.max(own) * ms as f64 / 1000.0)
                })
                .map(|(key, _)| key.clone())
                .collect();
            let areas = credited
                .iter()
                .filter_map(|(t, _)| area_of.get(t.as_str()).map(|a| a.to_string()))
                .collect();
            Row {
                kind,
                hits: entry.hits,
                seen: entry
                    .by_action
                    .keys()
                    .filter(|(t, a)| a != IDLE && t != OUTSIDE)
                    .count(),
                credited,
                baseline_per_minute: rate * 60.0,
                baseline_tours: entry.baseline_tours.len(),
                areas,
                name,
                entry,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.credited
            .len()
            .cmp(&a.credited.len())
            .then(b.hits.cmp(&a.hits))
            .then(a.name.cmp(&b.name))
    });
    let (errors, ranked): (Vec<Row>, Vec<Row>) =
        rows.into_iter().partition(|r| r.kind == Kind::Error);
    let mut every_tick: Vec<&Row> = ranked
        .iter()
        .filter(|r| r.baseline_tours * 2 >= tours.len().max(1))
        .collect();
    every_tick.sort_by(|a, b| {
        b.baseline_per_minute
            .total_cmp(&a.baseline_per_minute)
            .then(a.name.cmp(&b.name))
    });

    // What makes the run untrustworthy: said in the report, and an error exit.
    let mut problems = Vec::new();
    for t in &tours {
        if t.timed_out {
            problems.push(format!("tour {} was killed at the tour time limit", t.name));
        } else if !matches!(t.exit, Some(0 | 1)) {
            problems.push(format!(
                "tour {}'s agent exited with {:?} (2 is an error: no server, a failed login, a bad script)",
                t.name, t.exit
            ));
        }
        if !t.has_report || t.steps.is_empty() {
            problems.push(format!("tour {} reported no steps", t.name));
        } else if t.failed * 2 > t.ok + t.failed {
            problems.push(format!(
                "tour {}: {} of {} steps failed",
                t.name,
                t.failed,
                t.ok + t.failed
            ));
        }
    }
    if tours.is_empty() {
        problems.push("no tour ran".to_owned());
    }
    if not_ported_lines == 0 {
        problems.push(
            "the server logged no not-ported line at all (was RS09_NOT_PORTED_LOG=every set?)"
                .to_owned(),
        );
    }

    let row_json = |i: usize, r: &Row| {
        json!({
            "rank": i + 1,
            "kind": r.kind.label(),
            "name": r.name,
            "hits": r.hits,
            "creditedActions": r.credited.len(),
            "actionsSeen": r.seen,
            "baselinePerMinute": r.baseline_per_minute,
            "baselineTours": r.baseline_tours,
            "areas": r.areas,
            "credited": r.credited.iter().map(|(t, a)| json!({"tour": t, "action": a})).collect::<Vec<_>>(),
            "hitBy": r.entry.by_action.iter().map(|((t, a), n)| json!({"tour": t, "action": a, "hits": n})).collect::<Vec<_>>(),
            "examples": r.entry.examples,
        })
    };
    let seconds = |t: &TourRun| t.ended.saturating_sub(t.started) as f64 / 1000.0;
    let tour_rows: Vec<Value> = tours
        .iter()
        .map(|t| {
            json!({"tour": t.name, "area": t.area, "exit": t.exit, "timedOut": t.timed_out,
            "stepsOk": t.ok, "stepsFailed": t.failed, "seconds": seconds(t)})
        })
        .collect();
    fs::write(
        output.join("report.json"),
        serde_json::to_string_pretty(&json!({
            "problems": problems,
            "serverLines": lines.events.len(),
            "countedLines": counted,
            "tourMinutes": tour_minutes,
            "baselineSeconds": baseline_seconds,
            "tours": tour_rows,
            "entries": ranked.iter().enumerate().map(|(i, r)| row_json(i, r)).collect::<Vec<_>>(),
            "everyTick": every_tick.iter().map(|r| json!({"kind": r.kind.label(), "name": r.name,
                "baselinePerMinute": r.baseline_per_minute, "baselineTours": r.baseline_tours})).collect::<Vec<_>>(),
            "errors": errors.iter().enumerate().map(|(i, r)| row_json(i, r)).collect::<Vec<_>>(),
        }))?,
    )?;

    let cell = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut md = String::new();
    md.push_str("# Refusal census\n\n");
    if !problems.is_empty() {
        md.push_str(
            "> **WARNING: this run is incomplete; do not read the ranking as a census.**\n>\n",
        );
        for p in &problems {
            md.push_str(&format!("> - {}\n", cell(p)));
        }
        md.push('\n');
    }
    md.push_str("Generated by `cargo xtask census run` ([docs/refusal-census.md](../../docs/refusal-census.md)). Each not-ported refusal the server logged while the tours ran. **Credit is by time window, not causation:** a line counts for the tour action that was running when it was read. An action is *credited* with a refusal only when it hit it more often than the idle baseline (the character standing still, 20 s per tour) explains: any hit if the baseline never saw it, otherwise more than 1.5 times the baseline's expectation for that action's length plus a margin. The ranking is by credited actions, then hits. *Seen* counts every action during which it was logged; *baseline/min* is its rate while idle.\n\n");
    md.push_str(&format!(
        "{} server log lines over {:.1} tour minutes ({:.0} s of idle baseline); {} counted; {} refusals, {} errors.\n\n",
        lines.events.len(),
        tour_minutes,
        baseline_seconds,
        counted,
        ranked.len(),
        errors.len()
    ));
    md.push_str("## Tours\n\n| tour | area | exit | steps ok | steps failed | seconds |\n| --- | --- | --- | ---: | ---: | ---: |\n");
    for t in &tours {
        let exit = match (t.timed_out, t.exit) {
            (true, _) => "killed (time limit)".to_owned(),
            (_, Some(code)) => code.to_string(),
            (_, None) => "none".to_owned(),
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.0} |\n",
            t.name,
            t.area,
            exit,
            t.ok,
            t.failed,
            seconds(t)
        ));
    }
    md.push_str("\nExit 1 means some steps failed (an NPC out of view, a tile not reached); the tours keep going.\n");
    md.push_str("\n## Ranking\n\n| # | kind | refusal | credited actions | seen | hits | baseline/min | areas | e.g. credited to |\n| ---: | --- | --- | ---: | ---: | ---: | ---: | --- | --- |\n");
    for (i, r) in ranked.iter().enumerate() {
        let examples: Vec<String> = r
            .credited
            .iter()
            .take(3)
            .map(|(t, a)| format!("{t}: {a}"))
            .collect();
        md.push_str(&format!(
            "| {} | {} | `{}` | {} | {} | {} | {} | {} | {} |\n",
            i + 1,
            r.kind.label(),
            cell(&r.name),
            r.credited.len(),
            r.seen,
            r.hits,
            if r.baseline_per_minute > 0.0 {
                format!("{:.1}", r.baseline_per_minute)
            } else {
                String::new()
            },
            r.areas.iter().cloned().collect::<Vec<_>>().join(", "),
            cell(&examples.join("; "))
        ));
    }
    md.push_str("\n## Every tick (logged while standing still in at least half the tours)\n\nThese stay in the ranking above, where an action is credited only for hits beyond this rate.\n\n| kind | refusal | baseline/min |\n| --- | --- | ---: |\n");
    for r in &every_tick {
        md.push_str(&format!(
            "| {} | `{}` | {:.1} |\n",
            r.kind.label(),
            cell(&r.name),
            r.baseline_per_minute
        ));
    }
    md.push_str("\n## Errors (packet and service errors, panics)\n\n| # | error | credited actions | hits | e.g. credited to |\n| ---: | --- | ---: | ---: | --- |\n");
    for (i, r) in errors.iter().enumerate() {
        let examples: Vec<String> = r
            .credited
            .iter()
            .take(3)
            .map(|(t, a)| format!("{t}: {a}"))
            .collect();
        md.push_str(&format!(
            "| {} | `{}` | {} | {} | {} |\n",
            i + 1,
            cell(&r.name),
            r.credited.len(),
            r.hits,
            cell(&examples.join("; "))
        ));
    }
    fs::write(output.join("report.md"), md)?;
    println!(
        "Census report: {} ({} refusals, {} every tick, {} errors); JSON: {}",
        output.join("report.md").display(),
        ranked.len(),
        every_tick.len(),
        errors.len(),
        output.join("report.json").display()
    );
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the census run is incomplete (see the report's warning):\n- {}",
            problems.join("\n- ")
        )
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_server::world::live::not_ported::{self, Kind as Logged};

    #[test]
    fn classifies_what_the_server_prints() {
        // The server's own lines, with and without the every-call count.
        assert_eq!(
            classify(&not_ported::line(
                Logged::Skipped,
                "Shops.openShop (content)",
                Some(3)
            )),
            Some((Kind::Skipped, "Shops.openShop (content)".to_owned()))
        );
        assert_eq!(
            classify(&not_ported::line(
                Logged::Default,
                "quest requirements",
                None
            )),
            Some((Kind::EmptyAnswer, "quest requirements".to_owned()))
        );
        assert_eq!(
            classify(
                "[live] PacketProcessor Err Error Processing IfAction: Exception(\"Binding(\\\"not ported in the live world: global handler service open_interface\\\")\")"
            ),
            Some((
                Kind::Refused,
                "IfAction: not ported in the live world: global handler service open_interface"
                    .to_owned()
            ))
        );
        assert_eq!(
            classify(
                r#"[live] deferred walk service failed: Binding("Binding(\"Binding(Some(\\\"\\\\\\\"not ported in the live world: openGrandExchangeCollectionBox (the Grand Exchange is not installed)\\\\\\\"\\\"))\")")"#
            ),
            Some((
                Kind::Refused,
                "deferred walk service: not ported in the live world: openGrandExchangeCollectionBox (the Grand Exchange is not installed)".to_owned()
            ))
        );
        assert_eq!(
            classify(
                "[live] PacketProcessor Err Error Processing NpcAction: index 42 out of range"
            ),
            Some((
                Kind::Error,
                "Error Processing NpcAction: index # out of range".to_owned()
            ))
        );
        // Lines that only mention an error, and the start-up summary, are not hits.
        assert_eq!(classify("Native world listening on TCP 127.0.0.1:1"), None);
        assert_eq!(classify("[live] loaded error_messages.json"), None);
        assert_eq!(
            classify(
                "World booted in 12 ms: start-up complete = false, 3 steps partly ported or refused"
            ),
            None
        );
        assert_eq!(strip_count("x [#12]"), "x");
        assert_eq!(strip_count("x [#a]"), "x [#a]");
        assert_eq!(strip_count("x [#]"), "x [#]");
    }

    #[test]
    fn credits_only_hits_beyond_the_baseline() {
        assert!(credited(1, 0.0));
        assert!(!credited(0, 0.0));
        // A baseline expecting 0.13 hits in the action: one hit is noise.
        assert!(!credited(1, 0.13));
        assert!(credited(3, 0.13));
        assert!(!credited(18, 8.0));
        assert!(credited(20, 8.0));
        // A high, NPC-driven rate: a few percent more is not credit.
        assert!(!credited(5_000, 4_800.0));
        assert!(credited(8_000, 4_800.0));
    }

    #[test]
    fn saves_and_loads_the_lines() {
        let dir = std::env::temp_dir().join(format!("census-lines-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut lines = Lines::default();
        lines.push(
            5,
            "stderr",
            &not_ported::line(Logged::Skipped, "a", Some(1)),
        );
        lines.push(6, "stdout", "plain");
        lines.push(
            7,
            "stderr",
            &not_ported::line(Logged::Skipped, "a", Some(2)),
        );
        lines.save(&dir).unwrap();
        let back = Lines::load(&dir).unwrap();
        assert_eq!(back.texts.len(), 2);
        assert_eq!(back.events, vec![(5, 0, 0), (6, 1, 1), (7, 0, 0)]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_spawn_rows() {
        assert_eq!(
            rows("{3221,3218,0,1,0}-{1,2,3,4,5}-"),
            vec![vec![3221, 3218, 0, 1, 0], vec![1, 2, 3, 4, 5]]
        );
    }
}
