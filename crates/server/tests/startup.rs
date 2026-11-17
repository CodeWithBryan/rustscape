//! The native start-up against the original's (`cargo xtask startup-loaders-oracle`).
//!
//! `fixtures/startup-loaders` holds the original start-up run one statement
//! of `GameWorld.prompt` at a time (its boot record is byte-identical to the
//! world-trace fixture's): generator positions and log lines per phase, the
//! tables of the small loaders after their phase, and those loaders again over
//! authored inputs. The portable tests below need only the fixtures. The
//! whole-start-up comparison also needs the pinned cache and data directory
//! the generator extracts (runs/startup-loaders-oracle/template, or
//! `RS09_STARTUP_WORLD`); without them it says so and checks nothing.
use rs09_game_core::java_double;
use rs09_server::definitions::{property::Property, scenery};
use rs09_server::startup::{
    generators::{CountingRandom, Seeds, WorldGenerators},
    loaders::{
        Log, clue_rewards, clue_rewards::ClueRewards, music, object_config, object_override,
        ranged, ranged::RangedTables, shops, xtea, xtea::RegionKeys,
    },
    regions::{Regions, ZoneBorders},
    sequence::{self, Inputs, ORDER, Outcome, Policy, Step},
};
use rs09_server::world::drop_tables::{
    LogLevel, NpcDropTable, WeightBasedTable, parser as drop_parser, random::Source,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

const ORACLE: &str = include_str!("../../../fixtures/startup-loaders/oracle.jsonl");
const CASES: &str = include_str!("../../../fixtures/startup-loaders/cases.json");
const PROVENANCE: &str = include_str!("../../../fixtures/startup-loaders/provenance.json");
const TRACE: &str = include_str!("../../../fixtures/world-trace/trace.jsonl");
const TRACE_PROVENANCE: &str = include_str!("../../../fixtures/world-trace/provenance.json");
const DROP_TABLES: &str = include_str!("../../../fixtures/drop-tables/drop_tables.json");

fn sha256(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn records() -> Vec<Value> {
    ORACLE
        .lines()
        .map(|l| serde_json::from_str(l).expect("fixture line is JSON"))
        .collect()
}

fn phases() -> Vec<Value> {
    records()
        .into_iter()
        .filter(|r| r["t"] == "phase")
        .collect()
}

fn table(loader: &str) -> Value {
    records()
        .into_iter()
        .find(|r| r["t"] == "table" && r["loader"] == loader)
        .unwrap_or_else(|| panic!("no table for {loader}"))
}

fn boot() -> Value {
    serde_json::from_str(TRACE.lines().nth(1).unwrap()).unwrap()
}

/// Shared-generator draws the original made before the named phase.
fn shared_draws_before(name: &str) -> u64 {
    let mut before = 0;
    for phase in phases() {
        if phase["name"] == name {
            return before;
        }
        before = phase["generators"]["shared"][0].as_u64().unwrap();
    }
    panic!("no phase {name}")
}

fn advanced(draws: u64) -> CountingRandom {
    let mut g = CountingRandom::new(0);
    for _ in 0..draws {
        g.next(32);
    }
    g
}

#[derive(Default)]
struct Lines(Vec<String>);
impl Log for Lines {
    fn log(&mut self, origin: &'static str, level: LogLevel, message: &str) {
        // The original prints FINE/INFO/WARN to standard output; ERR goes to
        // standard error, which the fixture does not keep.
        if level != LogLevel::Err {
            self.0.push(format!("[{origin}] {message}"));
        }
    }
}

/// The shared generator alone, as `RandomFunction.RANDOM`; the loaders below
/// draw nothing else.
struct Shared(CountingRandom);
impl Source for Shared {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0.next_bound(bound).unwrap()
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_bool()
    }
    fn next_double(
        &mut self,
        _: f64,
    ) -> Result<f64, rs09_server::world::drop_tables::random::BadRange> {
        panic!("thread-local draw")
    }
    fn shuffle_int(&mut self, _: i32) -> i32 {
        panic!("shuffle draw")
    }
}

// ---- the oracle's dump format ----

fn xteas_json(keys: &RegionKeys) -> Value {
    let mut rows: Vec<_> = keys.iter().collect();
    rows.sort_by_key(|(id, _)| *id);
    json!({"size": keys.len(), "keys": rows.iter().map(|(id, k)| json!([id, k[0], k[1], k[2], k[3]])).collect::<Vec<_>>()})
}

fn property_text(p: &Property) -> Value {
    match p {
        Property::Text(s) => json!(s),
        Property::Int(i) => json!(i.to_string()),
        other => json!(format!("{other:?}")),
    }
}

fn scenery_json(registry: &scenery::Registry, ids: &[i64], base: usize) -> Value {
    let ids: BTreeSet<i64> = ids.iter().copied().collect();
    let rows: Vec<Value> = ids
        .iter()
        .map(|&id| match registry.definitions.get(&(id as i32)) {
            None => json!([id, false, null, null, null]),
            Some(def) => {
                let def = def.read();
                let handlers = def.handlers.read();
                json!([
                    id,
                    true,
                    handlers.get("examine").map(property_text),
                    handlers.keys().collect::<Vec<_>>(),
                    def.animation_id
                ])
            }
        })
        .collect();
    json!({"definitions": registry.definitions.len() + base, "rows": rows})
}

fn borders_json(b: &ZoneBorders) -> Value {
    json!({
        "box": [b.south_west_x, b.south_west_y, b.north_east_x, b.north_east_y, b.plane],
        "exceptions": b.exceptions.as_ref().map(|e| e.iter().map(borders_json).collect::<Vec<_>>()),
    })
}

fn regions_json(regions: &Regions) -> Value {
    let mut zones: Vec<Value> = Vec::new();
    let mut seen: Vec<*const music::MusicEntry> = Vec::new();
    let _ = &mut seen;
    let mut zone_ptrs: Vec<*const rs09_server::startup::regions::MusicZone> = Vec::new();
    let mut rows = Vec::new();
    for id in regions.ids() {
        let r = regions.get(id).unwrap();
        if r.add_sceneries.is_empty()
            && r.remove_sceneries.is_empty()
            && r.music == -1
            && r.music_zones.is_empty()
        {
            continue;
        }
        let o = |s: &rs09_server::startup::regions::SceneryOverride| {
            json!([
                s.id,
                s.location[0],
                s.location[1],
                s.location[2],
                s.kind,
                s.rotation
            ])
        };
        let mut z = Vec::new();
        for zone in &r.music_zones {
            let ptr = Arc::as_ptr(zone);
            let n = match zone_ptrs.iter().position(|p| *p == ptr) {
                Some(n) => n,
                None => {
                    zone_ptrs.push(ptr);
                    zones.push(
                        json!({"music": zone.music_id, "borders": borders_json(&zone.borders)}),
                    );
                    zone_ptrs.len() - 1
                }
            };
            z.push(n);
        }
        rows.push(json!([
            id,
            r.add_sceneries.iter().map(o).collect::<Vec<_>>(),
            r.remove_sceneries.iter().map(o).collect::<Vec<_>>(),
            r.music,
            z
        ]));
    }
    let mut keys: Vec<String> = regions.ids().iter().map(i32::to_string).collect();
    keys.sort();
    json!({"regionCache": regions.len(), "regionKeysSha256": sha256(keys.join("\n")), "regions": rows, "zones": zones})
}

fn songs_json(songs: &music::Songs) -> Value {
    let mut rows: Vec<_> = songs.iter().collect();
    rows.sort_by_key(|(k, _)| **k);
    json!({"songs": rows.iter().map(|(k, e)| json!([k, e.id, e.name, e.index])).collect::<Vec<_>>()})
}

fn ranged_json(tables: &RangedTables) -> Value {
    let g = |g: &ranged::Graphics| json!([g.id, g.height, g.delay]);
    let mut ammo: Vec<_> = tables.ammunition.iter().collect();
    ammo.sort_by_key(|(k, _)| **k);
    let mut weapons: Vec<_> = tables.weapons.iter().collect();
    weapons.sort_by_key(|(k, _)| **k);
    json!({
        "ammunition": ammo.iter().map(|(k, a)| {
            let p = a.projectile;
            json!([k, a.item_id, g(&a.start_graphics), a.dark_bow_graphics.as_ref().map(g),
                [p.projectile_id, p.start_height, p.end_height, p.start_delay, p.speed, p.angle, p.distance],
                a.poison_damage, a.effect.map(|e| e.name())])
        }).collect::<Vec<_>>(),
        "weapons": weapons.iter().map(|(k, w)| json!([k, w.item_id, w.animation, w.attack_speed, w.ammunition_slot, w.weapon_type, w.drop_ammo, w.ammunition])).collect::<Vec<_>>(),
    })
}

fn weighted_json(t: &WeightBasedTable) -> Value {
    let row = |i: &rs09_server::world::drop_tables::WeightedItem| {
        json!([
            i.id,
            i.min_amt,
            i.max_amt,
            java_double::format(i.weight),
            i.guaranteed
        ])
    };
    json!({
        "items": t.items.iter().map(row).collect::<Vec<_>>(),
        "totalWeight": java_double::format(t.total_weight),
        "guaranteed": t.guaranteed_items.iter().map(row).collect::<Vec<_>>(),
    })
}

fn clues_json(c: &ClueRewards) -> Value {
    json!({"easy": weighted_json(&c.easy), "medium": weighted_json(&c.medium), "hard": weighted_json(&c.hard), "rare": weighted_json(&c.rare)})
}

/// The first differing path, for readable failures.
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                if let Some(d) = diff(
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    format!("{path}/{k}"),
                ) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (x, y))| diff(x, y, format!("{path}/{i}"))),
        _ => Some(format!("{path}: native {a} original {b}")),
    }
}

