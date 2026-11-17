//! Native replay boundary for the player save corpus. Production code performs
//! every save and load; this file scripts the same inputs as Support.kt, stands
//! in for the unported systems behind `save::Host` and observes native state.
#[allow(dead_code)]
#[path = "../inventory/listeners.rs"]
mod listeners;
#[allow(dead_code)]
#[path = "../timers/native.rs"]
mod timer_native;
use rs09_game_core::{
    java_double,
    java_json::{self, Json},
    java_map::int_key_order,
};
use rs09_server::{
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        container::{Container, ContainerType, Factory, SortType},
        item::Item,
    },
    player::{
        appearance::{Appearance, BodyPart, Factory as AppearanceFactory},
        attributes::{AttributeLocation, AttributeValue, GameAttributes},
        bank_pin,
        core_data::{CoreData, DEFAULT_BANK_SIZE},
        ironman,
        save::{
            self, ActiveFamiliar, Autocast, Category, Diary, Emotes, Env, Familiars, Host, House,
            Level, Parser, PetDetail, Profile, Quests, Room, SaveOutcome, Servant, parser,
        },
        saved_data::{ItemData, SavedData},
        settings::{self, Settings},
        skills::{self, Effect, EntityKind, Skills},
    },
    timers::Manager,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use timer_native::ScriptState;

fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn number(a: &Value, key: &str, default: i32) -> i32 {
    a.get(key)
        .and_then(Value::as_i64)
        .map_or(default, |v| v as i32)
}
fn double(value: &Value) -> f64 {
    java_double::parse(value.as_str().unwrap()).unwrap()
}
fn bits(value: f64) -> String {
    format!(
        "{:016x}",
        if value.is_nan() {
            f64::NAN.to_bits()
        } else {
            value.to_bits()
        }
    )
}
fn category(error: &save::Error) -> &'static str {
    match error.category() {
        Category::Null => "Null",
        Category::Type => "Type",
        Category::Number => "Number",
        Category::Index => "Index",
        Category::ConcurrentModification => "ConcurrentModification",
        Category::Binding => "Binding",
        Category::Parse => "Parse",
        Category::Argument => "Argument",
        Category::Io => "Io",
        Category::NativeLimit => "NativeLimit",
        Category::NegativeArraySize => "NegativeSize",
    }
}
/// JSONValue.toJSONString of one value.
fn json_string(value: &Json) -> String {
    let text = Json::Array(vec![value.clone()]).text();
    text[1..text.len() - 1].to_owned()
}
#[derive(Default)]
struct Recorder {
    trace: Vec<Value>,
    counts: BTreeMap<String, i32>,
    failures: BTreeMap<String, i32>,
    clock: i64,
    step: i64,
}
impl Recorder {
    fn record(&mut self, tag: &str, values: Vec<Value>) -> bool {
        let mut row = vec![json!(tag)];
        row.extend(values);
        self.trace.push(Value::Array(row));
        let count = self.counts.entry(tag.to_owned()).or_default();
        *count += 1;
        self.failures.get(tag) == Some(count)
    }
}
struct JavaDouble(f64);
impl std::fmt::Display for JavaDouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&java_double::format(self.0))
    }
}
/// A Java `HashMap<Integer, V>` that is only ever added to.
#[derive(Default)]
struct IntMap<V> {
    insertion: Vec<i32>,
    values: BTreeMap<i32, V>,
}
impl<V> IntMap<V> {
    fn put(&mut self, key: i32, value: V) {
        if !self.values.contains_key(&key) {
            self.insertion.push(key);
        }
        self.values.insert(key, value);
    }
    fn iter(&self) -> impl Iterator<Item = (i32, &V)> {
        int_key_order(self.insertion.iter().copied())
            .into_iter()
            .map(|key| (key, &self.values[&key]))
    }
}
enum FamiliarSpec {
    None,
    Pet(i32),
    Familiar {
        original_id: i32,
        ticks: i32,
        special_points: i32,
        lifepoints: i32,
        burden: Option<Container>,
    },
}
/// The unported systems, as scripted by the corpus.
struct World {
    recorder: Arc<Mutex<Recorder>>,
    retaliating: bool,
    combat: i32,
    summon: i32,
    quest_points: i32,
    quests: IntMap<i32>,
    spells: BTreeMap<(i32, i32), bool>,
    autocast: Option<(i32, i32)>,
    songs: BTreeMap<i32, i32>,
    unlocked: IntMap<i32>,
    emotes: Vec<i32>,
    diaries: Vec<Diary>,
    house: House,
    pets: IntMap<Vec<PetDetail>>,
    familiar: FamiliarSpec,
    pouches: Vec<(String, String)>,
    poh: String,
    hooks: Vec<(String, String)>,
}
const DIARY_TYPES: [&str; 6] = [
    "KARAMJA",
    "VARROCK",
    "LUMBRIDGE",
    "FALADOR",
    "FREMENNIK",
    "SEERS_VILLAGE",
];
impl World {
    fn new(recorder: Arc<Mutex<Recorder>>) -> Self {
        Self {
            recorder,
            retaliating: true,
            combat: 0,
            summon: 0,
            quest_points: 0,
            quests: IntMap::default(),
            spells: BTreeMap::new(),
            autocast: None,
            songs: BTreeMap::new(),
            unlocked: IntMap::default(),
            emotes: (0..22).collect(),
            diaries: Vec::new(),
            house: House::default(),
            pets: IntMap::default(),
            familiar: FamiliarSpec::None,
            pouches: vec![("pouches".into(), "[]".into())],
            poh: "{\"containers\":{}}".into(),
            hooks: Vec::new(),
        }
    }
    fn call(&mut self, tag: &str, values: Vec<Value>) -> save::Result<()> {
        if self.recorder.lock().unwrap().record(tag, values) {
            Err(save::Error::Host(tag.to_owned()))
        } else {
            Ok(())
        }
    }
    fn script(&mut self, w: &Value, definitions: &Registry) {
        if let Some(q) = w.get("quests").filter(|q| q.is_object()) {
            self.quest_points = int(&q["points"]);
            for row in q["list"].as_array().unwrap() {
                self.quests.put(int(&row[0]), int(&row[1]));
            }
        }
        for row in w["spells"].as_array().into_iter().flatten() {
            self.spells
                .insert((int(&row[0]), int(&row[1])), row[2] == true);
        }
        if let Some(a) = w.get("autocast") {
            self.autocast = a.as_array().map(|a| (int(&a[0]), int(&a[1])));
        }
        for row in w["songs"].as_array().into_iter().flatten() {
            self.songs.insert(int(&row[0]), int(&row[1]));
        }
        for id in w["unlocked"].as_array().into_iter().flatten() {
            let id = int(id);
            self.unlocked.put(self.songs[&id], id);
        }
        if let Some(emotes) = w["emotes"].as_array() {
            self.emotes = emotes.iter().map(int).collect();
        }
        if let Some(diaries) = w["diaries"].as_array() {
            let flags = |v: &Value| -> Vec<bool> {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|b| b.as_bool().unwrap())
                    .collect()
            };
            self.diaries = diaries
                .iter()
                .map(|d| Diary {
                    name: DIARY_TYPES[int(&d["type"]) as usize].to_owned(),
                    started: flags(&d["started"]),
                    completed: d["completed"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(flags)
                        .collect(),
                    rewarded: flags(&d["rewarded"]),
                })
                .collect();
        }
        if let Some(h) = w.get("house").filter(|h| h.is_object()) {
            let mut rooms = vec![vec![vec![None; 8]; 8]; 4];
            for r in h["rooms"].as_array().into_iter().flatten() {
                rooms[int(&r["z"]) as usize][int(&r["x"]) as usize][int(&r["y"]) as usize] =
                    Some(Room {
                        properties: int(&r["properties"]),
                        rotation: int(&r["rotation"]),
                        hotspots: r["hotspots"].as_array().unwrap().iter().map(int).collect(),
                    });
            }
            self.house = House {
                location: int(&h["location"]),
                style: int(&h["style"]),
                crest: int(&h["crest"]),
                servant: h.get("servant").filter(|s| s.is_object()).map(|s| Servant {
                    kind: int(&s["type"]),
                    uses: int(&s["uses"]),
                    item: s["item"].as_array().map(|i| (int(&i[0]), int(&i[1]))),
                    greet: s["greet"] == true,
                }),
                rooms,
            };
        }
        for row in w["pets"].as_array().into_iter().flatten() {
            let details = row[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| PetDetail {
                    hunger: double(&pair[0]),
                    growth: double(&pair[1]),
                })
                .collect();
            self.pets.put(int(&row[0]), details);
        }
        if let Some(d) = w.get("familiar") {
            self.familiar = if d.is_null() {
                FamiliarSpec::None
            } else if let Some(pet) = d.get("pet") {
                FamiliarSpec::Pet(int(pet))
            } else {
                let burden = d.get("burden").map(|rows| {
                    let container = Factory::new(definitions)
                        .create(30, ContainerType::Default, SortType::Id)
                        .unwrap();
                    for row in rows.as_array().unwrap() {
                        let mut item =
                            Item::new(int(&row[1]), int(&row[2]), int(&row[3]), definitions);
                        item.index = int(&row[0]);
                        container.items().write()[int(&row[0]) as usize] = Some(Shared::new(item));
                    }
                    container
                });
                FamiliarSpec::Familiar {
                    original_id: int(&d["originalId"]),
                    ticks: int(&d["ticks"]),
                    special_points: int(&d["specialPoints"]),
                    lifepoints: int(&d["lifepoints"]),
                    burden,
                }
            };
        }
        let pairs = |v: &Value| -> Vec<(String, String)> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    (
                        row[0].as_str().unwrap().to_owned(),
                        row[1].as_str().unwrap().to_owned(),
                    )
                })
                .collect()
        };
        if w["pouches"].is_array() {
            self.pouches = pairs(&w["pouches"]);
        }
        if let Some(poh) = w["poh"].as_str() {
            self.poh = poh.to_owned();
        }
        if w["hooks"].is_array() {
            self.hooks.extend(pairs(&w["hooks"]));
        }
    }
}
impl Host for World {
    fn log(&mut self, level: Level, message: &str) {
        let level = match level {
            Level::Warn => "WARN",
            Level::Err => "ERR",
        };
        self.recorder
            .lock()
            .unwrap()
            .record("log", vec![json!(level), json!(message)]);
    }
    fn retaliating(&mut self) -> save::Result<bool> {
        Ok(self.retaliating)
    }
    fn quests(&mut self) -> save::Result<Quests> {
        Ok(Quests {
            points: self.quest_points,
            stages: self.quests.iter().map(|(id, stage)| (id, *stage)).collect(),
        })
    }
    fn autocast(&mut self) -> save::Result<Option<Autocast>> {
        Ok(self
            .autocast
            .map(|(book, spell_id)| Autocast { book, spell_id }))
    }
    fn unlocked_music(&mut self) -> save::Result<Vec<i32>> {
        Ok(self.unlocked.iter().map(|(_, id)| *id).collect())
    }
    fn familiars(&mut self) -> save::Result<Familiars> {
        Ok(Familiars {
            pets: self
                .pets
                .iter()
                .map(|(id, details)| (id, details.clone()))
                .collect(),
            active: match &self.familiar {
                FamiliarSpec::None => ActiveFamiliar::None,
                FamiliarSpec::Pet(item_id) => ActiveFamiliar::Pet { item_id: *item_id },
                FamiliarSpec::Familiar {
                    original_id,
                    ticks,
                    special_points,
                    lifepoints,
                    burden,
                } => ActiveFamiliar::Familiar {
                    original_id: *original_id,
                    ticks: *ticks,
                    special_points: *special_points,
                    burden: burden.clone(),
                    lifepoints: *lifepoints,
                },
            },
        })
    }
    fn house(&mut self) -> save::Result<House> {
        Ok(self.house.clone())
    }
    fn diaries(&mut self) -> save::Result<Vec<Diary>> {
        Ok(self.diaries.clone())
    }
    fn emotes(&mut self) -> save::Result<Emotes> {
        Ok(Emotes {
            save_required: self.emotes.iter().any(|ordinal| *ordinal > 21),
            ordinals: self.emotes.clone(),
        })
    }
    fn save_pouches(&mut self, root: &mut Json) -> save::Result<()> {
        self.call("pouches.save", vec![])?;
        for (key, value) in &self.pouches {
            root.put(key.clone(), java_json::parse(value).unwrap())
                .unwrap();
        }
        Ok(())
    }
    fn poh_storage(&mut self) -> save::Result<Json> {
        self.call("poh.save", vec![])?;
        Ok(java_json::parse(&self.poh).unwrap())
    }
    fn content_hooks(&mut self) -> usize {
        self.hooks.len()
    }
    fn save_content(&mut self, hook: usize, root: &mut Json) -> save::Result<()> {
        self.call("hook.save", vec![json!(hook)])?;
        let (key, value) = self.hooks[hook].clone();
        root.put(key, java_json::parse(&value).unwrap()).unwrap();
        Ok(())
    }
    fn parse_content(&mut self, hook: usize, root: &Json) -> save::Result<()> {
        let seen = root.get(&self.hooks[hook].0).map(json_string);
        self.call("hook.parse", vec![json!(hook), json!(seen)])
    }
    fn parse_quests(&mut self, data: &Json) -> save::Result<()> {
        self.call("quests.parse", vec![json!(data.text())])
    }
    fn update_grave_type(&mut self, _: &mut GameAttributes, ordinal: i32) -> save::Result<()> {
        self.call("grave", vec![json!(ordinal)])
    }
    fn set_autocast(&mut self, book: i32, spell: i32) -> save::Result<()> {
        match self.spells.get(&(book, spell)) {
            None => Err(save::Error::Null),
            Some(false) => Err(save::Error::Type),
            Some(true) => {
                self.autocast = Some((book, spell));
                Ok(())
            }
        }
    }
    fn unlock_music(&mut self, id: i32) -> save::Result<()> {
        let index = *self.songs.get(&id).ok_or(save::Error::Null)?;
        self.unlocked.put(index, id);
        Ok(())
    }
    fn parse_familiars(&mut self, data: &Json) -> save::Result<()> {
        self.call("familiar.parse", vec![json!(data.text())])
    }
    fn parse_house(&mut self, data: &Json) -> save::Result<()> {
        self.call("house.parse", vec![json!(data.text())])
    }
    fn emote_unlocked(&mut self, ordinal: i32) -> save::Result<bool> {
        Ok(self.emotes.contains(&ordinal))
    }
    fn unlock_emote(&mut self, ordinal: i32) -> save::Result<()> {
        self.emotes.push(ordinal);
        Ok(())
    }
    fn parse_diaries(&mut self, data: &Json) -> save::Result<()> {
        self.call("diary.parse", vec![json!(data.text())])
    }
    fn reset_diary_rewards(&mut self, _: &mut CoreData, _: &Registry) -> save::Result<()> {
        self.call("diary.reset", vec![])
    }
    fn parse_pouches(&mut self, data: &Json) -> save::Result<()> {
        self.call("pouches.parse", vec![json!(data.text())])
    }
    fn parse_poh_storage(&mut self, data: &Json) -> save::Result<()> {
        self.call("poh.parse", vec![json!(data.text())])
    }
}
impl World {
    fn skill_call(&mut self, tag: &str, value: i32) -> skills::Result<()> {
        if self
            .recorder
            .lock()
            .unwrap()
            .record(tag, vec![json!(value)])
        {
            Err(skills::Error::Binding)
        } else {
            Ok(())
        }
    }
}
impl skills::Context for World {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        unreachable!("the profile supplies the save version")
    }
    fn tick(&mut self) -> i32 {
        0
    }
    fn world_tick(&mut self) -> i32 {
        0
    }
    fn combat_level(&self) -> i32 {
        self.combat
    }
    fn summoning_combat_level(&self) -> i32 {
        self.summon
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        unreachable!()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        unreachable!()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn username(&mut self) -> skills::Result<String> {
        unreachable!()
    }
    fn permadeath_present(&self) -> bool {
        unreachable!("the profile supplies the permadeath attribute")
    }
    fn effect(&mut self, _: &mut Skills, effect: Effect) -> skills::Result<()> {
        match effect {
            Effect::Combat(level) => {
                self.skill_call("combat", level)?;
                self.combat = level;
            }
            Effect::SummoningCombat(level) => {
                self.skill_call("summon", level)?;
                self.summon = level;
            }
            Effect::Packet(skill) => self.skill_call("packet", skill as i32)?,
            other => unreachable!("unexpected skill effect {other:?}"),
        }
        Ok(())
    }
}
impl settings::Context for World {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        unreachable!()
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn send(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: rs09_protocol::game::Frame,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        unreachable!()
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        unreachable!()
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        Ok(self.retaliating)
    }
    fn set_retaliating(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        value: bool,
    ) -> settings::Result<()> {
        self.retaliating = value;
        Ok(())
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!()
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        unreachable!()
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        unreachable!()
    }
}
struct State {
    name: String,
    version: i32,
    save_parsed: bool,
    core: CoreData,
    attributes: GameAttributes,
    skills: Skills,
    settings: Settings,
    saved: SavedData,
    appearance: Appearance,
    bank_pin: bank_pin::Manager,
    ironman: ironman::Manager,
    spell_book: i32,
}
impl State {
    fn profile<'a>(&'a mut self, timers: &'a mut Manager) -> Profile<'a> {
        Profile {
            name: &self.name,
            version: &mut self.version,
            save_parsed: &mut self.save_parsed,
            core: &mut self.core,
            timers,
            attributes: &mut self.attributes,
            skills: &mut self.skills,
            settings: &self.settings,
            saved: &mut self.saved,
            appearance: &self.appearance,
            bank_pin: &self.bank_pin,
            ironman: &self.ironman,
            spell_book: &mut self.spell_book,
        }
    }
}
struct Directory(PathBuf);
impl Directory {
    fn new(index: usize) -> Self {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runs/player-save-native-tests");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "{}-{}-{index}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn prefix(&self, kind: &str) -> String {
        format!(
            "{}{}",
            self.0.join(kind).display(),
            std::path::MAIN_SEPARATOR
        )
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        // Only the directory created above, which holds nothing but this case's files.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn write_file(path: String, a: &Value) {
    let path = PathBuf::from(path);
    if a["delete"] == true {
        let _ = std::fs::remove_file(path);
        return;
    }
    if a["directory"] == true {
        std::fs::create_dir_all(path).unwrap();
        return;
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = match a["hex"].as_str() {
        Some(hex) => (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect(),
        None => a["text"].as_str().unwrap().as_bytes().to_vec(),
    };
    std::fs::write(path, bytes).unwrap();
}
fn container<'a>(core: &'a mut CoreData, name: &str) -> &'a mut Container {
    match name {
        "bank" => &mut core.primary.container,
        "secondary" => &mut core.secondary.container,
        "equipment" => &mut core.equipment,
        _ => &mut core.inventory,
    }
}
fn observe_container(c: &Container) -> Value {
    let items_handle = c.items();
    let items = items_handle.read();
    let sparse = items
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            t.as_ref().map(|t| {
                let t = t.read();
                json!([i, t.id(), t.amount(), t.charge(), t.index])
            })
        })
        .collect::<Vec<_>>();
    let kind = match c.kind() {
        ContainerType::Default => "DEFAULT",
        ContainerType::AlwaysStack => "ALWAYS_STACK",
        _ => unreachable!(),
    };
    let sort = match c.sort() {
        SortType::Id => "ID",
        SortType::Hash => "HASH",
    };
    let event_handle = c.event();
    let event = event_handle.read();
    json!([
        items.len(),
        kind,
        sort,
        c.listeners().len(),
        sparse,
        event.slots(),
        event.clear
    ])
}
/// Every observed section of the profile, keyed as Support.kt keys them.
fn sections(state: &State, world: &World) -> serde_json::Map<String, Value> {
    let core = &state.core;
    let varps = core
        .varps
        .order()
        .iter()
        .map(|k| json!([k, core.varps.get(*k).unwrap()]))
        .collect::<Vec<_>>();
    let utf16 = |key: &String| key.encode_utf16().collect::<Vec<_>>();
    let mut values = state.attributes.values.iter().collect::<Vec<_>>();
    values.sort_by_key(|(key, _)| utf16(key));
    let values = values
        .into_iter()
        .map(|(key, value)| {
            let (kind, text) = match value {
                None => ("null", Value::Null),
                Some(AttributeValue::Object(object)) => ("object", json!(object.java_string())),
                Some(value) => (value.saved_type(), json!(value.java_string())),
            };
            json!([key, kind, text])
        })
        .collect::<Vec<_>>();
    let mut expirations = state.attributes.expirations.iter().collect::<Vec<_>>();
    expirations.sort_by_key(|(key, _)| utf16(key));
    let s = &state.skills;
    let f = state.settings.fields.read().clone();
    let mut saved = serde_json::to_value(&state.saved).unwrap();
    saved["global"]["assistExperience"] = json!(
        state
            .saved
            .global
            .assist_experience
            .iter()
            .map(|&value| bits(value))
            .collect::<Vec<_>>()
    );
    let parts = state.appearance.parts();
    let parts = parts
        .read()
        .iter()
        .map(|part| {
            part.as_ref().map(|part| {
                let part = part.read();
                json!([part.look, part.color])
            })
        })
        .collect::<Vec<_>>();
    let pin = state.bank_pin.fields();
    let pin = pin.read();
    let mut music = world
        .unlocked
        .iter()
        .map(|(index, id)| (index, *id))
        .collect::<Vec<_>>();
    music.sort();
    let value = json!({
        "core": {"inventory":observe_container(&core.inventory),"bank":observe_container(&core.primary.container),"secondary":observe_container(&core.secondary.container),"equipment":observe_container(&core.equipment),"tabs":core.primary.tabs,"tabsSecondary":core.secondary.tabs,"secondarySelected":core.use_secondary,"location":core.location.as_ref().map(AttributeLocation::coordinates),"varps":varps,"saveVarps":core.save_varps},
        "version": state.version,
        "saveParsed": state.save_parsed,
        "spellbook": state.spell_book,
        "attributes": {"saved": state.attributes.saved, "values": values, "expirations": expirations.into_iter().map(|(key, value)| json!([key, value])).collect::<Vec<_>>()},
        "skills": {"combatMilestone":s.combat_milestone,"dynamicLevels":s.dynamic_levels,"experience":s.experience.map(bits),"experienceGained":bits(s.experience_gained),"experienceMultiplier":bits(s.experience_multiplier),"lifepoints":s.lifepoints,"lifepointsIncrease":s.lifepoints_increase,"lifepointsUpdate":s.lifepoints_update,"prayerPoints":bits(s.prayer_points),"skillMilestone":s.skill_milestone,"staticLevels":s.static_levels},
        "combat": [world.combat, world.summon],
        "settings": {"acceptAid":f.accept_aid,"areaSoundVolume":f.area_sound_volume,"assistSetting":f.assist_setting,"attackStyleIndex":f.attack_style_index,"brightness":f.brightness,"clanChatSetting":f.clan_chat_setting,"disableChatEffects":f.disable_chat_effects,"musicVolume":f.music_volume,"privateChatSetting":f.private_chat_setting,"publicChatSetting":f.public_chat_setting,"runEnergy":bits(f.run_energy),"runToggled":f.run_toggled,"singleMouseButton":f.single_mouse_button,"soundEffectVolume":f.sound_effect_volume,"specialEnergy":f.special_energy,"specialToggled":f.special_toggled,"splitPrivateChat":f.split_private_chat,"tradeSetting":f.trade_setting,"weight":bits(f.weight)},
        "retaliating": world.retaliating,
        "activity": saved["activity"],
        "quest": saved["quest"],
        "global": saved["global"],
        "appearance": [state.appearance.gender().map(|gender| gender.byte()), parts],
        "pin": [pin.pin.as_ref().map(|pin| String::from_utf16_lossy(pin)), pin.long_recovery, pin.status.map(|status| status as i32), pin.pending_delay, pin.try_delay],
        "ironman": state.ironman.mode().map(|mode| mode as i32),
        "music": music.into_iter().map(|(index, id)| json!([index, id])).collect::<Vec<_>>(),
        "emotes": world.emotes,
        "autocast": world.autocast.map(|(book, spell)| json!([book, spell])),
    });
    match value {
        Value::Object(map) => map,
        _ => unreachable!(),
    }
}
struct Case {
    definitions: Registry,
    state: State,
    world: World,
    recorder: Arc<Mutex<Recorder>>,
    harness: timer_native::Harness,
    manager: Manager,
    parser: Option<Parser>,
    directory: Directory,
    /// The last recorded value of each section; only changes are recorded.
    last: serde_json::Map<String, Value>,
}
type Outcome = save::Result<()>;
impl Case {
    fn new(request: &Value, index: usize) -> Self {
        let mut definitions = Registry::default();
        for id in 0..8 {
            definitions.insert(
                id,
                Shared::new(ItemDefinition {
                    id,
                    name: Some(format!("item{id}")),
                    value: id * 11,
                    ..Default::default()
                }),
            );
        }
        let config = &request["config"];
        let recorder = Arc::new(Mutex::new(Recorder {
            clock: config["clock"].as_i64().unwrap_or(0),
            step: config["clockStep"].as_i64().unwrap_or(0),
            ..Default::default()
        }));
        let mut world = World::new(recorder.clone());
        let core = CoreData::new(
            &Factory::new(&definitions),
            DEFAULT_BANK_SIZE,
            [0, 1, 2, 3].map(listeners::new),
            (config["location"] != false).then(|| AttributeLocation::new(3222, 3218, 0)),
        )
        .unwrap();
        let skills = Skills::new(&mut world).unwrap();
        let state = State {
            name: config["name"].as_str().unwrap_or("entity").to_owned(),
            version: config["version"].as_i64().map_or(6, |v| v as i32),
            save_parsed: false,
            core,
            attributes: GameAttributes::default(),
            skills,
            settings: Settings::default(),
            saved: SavedData::default(),
            appearance: Appearance::new(None, AppearanceFactory::default()).unwrap(),
            bank_pin: bank_pin::Manager::new(None),
            ironman: ironman::Manager::new(None),
            spell_book: 192,
        };
        world.script(&config["world"], &definitions);
        {
            let mut recorder = recorder.lock().unwrap();
            for (tag, at) in config["fail"].as_object().into_iter().flatten() {
                recorder.failures.insert(tag.clone(), int(at));
            }
            recorder.trace.clear();
            recorder.counts.clear();
        }
        let (harness, manager) = timer_native::Harness::new(request);
        let last = if config["full"] == true {
            serde_json::Map::new()
        } else {
            sections(&state, &world)
        };
        Self {
            last,
            definitions,
            state,
            world,
            recorder,
            harness,
            manager,
            parser: None,
            directory: Directory::new(index),
        }
    }
    fn save_section(&mut self, name: &str, env_clock: &rs09_server::auth::Clock) -> Outcome {
        let mut root = std::mem::replace(&mut self.harness.root, Json::object());
        let attribute_path = self.directory.prefix("a");
        let mut profile = self.state.profile(&mut self.manager);
        let mut env = Env {
            definitions: &self.definitions,
            clock: env_clock,
            timers: &mut self.harness,
            host: &mut self.world,
            attribute_path: &attribute_path,
        };
        let result = match name {
            "saveVersion" => save::save_version(&profile, &mut root),
            "saveCoreData" => save::save_core_data(&mut profile, &mut root, &mut env),
            "saveSkills" => save::save_skills(&profile, &mut root),
            "saveSettings" => save::save_settings(&profile, &mut root, &mut env),
            "saveQuests" => save::save_quests(&mut root, &mut env),
            "saveAppearance" => save::save_appearance(&profile, &mut root),
            "saveSpellbook" => save::save_spellbook(&profile, &mut root),
            "saveSavedData" => save::save_saved_data(&profile, &mut root),
            "saveActivityData" => save::save_activity_data(&profile, &mut root),
            "saveQuestData" => save::save_quest_data(&profile, &mut root),
            "saveGlobalData" => save::save_global_data(&profile, &mut root),
            "saveAutocast" => save::save_autocast(&mut root, &mut env),
            "savePlayerMonitor" => {
                save::save_player_monitor(&mut root);
                Ok(())
            }
            "saveMusicPlayer" => save::save_music_player(&mut root, &mut env),
            "saveFamiliarManager" => save::save_familiar_manager(&mut root, &mut env),
            "saveBankPinData" => save::save_bank_pin_data(&profile, &mut root, &mut env),
            "saveHouseData" => save::save_house_data(&mut root, &mut env),
            "saveAchievementData" => save::save_achievement_data(&mut root, &mut env),
            "saveIronManData" => save::save_iron_man_data(&profile, &mut root),
            "saveEmoteData" => save::save_emote_data(&mut root, &mut env),
            "saveStatManager" => save::save_stat_manager(&mut root),
            "saveAttributes" => save::save_attributes(&profile, &mut root, &mut env),
            "savePouches" => save::save_pouches(&mut root, &mut env),
            "savePOHStorageRoom" => save::save_poh_storage_room(&mut root, &mut env),
            other => panic!("unknown saver section {other}"),
        };
        self.harness.root = root;
        result
    }
    fn parse_section(
        &mut self,
        name: &str,
        save_file: Option<&Json>,
        env_clock: &rs09_server::auth::Clock,
    ) -> Outcome {
        let attribute_path = self.directory.prefix("a");
        let mut profile = self.state.profile(&mut self.manager);
        let mut env = Env {
            definitions: &self.definitions,
            clock: env_clock,
            timers: &mut self.harness,
            host: &mut self.world,
            attribute_path: &attribute_path,
        };
        match name {
            "parseData" => parser::parse_data(&mut profile, save_file, &mut env).map(|_| ()),
            "parseVersion" => parser::parse_version(&mut profile, save_file),
            "parseCore" => parser::parse_core(&mut profile, save_file, &mut env),
            "parseAttributes" => {
                parser::parse_attributes(&mut profile, save_file, &mut env).map(|_| ())
            }
            "parseSkills" => parser::parse_skills(&mut profile, save_file, &mut env),
            "parseSettings" => parser::parse_settings(&mut profile, save_file, &mut env),
            "parseQuests" => parser::parse_quests(save_file, &mut env),
            "parseAppearance" => parser::parse_appearance(&mut profile, save_file),
            "parseGrave" => parser::parse_grave(&mut profile, save_file, &mut env),
            "parseVarps" => parser::parse_varps(&mut profile, save_file),
            "parseSpellbook" => parser::parse_spellbook(&mut profile, save_file),
            "parseSavedData" => parser::parse_saved_data(&mut profile, save_file),
            "parseAutocastSpell" => parser::parse_autocast_spell(save_file, &mut env),
            "parseFarming" | "parseMonitor" => Ok(()),
            "parseConfigs" => parser::parse_configs(&mut profile, save_file),
            "parseMusic" => parser::parse_music(save_file, &mut env),
            "parseFamiliars" => parser::parse_familiars(save_file, &mut env),
            "parseBankPin" => parser::parse_bank_pin(&mut profile, save_file),
            "parseHouse" => parser::parse_house(save_file, &mut env),
            "parseIronman" => parser::parse_ironman(&mut profile, save_file),
            "parseEmoteManager" => parser::parse_emote_manager(save_file, &mut env),
            "parseStatistics" => parser::parse_statistics(save_file),
            "parseAchievements" => parser::parse_achievements(&mut profile, save_file, &mut env),
            "parsePouches" => parser::parse_pouches(save_file, &mut env),
            "parsePOHStorable" => parser::parse_poh_storable(save_file, &mut env),
            other => panic!("unknown parser section {other}"),
        }
    }
    fn run(
        &mut self,
        a: &Value,
        out: &mut serde_json::Map<String, Value>,
        clock: &rs09_server::auth::Clock,
    ) -> Outcome {
        let save_path = self.directory.prefix("s");
        let attribute_path = self.directory.prefix("a");
        let file = format!("{save_path}{}.json", self.state.name);
        macro_rules! services {
            () => {
                Env {
                    definitions: &self.definitions,
                    clock,
                    timers: &mut self.harness,
                    host: &mut self.world,
                    attribute_path: &attribute_path,
                }
            };
        }
        match a["op"].as_str().unwrap() {
            "saveFile" => write_file(file, a),
            "blockSaveDirectory" => {
                let directory = self.directory.0.join("s");
                std::fs::create_dir_all(&self.directory.0).unwrap();
                std::fs::write(directory, "not a directory").unwrap();
            }
            "attributeFile" => write_file(format!("{attribute_path}{}.xml", self.state.name), a),
            "profileParse" => {
                let mut parser = Parser::default();
                let mut profile = self.state.profile(&mut self.manager);
                let result = parser.parse(&mut profile, &mut services!(), &save_path);
                out.insert("read".into(), json!(parser.read));
                out.insert("loaded".into(), json!(parser.save_file.is_some()));
                self.parser = Some(parser);
                result?;
            }
            "contentHooks" => {
                let fresh = Parser::default();
                let parser = if a["fresh"] == true {
                    &fresh
                } else {
                    self.parser.as_ref().ok_or(save::Error::Null)?
                };
                let mut profile = self.state.profile(&mut self.manager);
                parser.run_content_hooks(&mut profile, &mut services!())?;
            }
            "parseText" => {
                let mut parser = Parser::default();
                let mut profile = self.state.profile(&mut self.manager);
                let result =
                    parser.parse_text(&mut profile, &mut services!(), a["text"].as_str().unwrap());
                self.parser = Some(parser);
                result?;
            }
            "parseSection" => {
                let save_file = match a["text"].as_str() {
                    Some(text) => Some(parser::parse_text(text)?),
                    None => None,
                };
                self.parse_section(a["name"].as_str().unwrap(), save_file.as_ref(), clock)?;
            }
            "populate" => {
                let mut profile = self.state.profile(&mut self.manager);
                let text = save::save_text(&mut profile, &mut services!())?;
                out.insert("text".into(), json!(text));
            }
            "profileSave" => {
                out.insert(
                    "existed".into(),
                    json!(std::path::Path::new(&file).exists()),
                );
                let mut profile = self.state.profile(&mut self.manager);
                let outcome = save::save_file(&mut profile, &mut services!(), &save_path)?;
                // The original swallows a failed write, so the step itself has succeeded.
                out.insert("saved".into(), json!(true));
                let path = std::path::Path::new(&file);
                let text = if path.exists() {
                    let bytes = std::fs::read(path).map_err(|e| save::Error::Io(e.kind()))?;
                    json!(String::from_utf8_lossy(&bytes))
                } else {
                    assert_ne!(outcome, SaveOutcome::Written);
                    Value::Null
                };
                out.insert("file".into(), text);
            }
            "saveSection" => self.save_section(a["name"].as_str().unwrap(), clock)?,
            "world" => self.world.script(&a["world"], &self.definitions),
            "failAt" => {
                let mut recorder = self.recorder.lock().unwrap();
                let tag = a["tag"].as_str().unwrap();
                let seen = recorder.counts.get(tag).copied().unwrap_or(0);
                recorder
                    .failures
                    .insert(tag.to_owned(), seen + number(a, "at", 1));
            }
            "clock" => {
                let mut recorder = self.recorder.lock().unwrap();
                recorder.clock = a["value"].as_i64().unwrap();
                if let Some(step) = a["step"].as_i64() {
                    recorder.step = step;
                }
            }
            "name" => self.state.name = a["value"].as_str().unwrap().to_owned(),
            "version" => self.state.version = int(&a["value"]),
            "saveParsed" => self.state.save_parsed = a["value"].as_bool().unwrap(),
            "retaliate" => self.world.retaliating = a["value"].as_bool().unwrap(),
            "spellbook" => self.state.spell_book = int(&a["value"]),
            "ironman" => self.state.ironman.set_mode(a["mode"].as_i64().map(|mode| {
                [
                    ironman::Mode::None,
                    ironman::Mode::Standard,
                    ironman::Mode::Hardcore,
                    ironman::Mode::Ultimate,
                ][mode as usize]
            })),
            "gender" => {
                assert!(a["value"].is_null(), "only a null gender is scripted");
                let _ = self.state.appearance.set_gender(None);
            }
            "part" => {
                self.state.appearance.parts().write()[int(&a["index"]) as usize] = (a["null"]
                    != true)
                    .then(|| Shared::new(BodyPart::new(int(&a["look"]), int(&a["color"]))));
            }
            "pin" => {
                let fields = self.state.bank_pin.fields();
                let mut f = fields.write();
                if let Some(pin) = a.get("pin") {
                    f.pin = pin.as_str().map(bank_pin::text);
                }
                if let Some(status) = a.get("status") {
                    f.status = status.as_i64().map(|status| {
                        [
                            bank_pin::Status::NoPin,
                            bank_pin::Status::Pending,
                            bank_pin::Status::Active,
                        ][status as usize]
                    });
                }
                if let Some(value) = a.get("longRecovery") {
                    f.long_recovery = value.as_bool().unwrap();
                }
                if let Some(value) = a.get("pendingDelay") {
                    f.pending_delay = value.as_i64().unwrap();
                }
                if let Some(value) = a.get("tryDelay") {
                    f.try_delay = value.as_i64().unwrap();
                }
            }
            "settings" => {
                let mut f = self.state.settings.fields.write();
                for (key, value) in a["fields"].as_object().unwrap() {
                    match key.as_str() {
                        "runEnergy" => f.run_energy = double(value),
                        "weight" => f.weight = double(value),
                        "brightness" => f.brightness = int(value),
                        "musicVolume" => f.music_volume = int(value),
                        "soundEffectVolume" => f.sound_effect_volume = int(value),
                        "areaSoundVolume" => f.area_sound_volume = int(value),
                        "singleMouseButton" => f.single_mouse_button = value.as_bool().unwrap(),
                        "disableChatEffects" => f.disable_chat_effects = value.as_bool().unwrap(),
                        "splitPrivateChat" => f.split_private_chat = value.as_bool().unwrap(),
                        "acceptAid" => f.accept_aid = value.as_bool().unwrap(),
                        "runToggled" => f.run_toggled = value.as_bool().unwrap(),
                        "publicChatSetting" => f.public_chat_setting = int(value),
                        "privateChatSetting" => f.private_chat_setting = int(value),
                        "clanChatSetting" => f.clan_chat_setting = int(value),
                        "tradeSetting" => f.trade_setting = int(value),
                        "assistSetting" => f.assist_setting = int(value),
                        "specialToggled" => f.special_toggled = value.as_bool().unwrap(),
                        "specialEnergy" => f.special_energy = int(value),
                        "attackStyleIndex" => f.attack_style_index = int(value),
                        other => panic!("unknown settings field {other}"),
                    }
                }
            }
            "skills" => {
                let s = &mut self.state.skills;
                for (key, value) in a["fields"].as_object().unwrap() {
                    let list = || value.as_array().unwrap().iter().enumerate();
                    match key.as_str() {
                        "experience" => list().for_each(|(i, v)| s.experience[i] = double(v)),
                        "staticLevels" => list().for_each(|(i, v)| s.static_levels[i] = int(v)),
                        "dynamicLevels" => list().for_each(|(i, v)| s.dynamic_levels[i] = int(v)),
                        "prayerPoints" => s.prayer_points = double(value),
                        "experienceGained" => s.experience_gained = double(value),
                        "experienceMultiplier" => s.experience_multiplier = double(value),
                        "lifepoints" => s.lifepoints = int(value),
                        "lifepointsIncrease" => s.lifepoints_increase = int(value),
                        "lifepointsUpdate" => s.lifepoints_update = value.as_bool().unwrap(),
                        "combatMilestone" => s.combat_milestone = int(value),
                        "skillMilestone" => s.skill_milestone = int(value),
                        other => panic!("unknown skills field {other}"),
                    }
                }
            }
            "attr" => {
                let key = a["key"].as_str().unwrap();
                let v = &a["value"];
                let value = match a["kind"].as_str().unwrap() {
                    "int" => Some(AttributeValue::Int(int(v))),
                    "long" => Some(AttributeValue::Long(v.as_i64().unwrap())),
                    "short" => Some(AttributeValue::Short(v.as_i64().unwrap() as i16)),
                    "byte" => Some(AttributeValue::Byte(v.as_i64().unwrap() as i8)),
                    "bool" => Some(AttributeValue::Bool(v.as_bool().unwrap())),
                    "str" => Some(AttributeValue::Text(v.as_str().unwrap().to_owned())),
                    "location" => Some(AttributeValue::Location(AttributeLocation::new(
                        int(&v[0]),
                        int(&v[1]),
                        int(&v[2]),
                    ))),
                    "double" => Some(AttributeValue::Object(Arc::new(JavaDouble(double(v))))),
                    "null" => None,
                    other => panic!("unknown attribute kind {other}"),
                };
                self.state.attributes.set(key, value);
                if let Some(expires) = a.get("expires") {
                    self.state.attributes.expirations.insert(
                        key.strip_prefix("/save:").unwrap_or(key).to_owned(),
                        expires.as_i64(),
                    );
                }
            }
            "questItem" => self
                .state
                .saved
                .quest
                .set_desert_treasure_item(
                    int(&a["index"]),
                    (a["null"] != true).then(|| ItemData::new(int(&a["id"]), int(&a["amount"]))),
                )
                .unwrap(),
            "attrSaved" => self
                .state
                .attributes
                .saved
                .push(Some(a["key"].as_str().unwrap().to_owned())),
            "coreVarp" | "coreSecondary" | "coreLocation" => {
                self.state.core.action(a).unwrap();
            }
            "coreTab" => {
                let tabs = if a["secondary"] == true {
                    &mut self.state.core.secondary.tabs
                } else {
                    &mut self.state.core.primary.tabs
                };
                *tabs
                    .get_mut(number(a, "index", 0) as usize)
                    .ok_or(save::Error::Index)? = number(a, "value", 0);
            }
            "coreRaw" => {
                let c = container(
                    &mut self.state.core,
                    a["container"].as_str().unwrap_or("inventory"),
                );
                let t = if a["null"] == true {
                    None
                } else {
                    let mut t = Item::new(
                        number(a, "id", 1),
                        number(a, "amount", 1),
                        number(a, "charge", 1000),
                        &self.definitions,
                    );
                    t.index = number(a, "index", number(a, "slot", 0));
                    Some(Shared::new(t))
                };
                *c.items()
                    .write()
                    .get_mut(number(a, "slot", 0) as usize)
                    .ok_or(save::Error::Index)? = t;
            }
            other => panic!("unknown profile action {other}"),
        }
        Ok(())
    }
}
const PROFILE_OPS: &[&str] = &[
    "saveFile",
    "blockSaveDirectory",
    "attributeFile",
    "profileParse",
    "contentHooks",
    "parseText",
    "parseSection",
    "populate",
    "profileSave",
    "saveSection",
    "world",
    "failAt",
    "clock",
    "name",
    "version",
    "saveParsed",
    "retaliate",
    "spellbook",
    "ironman",
    "gender",
    "part",
    "pin",
    "settings",
    "skills",
    "attr",
    "questItem",
    "attrSaved",
    "coreVarp",
    "coreSecondary",
    "coreLocation",
    "coreTab",
    "coreRaw",
];
fn timer_error(error: &rs09_server::timers::Error) -> &'static str {
    use rs09_server::timers::Error;
    match error {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Number => "Number",
        Error::Index => "Index",
        Error::ConcurrentModification => "ConcurrentModification",
        Error::Binding(_) => "Binding",
    }
}
pub fn evaluate(request: &Value, index: usize) -> Value {
    let mut case = Case::new(request, index);
    let recorder = case.recorder.clone();
    let clock = move || {
        let mut recorder = recorder.lock().unwrap();
        let value = recorder.clock;
        recorder.clock = value.wrapping_add(recorder.step);
        recorder.record("now", vec![json!(value)]);
        value
    };
    let mut states = Vec::new();
    for a in request["actions"].as_array().unwrap() {
        case.harness.clear_events();
        let mut state = if PROFILE_OPS.contains(&a["op"].as_str().unwrap()) {
            let mut out = serde_json::Map::new();
            out.insert("error".into(), Value::Null);
            if let Err(error) = case.run(a, &mut out, &clock) {
                out.insert("error".into(), json!(category(&error)));
            }
            case.harness
                .snapshot(&case.manager, Value::Object(out), None)
        } else {
            let (value, error) =
                match case
                    .harness
                    .action(&mut case.manager, &mut case.state.core, a)
                {
                    Ok(value) => (value, None),
                    Err(error) => (Value::Null, Some(timer_error(&error))),
                };
            case.harness.snapshot(&case.manager, value, error)
        };
        let mut extra = serde_json::Map::new();
        extra.insert(
            "trace".into(),
            json!(std::mem::take(&mut case.recorder.lock().unwrap().trace)),
        );
        if !(a["quiet"] == true && PROFILE_OPS.contains(&a["op"].as_str().unwrap())) {
            for (key, value) in sections(&case.state, &case.world) {
                if case.last.get(&key) != Some(&value) {
                    case.last.insert(key.clone(), value.clone());
                    extra.insert(key, value);
                }
            }
        }
        state["extra"] = Value::Object(extra);
        states.push(state);
    }
    json!(states)
}