fn assert_same(native: &Value, original: &Value, what: &str) {
    if let Some(d) = diff(native, original, String::new()) {
        panic!("{what}: {d}");
    }
}

// ---- pins ----

#[test]
fn fixture_pins_and_boot_identity() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["schema"], 1);
    assert_eq!(
        provenance["fixtureSha256"],
        sha256(ORACLE.replace('\r', ""))
    );
    assert_eq!(provenance["casesSha256"], sha256(CASES.replace('\r', "")));
    assert_eq!(provenance["bootRecord"]["identicalToWorldTrace"], true);
    assert_eq!(provenance["bootRecord"]["sha256"], boot()["sha256"]);
    assert_eq!(
        provenance["bootRecord"]["worldTraceFixtureSha256"],
        sha256(TRACE.replace('\r', ""))
    );
    let world: Value = serde_json::from_str(TRACE_PROVENANCE).unwrap();
    for (name, digest) in provenance["inputs"].as_object().unwrap() {
        let bytes = std::fs::read(root().join("fixtures/startup-loaders").join(name)).unwrap();
        assert_eq!(&json!(sha256(&bytes)), digest, "{name}");
        assert_eq!(
            &world["data"][format!("data/configs/{name}")],
            digest,
            "{name} is not the pinned file"
        );
    }
    assert_eq!(
        world["data"]["data/configs/drop_tables.json"],
        json!(sha256(DROP_TABLES.replace('\r', "")))
    );
    // The phases add up to the boot record's generator positions, except the
    // thread-local seed: writing the boot record first runs the one block the
    // start-up queued on the dispatcher (handoffs.blocks = 1), which draws it.
    let mut last = phases().last().unwrap()["generators"].clone();
    let mut theirs = boot()["generators"].clone();
    assert_eq!(boot()["handoffs"]["blocks"], 1);
    assert_ne!(last["threadLocal"], theirs["threadLocal"]);
    last.as_object_mut().unwrap().remove("threadLocal");
    theirs.as_object_mut().unwrap().remove("threadLocal");
    assert_eq!(last, theirs);
    assert_eq!(phases().len(), 52);
}

// ---- the loaders over authored inputs ----

#[test]
fn loader_cases_match_the_original() {
    let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
    let observed: Vec<Value> = records().into_iter().filter(|r| r["t"] == "case").collect();
    assert_eq!(cases.len(), observed.len());
    assert_eq!(cases.len(), 47);
    let boot = boot();
    let scenery_base =
        boot["registries"]["core.cache.def.impl.SceneryDefinition"]["DEFINITIONS"]["size"]
            .as_u64()
            .unwrap() as usize;
    let mut definitions = scenery::Registry::default();
    let mut regions = Regions::default();
    let items = rs09_server::definitions::item::Registry::default();
    let mut clues = ClueRewards::default();
    // The shared generator after start-up: 23,041 draws from seed 0.
    let mut random = Shared(advanced(boot["generators"]["shared"][0].as_u64().unwrap()));
    for (case, original) in cases.iter().zip(&observed) {
        let name = case["name"].as_str().unwrap();
        assert_eq!(original["name"], case["name"]);
        let dir =
            std::env::temp_dir().join(format!("rs09-startup-case-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, text) in case["files"].as_object().unwrap() {
            std::fs::write(dir.join(file), text.as_str().unwrap()).unwrap();
        }
        let prefix = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
        let mut log = Lines::default();
        let before = random.0.draws();
        let (error, state) = match case["loader"].as_str().unwrap() {
            "XteaParser" => {
                let mut keys = RegionKeys::default();
                let r = xtea::load_config(Some(&prefix), &mut keys, &mut log);
                (r.err(), xteas_json(&keys))
            }
            "ObjectConfigParser" => {
                let r = object_config::load_config(Some(&prefix), &mut definitions, &mut log);
                let ids: Vec<i64> = case["dump"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap())
                    .collect();
                (r.err(), scenery_json(&definitions, &ids, scenery_base))
            }
            "ObjectOverrideParser" => {
                regions.clear_configuration();
                let r = object_override::load_config(
                    Some(&prefix),
                    &mut definitions,
                    &mut regions,
                    &mut log,
                );
                (r.err(), regions_json(&regions))
            }
            "ShopParser" => (
                shops::load_config(Some(&prefix), &mut log).err(),
                Value::Null,
            ),
            "MusicConfigLoader" => {
                regions.clear_configuration();
                // The song half reads the cache's data maps; its pinned result is
                // compared by the whole-start-up test. Cases cover the files.
                let r = music::load_files(Some(&prefix), &mut regions, &mut log);
                (r.err(), regions_json(&regions))
            }
            "RangedConfigLoader" => {
                let mut ranged_tables = RangedTables::default();
                let r = ranged::load_config(Some(&prefix), &items, &mut ranged_tables, &mut log);
                (r.err(), ranged_json(&ranged_tables))
            }
            "ClueRewardParser" => {
                let r = clue_rewards::load_config(Some(&prefix), &mut clues, &mut random);
                (r.err(), clues_json(&clues))
            }
            other => panic!("{other}"),
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            json!(error.as_ref().map(|e| e.java_class())),
            original["error"],
            "{name}: exception"
        );
        assert_eq!(
            json!([random.0.draws() - before, 0, 0]),
            original["draws"],
            "{name}: draws"
        );
        assert_eq!(json!(log.0), original["log"], "{name}: log");
        let mut state = state;
        let mut theirs = original["state"].clone();
        if case["loader"] == "ObjectOverrideParser" || case["loader"] == "MusicConfigLoader" {
            // Regions created by the start-up before these cases are not in
            // this native table; the configured rows are compared.
            for v in [&mut state, &mut theirs] {
                v.as_object_mut().unwrap().remove("regionCache");
                v.as_object_mut().unwrap().remove("regionKeysSha256");
            }
        }
        assert_same(&state, &theirs, name);
    }
}

// ---- the pinned tables, portable part ----

fn fixture_prefix() -> String {
    format!(
        "{}{}",
        root().join("fixtures/startup-loaders").display(),
        std::path::MAIN_SEPARATOR
    )
}

#[test]
fn pinned_xteas_match_the_original() {
    let mut keys = RegionKeys::default();
    let mut log = Lines::default();
    assert_eq!(
        xtea::load_config(Some(&fixture_prefix()), &mut keys, &mut log).unwrap(),
        1122
    );
    let mut original = table("XteaParser");
    original
        .as_object_mut()
        .unwrap()
        .retain(|k, _| k == "size" || k == "keys");
    assert_same(&xteas_json(&keys), &original, "XteaParser");
    let phase = phases()
        .into_iter()
        .find(|p| p["name"] == "XteaParser")
        .unwrap();
    assert_eq!(json!(log.0), phase["log"]);
}

#[test]
fn pinned_overrides_match_the_original() {
    let mut definitions = scenery::Registry::default();
    let mut regions = Regions::default();
    let mut log = Lines::default();
    assert_eq!(
        object_override::load_config(
            Some(&fixture_prefix()),
            &mut definitions,
            &mut regions,
            &mut log
        )
        .unwrap(),
        26
    );
    // Before this loader the original's REGION_CACHE is empty, so the region
    // count and key digest are comparable too.
    let mut original = table("ObjectOverrideParser");
    original
        .as_object_mut()
        .unwrap()
        .retain(|k, _| k != "t" && k != "loader");
    assert_same(&regions_json(&regions), &original, "ObjectOverrideParser");
}

#[test]
fn pinned_shop_count_matches_the_original() {
    let mut log = Lines::default();
    assert_eq!(
        shops::load_config(Some(&fixture_prefix()), &mut log).unwrap(),
        245
    );
    let phase = phases()
        .into_iter()
        .find(|p| p["name"] == "ShopParser")
        .unwrap();
    assert_eq!(json!(log.0), phase["log"]);
}

#[test]
fn pinned_clue_rewards_match_at_the_original_generator_position() {
    let draws = shared_draws_before("ClueRewardParser");
    assert_eq!(draws, 22_636);
    let mut random = Shared(advanced(draws));
    let mut clues = ClueRewards::default();
    clue_rewards::load_config(Some(&fixture_prefix()), &mut clues, &mut random).unwrap();
    let mut original = table("ClueRewardParser");
    original
        .as_object_mut()
        .unwrap()
        .retain(|k, _| k != "t" && k != "loader");
    assert_same(&clues_json(&clues), &original, "ClueRewardParser");
    let phase = phases()
        .into_iter()
        .find(|p| p["name"] == "ClueRewardParser")
        .unwrap();
    assert_eq!(
        json!([random.0.draws(), random.0.state_hex()]),
        phase["generators"]["shared"]
    );
}

/// Every NPC id has drop tables here; the draws do not depend on that.
struct AllDefinitions;
impl drop_parser::Definitions for AllDefinitions {
    fn assign(&mut self, _: i32, _: &Arc<NpcDropTable>) -> Result<bool, drop_parser::LoadError> {
        Ok(true)
    }
    fn name(&mut self, _: i32) -> Result<Option<String>, drop_parser::LoadError> {
        Ok(None)
    }
    fn log(&mut self, _: LogLevel, _: &str) {}
}

#[test]
fn drop_table_draws_match_at_the_original_generator_position() {
    let draws = shared_draws_before("DropTableParser");
    assert_eq!(draws, 378);
    let mut random = Shared(advanced(draws));
    assert_eq!(
        drop_parser::load(DROP_TABLES, &mut random, &mut AllDefinitions).unwrap(),
        2152
    );
    let phase = phases()
        .into_iter()
        .find(|p| p["name"] == "DropTableParser")
        .unwrap();
    assert_eq!(
        json!([random.0.draws(), random.0.state_hex()]),
        phase["generators"]["shared"]
    );
}

#[test]
fn world_generators_report_positions_like_the_oracle() {
    let generators = WorldGenerators::new(Seeds {
        shared: 0,
        math: 0,
        shuffle: 0,
        thread_local: 537_462_064,
    });
    let p = generators.positions();
    let first = &phases()[0]["generators"];
    assert_eq!(json!([p.shared.0, p.shared.1]), first["shared"]);
    assert_eq!(json!([p.math.0, p.math.1]), first["math"]);
    assert_eq!(json!([p.shuffle.0, p.shuffle.1]), first["shuffle"]);
    assert_eq!(json!(p.thread_local), first["threadLocal"]);
}

// ---- the whole start-up, with the pinned cache and data ----

/// The template directory, verified file by file against the world-trace
/// provenance. `None` when it has not been extracted.
fn pinned_world() -> Option<PathBuf> {
    let dir = std::env::var_os("RS09_STARTUP_WORLD")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("runs/startup-loaders-oracle/template"));
    if !dir.join("worldprops/default.conf").is_file() {
        return None;
    }
    let world: Value = serde_json::from_str(TRACE_PROVENANCE).unwrap();
    for (name, digest) in world["data"].as_object().unwrap() {
        let bytes = std::fs::read(dir.join(name))
            .unwrap_or_else(|_| panic!("{name} missing from {}", dir.display()));
        assert_eq!(
            &json!(sha256(&bytes)),
            digest,
            "{name} differs from the pinned data"
        );
    }
    let conf = std::fs::read(dir.join("worldprops/default.conf")).unwrap();
    assert_eq!(json!(sha256(&conf)), world["config"]["usedSha256"]);
    Some(dir)
}

fn sorted_keys_digest(keys: impl Iterator<Item = i32>) -> String {
    let mut keys: Vec<String> = keys.map(|k| k.to_string()).collect();
    keys.sort();
    sha256(keys.join("\n"))
}

/// Fields of the boot record (and of the phase records) the native start-up
/// does not reproduce yet, each with its cause. The test fails if a field
/// listed here starts matching or a field not listed stops matching.
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "draws-refused:ClassScanner.loadPureInterfaces",
        "content interfaces are not ported: 328 shared, 2,336 shuffle and thread-local draws",
    ),
    (
        "draws-refused:ClassScanner.loadSideEffectfulPlugins",
        "legacy plugins are not ported: 50 shared, 14 math, 6 shuffle and thread-local draws",
    ),
    (
        "draws-refused:NPCSpawner",
        "NPC.create/init is not joined: 13,846 shared draws",
    ),
    (
        "draws-refused:startup:",
        "start-up listeners are content: RareDropTable 27, PenguinHNSEvent 10, SpotManager 10 shared draws and thread-local draws",
    ),
    (
        "generators.shared",
        "8,770 native draws (DropTableParser 8,412 + ClueRewardParser 358) against 23,041: the refused steps' 14,271",
    ),
    (
        "generators.math",
        "the 14 Math.random draws are in legacy plugins",
    ),
    (
        "generators.shuffle",
        "the 2,342 shuffle draws are in content interfaces and plugins",
    ),
    (
        "generators.threadLocal",
        "thread-local draws are in content, plugins, two start-up listeners and the dispatcher block run when the boot record is written",
    ),
    ("npcs.count", "NPCSpawner and content spawns are refused"),
    (
        "tables.npcDefinitions",
        "183 more NPC definitions are looked up by refused steps (spawns, content)",
    ),
    (
        "registries.core.cache.def.impl.NPCDefinition.DEFINITIONS",
        "as tables.npcDefinitions",
    ),
    (
        "tables.sceneryDefinitions",
        "3 more scenery definitions are created by refused steps",
    ),
    (
        "registries.core.cache.def.impl.SceneryDefinition.DEFINITIONS",
        "as tables.sceneryDefinitions",
    ),
    (
        "registries.core.game.component.ComponentDefinition.DEFINITIONS",
        "91 component definitions come from content, not InterfaceConfigParser",
    ),
    (
        "registries.core.game.node.item.GroundItemManager.GROUND_ITEMS",
        "GroundSpawnLoader is refused",
    ),
    (
        "registries.core.game.world.map.RegionManager.REGION_CACHE",
        "331 regions are created by refused steps (NPC and ground spawns, content)",
    ),
    (
        "table:MusicConfigLoader.regionCache",
        "300 of those regions exist before the music loader (NPCSpawner, GroundSpawnLoader)",
    ),
    ("tables.contentInterfaces", "class scan refused"),
    ("tables.legacyPlugins", "plugins refused"),
    (
        "tables.pulseClasses",
        "the world pulses are submitted by content",
    ),
    (
        "tables.timerTypes",
        "timer classes come from the class scan",
    ),
    (
        "table:ClueRewardParser",
        "built at shared draw 8,412 natively, 22,636 in the original: the refused steps' draws move every random insertion (the same holds for the drop tables, whose draw count matches by chance)",
    ),
];

/// Boot-record fields with no native counterpart in this World; they are
/// listed, not compared.
const UNCOMPARED: &[&str] = &[
    "listeners (login, logout, tick, startup, shutdown, persistence, login plugins, timers)",
    "scan (content interfaces, Initializable plugins, timers)",
    "the other registries (activities, dialogues, interaction/interface listeners, plugin interactions, AbstractNPC, NPCBehavior, quests, shops, commands, timers, world events, option handlers, Entrana lists, clipping/projectile flags)",
    "npcs.list/indexSha256/stateSha256/renderable",
    "pulses, systemState, threads, files (SQLite databases content creates)",
    "generators.clock/clockReads/seeded",
];

#[test]
fn native_startup_against_the_boot_record() {
    let Some(world) = pinned_world() else {
        eprintln!(
            "SKIPPED: no pinned world directory (run cargo xtask startup-loaders-oracle); nothing was compared"
        );
        return;
    };
    let seeds = Seeds {
        shared: 0,
        math: 0,
        shuffle: 0,
        thread_local: 537_462_064,
    };
    let inputs = Inputs {
        world,
        seeds,
        max_definitions: 1_000_000,
    };
    assert!(matches!(
        sequence::start(&inputs, Policy::StopAtRefusal),
        Err(sequence::StartupError::NotPorted {
            step: Step::ChunkDirtyListener,
            ..
        })
    ));
    let startup =
        sequence::start(&inputs, Policy::ContinuePastRefusals).expect("native steps succeed");
    assert!(!startup.complete);
    let boot = boot();
    let phases = phases();
    let mut matched: Vec<String> = Vec::new();
    let mut gaps: BTreeMap<String, String> = BTreeMap::new();
    let mut check = |field: &str, native: Value, original: Value| {
        if native == original {
            matched.push(field.into());
        } else {
            gaps.insert(field.into(), format!("native {native} original {original}"));
        }
    };

    // Order: the native steps follow the original's phases.
    let mut original_order: Vec<String> = Vec::new();
    for p in &phases {
        let name = p["name"].as_str().unwrap();
        let name = if name.starts_with("startup:") {
            "startup:"
        } else {
            name
        };
        if original_order.last().map(String::as_str) != Some(name) || name != "startup:" {
            original_order.push(name.into());
        }
    }
    let native_order: Vec<String> = ORDER
        .iter()
        .filter(|s| **s != Step::PreloadMap)
        .map(|s| s.phase().to_string())
        .collect();
    check("order", json!(native_order), json!(original_order));

    // Per native step: its log and its own draws.
    let mut previous = (0u64, 0u64, 0u64);
    let mut original_previous = (0u64, 0u64, 0u64);
    let mut index = 0;
    for report in &startup.steps {
        let p = &report.positions;
        let native_delta = (
            p.shared.0 - previous.0,
            p.math.0 - previous.1,
            p.shuffle.0 - previous.2,
        );
        previous = (p.shared.0, p.math.0, p.shuffle.0);
        if report.step == Step::PreloadMap {
            continue;
        }
        let mut original_delta = (0, 0, 0);
        let mut original_log: Vec<Value> = Vec::new();
        while index < phases.len() {
            let ph = &phases[index];
            let name = ph["name"].as_str().unwrap();
            let same = if report.step == Step::StartupListeners {
                name.starts_with("startup:")
            } else {
                name == report.step.phase()
            };
            if !same {
                break;
            }
            let g = &ph["generators"];
            let now = (
                g["shared"][0].as_u64().unwrap(),
                g["math"][0].as_u64().unwrap(),
                g["shuffle"][0].as_u64().unwrap(),
            );
            original_delta = (
                original_delta.0 + now.0 - original_previous.0,
                original_delta.1 + now.1 - original_previous.1,
                original_delta.2 + now.2 - original_previous.2,
            );
            original_previous = now;
            original_log.extend(ph["log"].as_array().unwrap().iter().cloned());
            index += 1;
        }
        let name = report.step.phase();
        if report.outcome == Outcome::Native {
            check(
                &format!("draws:{name}"),
                json!([native_delta.0, native_delta.1, native_delta.2]),
                json!([original_delta.0, original_delta.1, original_delta.2]),
            );
            if report.step != Step::ServerConfig {
                check(
                    &format!("log:{name}"),
                    json!(report.log),
                    json!(original_log),
                );
            }
        } else {
            // A refused step draws nothing natively; zero against zero only
            // says the original drew nothing there either.
            check(
                &format!("draws-refused:{name}"),
                json!([native_delta.0, native_delta.1, native_delta.2]),
                json!([original_delta.0, original_delta.1, original_delta.2]),
            );
        }
    }

    let w = &startup.world;
    // Tables after their own loader.
    let configs = table("ObjectConfigParser");
    let ids: Vec<i64> = configs["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[0].as_i64().unwrap())
        .collect();
    let native_configs = scenery_json(&w.scenery, &ids, 0);
    check(
        "table:ObjectConfigParser.rows",
        native_configs["rows"].clone(),
        configs["rows"].clone(),
    );
    let x = table("XteaParser");
    check(
        "table:XteaParser",
        xteas_json(&w.region_keys)["keys"].clone(),
        x["keys"].clone(),
    );
    let music_table = table("MusicConfigLoader");
    let native_regions = regions_json(&w.regions);
    check(
        "table:MusicConfigLoader.regions",
        native_regions["regions"].clone(),
        music_table["regions"].clone(),
    );
    check(
        "table:MusicConfigLoader.zones",
        native_regions["zones"].clone(),
        music_table["zones"].clone(),
    );
    check(
        "table:MusicConfigLoader.regionCache",
        native_regions["regionCache"].clone(),
        music_table["regionCache"].clone(),
    );
    check(
        "table:MusicConfigLoader.songs",
        songs_json(&w.songs)["songs"].clone(),
        music_table["songs"].clone(),
    );
    let r = table("RangedConfigLoader");
    let native_ranged = ranged_json(&w.ranged);
    check(
        "table:RangedConfigLoader.ammunition",
        native_ranged["ammunition"].clone(),
        r["ammunition"].clone(),
    );
    check(
        "table:RangedConfigLoader.weapons",
        native_ranged["weapons"].clone(),
        r["weapons"].clone(),
    );
    // The stored order of a weighted table depends on the shared generator's
    // position when it is built (see the isolated test above, which matches).
    let mut clue = table("ClueRewardParser");
    clue.as_object_mut()
        .unwrap()
        .retain(|k, _| k != "t" && k != "loader");
    check("table:ClueRewardParser", clues_json(&w.clue_rewards), clue);

    // The boot record.
    let t = &boot["tables"];
    check(
        "tables.itemDefinitions",
        json!(w.items.definitions().len()),
        t["itemDefinitions"].clone(),
    );
    check(
        "tables.npcDefinitions",
        json!(w.npcs.definitions.len()),
        t["npcDefinitions"].clone(),
    );
    check(
        "tables.sceneryDefinitions",
        json!(w.scenery.definitions.len()),
        t["sceneryDefinitions"].clone(),
    );
    check(
        "tables.contentInterfaces",
        json!(0),
        t["contentInterfaces"].clone(),
    );
    check("tables.legacyPlugins", json!(0), t["legacyPlugins"].clone());
    check("tables.timerTypes", json!([]), t["timerTypes"].clone());
    check("tables.pulseClasses", json!([]), t["pulseClasses"].clone());
    let reg = &boot["registries"];
    let registry = |class: &str, field: &str, size: usize, digest: Option<String>| {
        let o = &reg[class][field];
        (
            json!({"size": size, "keysSha256": digest}),
            json!({"size": o["size"], "keysSha256": o.get("keysSha256")}),
        )
    };
    let mut registries = vec![
        (
            "core.cache.def.impl.ItemDefinition",
            "DEFINITIONS",
            w.items.definitions().len(),
            Some(sorted_keys_digest(w.items.definitions().keys().copied())),
        ),
        (
            "core.cache.def.impl.NPCDefinition",
            "DEFINITIONS",
            w.npcs.definitions.len(),
            Some(sorted_keys_digest(w.npcs.definitions.keys().copied())),
        ),
        (
            "core.cache.def.impl.SceneryDefinition",
            "DEFINITIONS",
            w.scenery.definitions.len(),
            Some(sorted_keys_digest(w.scenery.definitions.keys().copied())),
        ),
        (
            "core.cache.def.impl.VarbitDefinition",
            "MAPPING",
            w.varbits.mapping().read().len(),
            Some(sorted_keys_digest(
                w.varbits.mapping().read().keys().copied(),
            )),
        ),
        (
            "core.game.component.ComponentDefinition",
            "DEFINITIONS",
            w.interfaces.definitions.len(),
            Some(sorted_keys_digest(w.interfaces.definitions.keys().copied())),
        ),
        (
            "core.game.node.entity.combat.equipment.RangeWeapon",
            "RANGE_WEAPONS",
            w.ranged.weapons.len(),
            Some(sorted_keys_digest(w.ranged.weapons.keys().copied())),
        ),
        (
            "core.game.node.entity.player.link.music.MusicEntry",
            "SONGS",
            w.songs.len(),
            Some(sorted_keys_digest(w.songs.iter().map(|(k, _)| *k))),
        ),
        (
            "core.game.world.map.RegionManager",
            "REGION_CACHE",
            w.regions.len(),
            Some(sorted_keys_digest(w.regions.ids().into_iter())),
        ),
        (
            "core.game.node.item.GroundItemManager",
            "GROUND_ITEMS",
            0,
            None,
        ),
    ];
    for (class, field, size, digest) in registries.drain(..) {
        let (native, original) = registry(class, field, size, digest);
        check(&format!("registries.{class}.{field}"), native, original);
    }
    check("npcs.count", json!(0), boot["npcs"]["count"].clone());
    let g = &boot["generators"];
    let p = w.generators.positions();
    check(
        "generators.shared",
        json!([p.shared.0, p.shared.1]),
        g["shared"].clone(),
    );
    check(
        "generators.math",
        json!([p.math.0, p.math.1]),
        g["math"].clone(),
    );
    check(
        "generators.shuffle",
        json!([p.shuffle.0, p.shuffle.1]),
        g["shuffle"].clone(),
    );
    check(
        "generators.threadLocal",
        json!(p.thread_local),
        g["threadLocal"].clone(),
    );

    eprintln!("matched {} fields:", matched.len());
    for m in &matched {
        eprintln!("  = {m}");
    }
    eprintln!("{} known gaps:", gaps.len());
    for (field, detail) in &gaps {
        let detail: String = detail.chars().take(240).collect();
        eprintln!("  ! {field}: {detail}");
    }
    eprintln!("not compared (no native counterpart):");
    for u in UNCOMPARED {
        eprintln!("  - {u}");
    }
    let listed: BTreeSet<&str> = KNOWN_GAPS.iter().map(|(f, _)| *f).collect();
    let found: BTreeSet<&str> = gaps.keys().map(String::as_str).collect();
    assert_eq!(
        found, listed,
        "the mismatching fields differ from KNOWN_GAPS"
    );
}
