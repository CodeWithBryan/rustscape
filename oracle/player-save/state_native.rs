//! Native replay of the appended state cases (marked "state"). Production
//! code performs every save, load and state change: `player::persisted` holds
//! the systems' state and `persisted::host::Native` answers `save::Host` from
//! it. This file scripts the same inputs as State.kt, stands in for the
//! services that stay outside native state and observes the result.
#[allow(dead_code)]
#[path = "../inventory/listeners.rs"]
mod listeners;
#[allow(dead_code)]
#[path = "../timers/native.rs"]
mod timer_native;
use rs09_game_core::{java_double, java_json::Json};
use rs09_server::{
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        self,
        container::{Container, ContainerType, Event, Factory, SortType},
        container_listener::Handle as Listener,
        item::{Handle, Item},
    },
    player::{
        appearance::{Appearance, Factory as AppearanceFactory},
        attributes::{AttributeLocation, AttributeValue, GameAttributes},
        bank_pin,
        core_data::{CoreData, DEFAULT_BANK_SIZE},
        ironman,
        persisted::{
            diaries, graves,
            host::{Native, Persisted, Remaining, World},
            music, pouches, quests, spells,
        },
        save::{
            self, ActiveFamiliar, Category, Env, Host, Level, Parser, PetDetail, Profile, Room,
            Servant, parser,
        },
        saved_data::SavedData,
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

fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
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

/// The services that stay outside native state, as State.kt's stand-ins.
struct Rest {
    recorder: Arc<Mutex<Recorder>>,
    retaliating: bool,
    combat: i32,
    summon: i32,
    hooks: Vec<(String, String)>,
}
impl Rest {
    fn call(&mut self, tag: &str, values: Vec<Value>) -> save::Result<()> {
        if self.recorder.lock().unwrap().record(tag, values) {
            Err(save::Error::Host(tag.to_owned()))
        } else {
            Ok(())
        }
    }
    fn listener(&mut self, listener: &Listener, what: &str) -> inventory::Result<()> {
        let class = match listeners::label(listener) {
            0 => "InventoryListener",
            1 => "EquipmentListener",
            _ => "BankListener",
        };
        if self
            .recorder
            .lock()
            .unwrap()
            .record("listener", vec![json!(class), json!(what)])
        {
            Err(inventory::Error::Binding("listener".into()))
        } else {
            Ok(())
        }
    }
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
impl Remaining for Rest {
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
    fn attribute_set(&mut self, key: &str, value: &AttributeValue) -> save::Result<()> {
        self.call(
            "event",
            vec![
                json!("AttributeSet"),
                json!(key),
                json!(value.java_string()),
            ],
        )
    }
    fn content_hooks(&mut self) -> usize {
        self.hooks.len()
    }
    fn save_content(&mut self, hook: usize, root: &mut Json) -> save::Result<()> {
        self.call("hook.save", vec![json!(hook)])?;
        let (key, value) = self.hooks[hook].clone();
        root.put(key, rs09_game_core::java_json::parse(&value).unwrap())
            .unwrap();
        Ok(())
    }
    fn parse_content(&mut self, hook: usize, root: &Json) -> save::Result<()> {
        let seen = root.get(&self.hooks[hook].0).map(|value| {
            let text = Json::Array(vec![value.clone()]).text();
            text[1..text.len() - 1].to_owned()
        });
        self.call("hook.parse", vec![json!(hook), json!(seen)])
    }
    fn parse_familiars(&mut self, data: &Json) -> save::Result<()> {
        self.call("familiar.parse", vec![json!(data.text())])
    }
    fn parse_house(&mut self, data: &Json) -> save::Result<()> {
        self.call("house.parse", vec![json!(data.text())])
    }
}
impl inventory::container::Context for Rest {
    fn update(
        &mut self,
        listener: Listener,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> inventory::Result<()> {
        self.listener(&listener, "update")
    }
    fn refresh(
        &mut self,
        listener: Listener,
        _: &mut Container,
        _: &Registry,
    ) -> inventory::Result<()> {
        self.listener(&listener, "refresh")
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<Handle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "no ground in the state corpus".into(),
        ))
    }
}
impl skills::Context for Rest {
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
impl settings::Context for Rest {
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

/// The profile sections the save module already joins, as one character.
struct Character {
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
    manager: Manager,
    persisted: Persisted,
}

struct Directory(PathBuf);
impl Directory {
    fn new(index: usize) -> Self {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runs/player-save-native-tests");
        std::fs::create_dir_all(&root).unwrap();
        // (A counter as well: two cases of one process can start within one tick
        // of the clock, and would otherwise be given the same directory.)
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = root.join(format!(
            "state-{}-{unique}-{}-{index}",
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

struct Case {
    request: Value,
    definitions: Registry,
    factory: Factory,
    world: World,
    quest_names: Vec<String>,
    rest: Rest,
    recorder: Arc<Mutex<Recorder>>,
    harness: timer_native::Harness,
    player: Character,
    parser: Option<Parser>,
    directory: Directory,
    last: serde_json::Map<String, Value>,
}
fn define(definitions: &mut Registry, id: i32, stackable: bool) {
    definitions.insert(
        id,
        Shared::new(ItemDefinition {
            id,
            name: Some(format!("item{id}")),
            value: id * 11,
            stackable,
            ..Default::default()
        }),
    );
}
impl Case {
    fn new(request: &Value, index: usize) -> Self {
        let w = &request["world"];
        let mut definitions = Registry::default();
        for id in 0..8 {
            define(&mut definitions, id, false);
        }
        let recorder = Arc::new(Mutex::new(Recorder {
            clock: w["clock"].as_i64().unwrap_or(0),
            step: w["clockStep"].as_i64().unwrap_or(0),
            ..Default::default()
        }));
        let quest_names: Vec<String> = w["questNames"]
            .as_array()
            .map(|names| {
                names
                    .iter()
                    .map(|n| n.as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let factory = Factory::new(&definitions);
        let mut rest = Rest {
            recorder: recorder.clone(),
            retaliating: true,
            combat: 0,
            summon: 0,
            hooks: Vec::new(),
        };
        let mut world = World::default();
        Self::registry(w, &mut world, &mut definitions, &mut rest, &quest_names);
        // No timer is registered or scripted in these cases.
        let (harness, _) = timer_native::Harness::new(&json!({"timers": {}}));
        let player = Self::character(request, &factory, &world, &mut rest);
        let mut case = Self {
            request: request.clone(),
            factory,
            definitions,
            world,
            quest_names,
            rest,
            recorder,
            harness,
            player,
            parser: None,
            directory: Directory::new(index),
            last: serde_json::Map::new(),
        };
        {
            let mut recorder = case.recorder.lock().unwrap();
            recorder.trace.clear();
            recorder.counts.clear();
        }
        if request["full"] != true {
            case.last = case.sections();
        }
        case
    }
    fn registry(
        w: &Value,
        world: &mut World,
        definitions: &mut Registry,
        rest: &mut Rest,
        names: &[String],
    ) {
        for row in w["quests"].as_array().into_iter().flatten() {
            let ordinal = int(&row[0]);
            world.quests.register(
                ordinal,
                quests::Quest {
                    name: names.get(ordinal as usize).cloned().unwrap_or_default(),
                    index: int(&row[1]),
                    button: int(&row[2]),
                    points: int(&row[3]),
                },
            );
        }
        for row in w["songs"].as_array().into_iter().flatten() {
            world.songs.songs.insert(
                int(&row[0]),
                music::Song {
                    name: row[1].as_str().unwrap().to_owned(),
                    index: int(&row[2]),
                },
            );
        }
        for row in w["spells"].as_array().into_iter().flatten() {
            world.spells.spells.insert(
                (int(&row[0]), int(&row[1])),
                spells::Spell {
                    book: int(&row[3]),
                    combat: row[2] == true,
                },
            );
        }
        for row in w["items"].as_array().into_iter().flatten() {
            define(definitions, int(&row[0]), row[1] == true);
        }
        for row in w["hooks"].as_array().into_iter().flatten() {
            rest.hooks.push((
                row[0].as_str().unwrap().to_owned(),
                row[1].as_str().unwrap().to_owned(),
            ));
        }
    }
    /// What `new Player()` builds, with State.kt's location, name and version.
    fn character(request: &Value, factory: &Factory, world: &World, rest: &mut Rest) -> Character {
        let w = &request["world"];
        let core = CoreData::new(
            factory,
            DEFAULT_BANK_SIZE,
            [0, 1, 2, 3].map(listeners::new),
            Some(AttributeLocation::new(3222, 3218, 0)),
        )
        .unwrap();
        let skills = Skills::new(rest).unwrap();
        let manager = Manager::default();
        Character {
            name: w["name"].as_str().unwrap_or("entity").to_owned(),
            version: w["version"].as_i64().map_or(6, |v| v as i32),
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
            manager,
            persisted: Persisted::new(world, factory).unwrap(),
        }
    }

    fn items(c: &Container) -> Value {
        let items = c.items();
        let items = items.read();
        json!(
            items
                .iter()
                .enumerate()
                .filter_map(|(i, t)| t.as_ref().map(|t| {
                    let t = t.read();
                    json!([i, t.id(), t.amount(), t.charge()])
                }))
                .collect::<Vec<_>>()
        )
    }
    /// State.kt's sections, read from native state in the original collections' order.
    fn sections(&self) -> serde_json::Map<String, Value> {
        let p = &self.player;
        let s = &p.persisted;
        let flags = |values: &[bool]| {
            values
                .iter()
                .map(|v| if *v { '1' } else { '0' })
                .collect::<String>()
        };
        let utf16 = |key: &String| key.encode_utf16().collect::<Vec<_>>();
        let mut values = p.attributes.values.iter().collect::<Vec<_>>();
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
        let house = &s.house;
        let rooms = house
            .rooms
            .iter()
            .flatten()
            .flatten()
            .filter(|r| r.is_some())
            .count();
        let value = json!({
            "quests": [s.quests.points, s.quests.stages.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>()],
            "music": s.music.unlocked.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
            "emotes": [s.emotes.is_save_required(), s.emotes.emotes],
            "diaries": s.diaries.diaries.iter().map(|d| json!([diaries::TYPES[d.kind].name, flags(&d.started), d.completed.iter().map(|l| flags(l)).collect::<Vec<_>>(), flags(&d.rewarded)])).collect::<Vec<_>>(),
            "diaryLevels": self.world.completed_levels,
            "autocast": s.spells.autocast.map(|a| json!([a.book, a.spell_id])),
            "spellbook": p.spell_book,
            "attributes": {"saved": p.attributes.saved, "values": values},
            "pouches": s.pouches.pouches.iter().map(|q| json!([q.id, q.charges, q.current_cap, q.container.capacity(), Self::items(&q.container)])).collect::<Vec<_>>(),
            "poh": s.poh.containers.iter().map(|c| json!([c.stored, c.current_page])).collect::<Vec<_>>(),
            "house": [house.location, house.style, house.crest, house.servant.is_some(), rooms],
            "pets": s.familiars.pets.iter().map(|(id, d)| json!([id, d.iter().map(|d| json!([bits(d.hunger), bits(d.growth)])).collect::<Vec<_>>()])).collect::<Vec<_>>(),
            "familiar": match &s.familiars.active {
                ActiveFamiliar::None => Value::Null,
                ActiveFamiliar::Pet { item_id } => json!(["pet", item_id]),
                ActiveFamiliar::Familiar { original_id, .. } => json!(["familiar", original_id]),
            },
            "containers": {"inventory": Self::items(&p.core.inventory), "bank": Self::items(&p.core.primary.container), "secondary": Self::items(&p.core.secondary.container), "equipment": Self::items(&p.core.equipment), "useSecondary": p.core.use_secondary},
            "version": p.version,
            "saveParsed": p.save_parsed,
        });
        match value {
            Value::Object(map) => map,
            _ => unreachable!(),
        }
    }

    fn run(
        &mut self,
        a: &Value,
        out: &mut serde_json::Map<String, Value>,
        clock: &rs09_server::auth::Clock,
    ) -> save::Result<()> {
        let save_path = self.directory.prefix("s");
        let attribute_path = self.directory.prefix("a");
        let file = format!("{save_path}{}.json", self.player.name);
        let Self {
            request,
            recorder,
            definitions,
            factory,
            world,
            rest,
            harness,
            player,
            parser: stored,
            quest_names,
            ..
        } = self;
        let Character {
            name,
            version,
            save_parsed,
            core,
            attributes,
            skills,
            settings,
            saved,
            appearance,
            bank_pin,
            ironman,
            spell_book,
            manager,
            persisted,
        } = player;
        macro_rules! profile {
            () => {
                Profile {
                    name,
                    version,
                    save_parsed,
                    core,
                    timers: manager,
                    attributes,
                    skills,
                    settings,
                    saved,
                    appearance,
                    bank_pin,
                    ironman,
                    spell_book,
                }
            };
        }
        macro_rules! native {
            () => {
                Native {
                    state: persisted,
                    world,
                    factory,
                    definitions,
                    rest,
                }
            };
        }
        let op = a["op"].as_str().unwrap();
        match op {
            "populate" | "save" | "parse" | "parseData" | "profileParse" | "contentHooks"
            | "profileSave" => {
                let mut host = native!();
                let mut env = Env {
                    definitions,
                    clock,
                    timers: harness,
                    host: &mut host,
                    attribute_path: &attribute_path,
                };
                let mut profile = profile!();
                match op {
                    "populate" => {
                        let text = save::save_text(&mut profile, &mut env)?;
                        out.insert("text".into(), json!(text));
                    }
                    "save" => {
                        let mut root = Json::object();
                        let result = match a["name"].as_str().unwrap() {
                            "saveQuests" => save::save_quests(&mut root, &mut env),
                            "saveSpellbook" => save::save_spellbook(&profile, &mut root),
                            "saveAutocast" => save::save_autocast(&mut root, &mut env),
                            "saveMusicPlayer" => save::save_music_player(&mut root, &mut env),
                            "saveFamiliarManager" => {
                                save::save_familiar_manager(&mut root, &mut env)
                            }
                            "saveHouseData" => save::save_house_data(&mut root, &mut env),
                            "saveAchievementData" => {
                                save::save_achievement_data(&mut root, &mut env)
                            }
                            "saveEmoteData" => save::save_emote_data(&mut root, &mut env),
                            "saveAttributes" => {
                                save::save_attributes(&profile, &mut root, &mut env)
                            }
                            "savePouches" => save::save_pouches(&mut root, &mut env),
                            "savePOHStorageRoom" => {
                                save::save_poh_storage_room(&mut root, &mut env)
                            }
                            other => panic!("unknown saver section {other}"),
                        };
                        out.insert("text".into(), json!(root.text()));
                        result?;
                    }
                    "parse" => {
                        let save_file = match a["text"].as_str() {
                            Some(text) => Some(parser::parse_text(text)?),
                            None => None,
                        };
                        let save_file = save_file.as_ref();
                        match a["name"].as_str().unwrap() {
                            "parseData" => {
                                parser::parse_data(&mut profile, save_file, &mut env).map(|_| ())?
                            }
                            "parseQuests" => parser::parse_quests(save_file, &mut env)?,
                            "parseGrave" => parser::parse_grave(&mut profile, save_file, &mut env)?,
                            "parseSpellbook" => parser::parse_spellbook(&mut profile, save_file)?,
                            "parseAutocastSpell" => {
                                parser::parse_autocast_spell(save_file, &mut env)?
                            }
                            "parseMusic" => parser::parse_music(save_file, &mut env)?,
                            "parseFamiliars" => parser::parse_familiars(save_file, &mut env)?,
                            "parseHouse" => parser::parse_house(save_file, &mut env)?,
                            "parseEmoteManager" => {
                                parser::parse_emote_manager(save_file, &mut env)?
                            }
                            "parseAchievements" => {
                                parser::parse_achievements(&mut profile, save_file, &mut env)?
                            }
                            "parsePouches" => parser::parse_pouches(save_file, &mut env)?,
                            "parsePOHStorable" => parser::parse_poh_storable(save_file, &mut env)?,
                            "parseAttributes" => {
                                parser::parse_attributes(&mut profile, save_file, &mut env)
                                    .map(|_| ())?
                            }
                            other => panic!("unknown parser section {other}"),
                        }
                    }
                    "parseData" => {
                        let mut parser = Parser::default();
                        let result =
                            parser.parse_text(&mut profile, &mut env, a["text"].as_str().unwrap());
                        *stored = Some(parser);
                        result?;
                    }
                    "profileParse" => {
                        let mut parser = Parser::default();
                        let result = parser.parse(&mut profile, &mut env, &save_path);
                        out.insert("read".into(), json!(parser.read));
                        out.insert("loaded".into(), json!(parser.save_file.is_some()));
                        *stored = Some(parser);
                        result?;
                    }
                    "contentHooks" => {
                        let fresh = Parser::default();
                        let parser = if a["fresh"] == true {
                            &fresh
                        } else {
                            stored.as_ref().ok_or(save::Error::Null)?
                        };
                        parser.run_content_hooks(&mut profile, &mut env)?;
                    }
                    "profileSave" => {
                        out.insert(
                            "existed".into(),
                            json!(std::path::Path::new(&file).exists()),
                        );
                        save::save_file(&mut profile, &mut env, &save_path)?;
                        let path = std::path::Path::new(&file);
                        let text = if path.exists() {
                            let bytes =
                                std::fs::read(path).map_err(|e| save::Error::Io(e.kind()))?;
                            json!(String::from_utf8_lossy(&bytes))
                        } else {
                            Value::Null
                        };
                        out.insert("file".into(), text);
                    }
                    _ => unreachable!(),
                }
            }
            "newPlayer" => {
                *player = Self::character(request, factory, world, rest);
                *stored = None;
            }
            "registry" => Self::registry(&a["world"], world, definitions, rest, quest_names),
            "failAt" => {
                let mut recorder = recorder.lock().unwrap();
                let tag = a["tag"].as_str().unwrap();
                let seen = recorder.counts.get(tag).copied().unwrap_or(0);
                let at = a["at"].as_i64().map_or(1, |v| v as i32);
                recorder.failures.insert(tag.to_owned(), seen + at);
            }
            "version" => *version = int(&a["value"]),
            "saveParsed" => *save_parsed = a["value"].as_bool().unwrap(),
            // Quests.
            "quest.parse" => persisted.quests.parse(
                &parser::parse_text(a["text"].as_str().unwrap())?,
                &world.quests,
            )?,
            "quest.put" => {
                persisted
                    .quests
                    .stages
                    .put(int(&a["index"]), int(&a["stage"]));
            }
            "quest.stage" => {
                let quest = world
                    .quests
                    .get(int(&a["quest"]))
                    .cloned()
                    .ok_or(save::Error::Null)?;
                if a["any"] == true {
                    persisted
                        .quests
                        .set_stage_nonmonotonic(&quest, int(&a["stage"]));
                } else {
                    persisted.quests.set_stage(
                        &quest,
                        int(&a["stage"]),
                        name,
                        &mut |level, message| rest.log(level, message),
                    )?;
                }
            }
            "quest.points" => {
                if a["dock"] == true {
                    persisted.quests.dock_points(int(&a["value"]));
                } else {
                    persisted.quests.increment_points(int(&a["value"]));
                }
            }
            "quest.sync" => persisted.quests.sync_points(&world.quests)?,
            "quest.getStage" => {
                out.insert(
                    "value".into(),
                    json!(persisted.quests.stage_of(&world.quests, int(&a["quest"]))?),
                );
            }
            "quest.isComplete" => {
                let ordinal = int(&a["quest"]);
                let name = quest_names[ordinal as usize].clone();
                let value = persisted.quests.is_complete(
                    &world.quests,
                    ordinal,
                    &name,
                    &mut |level, message| rest.log(level, message),
                )?;
                out.insert("value".into(), json!(value));
            }
            "quest.hasStarted" => {
                out.insert(
                    "value".into(),
                    json!(
                        persisted
                            .quests
                            .has_started(&world.quests, int(&a["quest"]))?
                    ),
                );
            }
            "quest.totals" => {
                out.insert(
                    "value".into(),
                    json!([
                        world.quests.available_points(),
                        persisted.quests.has_completed_all(&world.quests)
                    ]),
                );
            }
            // Music.
            "music.put" => persisted.music.put(&world.songs, int(&a["id"]))?,
            "music.clear" => persisted.music.clear_unlocked(),
            "music.query" => {
                let id = int(&a["id"]);
                let m = &persisted.music;
                out.insert(
                    "value".into(),
                    json!([
                        m.has_unlocked(&world.songs, id),
                        m.has_unlocked_index(id),
                        m.has_air_guitar(&world.songs)
                    ]),
                );
            }
            // Emotes.
            "emote.add" => persisted.emotes.emotes.push(int(&a["ordinal"])),
            "emote.remove" => {
                out.insert(
                    "value".into(),
                    json!(persisted.emotes.remove(int(&a["ordinal"]))),
                );
            }
            "emote.clear" => persisted.emotes.emotes.clear(),
            "emote.isUnlocked" => {
                out.insert(
                    "value".into(),
                    json!(persisted.emotes.is_unlocked(int(&a["ordinal"]))),
                );
            }
            // Achievement diaries.
            "diary.parse" => {
                let data =
                    parser::parse_text(&format!("{{\"d\":{}}}", a["text"].as_str().unwrap()))?;
                let Some(Json::Array(values)) = data.get("d") else {
                    panic!("diary.parse takes an array")
                };
                persisted
                    .diaries
                    .parse(values, &mut world.completed_levels)?;
            }
            "diary.raw" => {
                let d = &mut persisted.diaries.diaries[int(&a["type"]) as usize];
                for (i, v) in a["started"].as_array().into_iter().flatten().enumerate() {
                    d.started[i] = v.as_bool().unwrap();
                }
                for (i, v) in a["rewarded"].as_array().into_iter().flatten().enumerate() {
                    d.rewarded[i] = v.as_bool().unwrap();
                }
                for row in a["tasks"].as_array().into_iter().flatten() {
                    d.completed[int(&row[0]) as usize][int(&row[1]) as usize] =
                        row[2].as_bool().unwrap();
                }
            }
            "diary.reset" => persisted.diaries.reset_rewards(core, definitions, rest)?,
            "diary.table" => {
                let table = diaries::TYPES
                    .iter()
                    .map(|t| {
                        json!([
                            t.name,
                            t.display,
                            t.levels,
                            t.rewards
                                .iter()
                                .map(|l| l
                                    .iter()
                                    .map(|(id, amount, charge)| json!([id, amount, charge]))
                                    .collect::<Vec<_>>())
                                .collect::<Vec<_>>()
                        ])
                    })
                    .collect::<Vec<_>>();
                out.insert("value".into(), json!(table));
            }
            // Spells.
            "autocast" => match a["spell"].as_array() {
                None => persisted.spells.autocast = None,
                Some(spell) => {
                    persisted
                        .spells
                        .set(&world.spells, int(&spell[0]), int(&spell[1]))?
                }
            },
            "spellbook" => *spell_book = int(&a["value"]),
            // Grave type.
            "grave" => native!().update_grave_type(attributes, int(&a["ordinal"]))?,
            "grave.get" => {
                out.insert("value".into(), json!(graves::get(attributes)?));
            }
            // Runecrafting pouches.
            "pouch.parse" => {
                let data =
                    parser::parse_text(&format!("{{\"d\":{}}}", a["text"].as_str().unwrap()))?;
                let Some(Json::Array(values)) = data.get("d") else {
                    panic!("pouch.parse takes an array")
                };
                persisted.pouches.parse(values, factory, definitions)?;
            }
            "pouch.set" => {
                let pouch = persisted
                    .pouches
                    .get_mut(int(&a["id"]))
                    .ok_or(save::Error::Null)?;
                if let Some(charges) = a["charges"].as_i64() {
                    pouch.charges = charges as i32;
                }
                if let Some(cap) = a["cap"].as_i64() {
                    pouch.current_cap = cap as i32;
                }
                if a["remake"] == true {
                    pouch.remake_container(factory)?;
                }
                for row in a["items"].as_array().into_iter().flatten() {
                    let added =
                        pouches::Manager::add(pouch, int(&row[0]), int(&row[1]), definitions)?;
                    out.insert("added".into(), json!(added));
                }
            }
            // POH storage.
            "poh.add" => {
                for id in a["ids"].as_array().unwrap() {
                    persisted.poh.containers[int(&a["family"]) as usize].add_item(int(id));
                }
            }
            "poh.withdraw" => {
                let value =
                    persisted.poh.containers[int(&a["family"]) as usize].withdraw(int(&a["id"]));
                out.insert("value".into(), json!(value));
            }
            "poh.page" => persisted.poh.containers[int(&a["family"]) as usize]
                .next_page(int(&a["total"]), int(&a["size"])),
            "poh.read" => persisted
                .poh
                .read_json(&parser::parse_text(a["text"].as_str().unwrap())?)?,
            "poh.clear" => persisted.poh.clear(),
            // House and familiars: scripted values for the saving side only.
            "house" => {
                let h = &mut persisted.house;
                h.location = int(&a["location"]);
                h.style = int(&a["style"]);
                h.crest = int(&a["crest"]);
                h.servant = a["servant"].as_object().map(|s| Servant {
                    kind: int(&s["type"]),
                    uses: int(&s["uses"]),
                    item: s["item"].as_array().map(|i| (int(&i[0]), int(&i[1]))),
                    greet: s["greet"] == true,
                });
                h.rooms = vec![vec![vec![None; 8]; 8]; 4];
                for r in a["rooms"].as_array().into_iter().flatten() {
                    h.rooms[int(&r["z"]) as usize][int(&r["x"]) as usize][int(&r["y"]) as usize] =
                        Some(Room {
                            properties: int(&r["properties"]),
                            rotation: int(&r["rotation"]),
                            hotspots: r["hotspots"].as_array().unwrap().iter().map(int).collect(),
                        });
                }
            }
            "pets" => {
                for row in a["pets"].as_array().unwrap() {
                    let details = row[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|pair| PetDetail {
                            hunger: java_double::parse(pair[0].as_str().unwrap()).unwrap(),
                            growth: java_double::parse(pair[1].as_str().unwrap()).unwrap(),
                        })
                        .collect();
                    persisted.familiars.pets.put(int(&row[0]), details);
                }
            }
            "familiar" => {
                let d = &a["familiar"];
                if !d.is_null() {
                    // State.kt's stand-in Familiar is an Entity: building it
                    // constructs Skills, which reports its combat level. That
                    // is the scripting, not the save; it is mirrored here.
                    rest.skill_call("combat", 3)
                        .map_err(|_| save::Error::Host("combat".into()))?;
                }
                persisted.familiars.active = if d.is_null() {
                    ActiveFamiliar::None
                } else if let Some(pet) = d.get("pet") {
                    ActiveFamiliar::Pet { item_id: int(pet) }
                } else {
                    let burden = d.get("burden").map(|rows| {
                        let container = factory
                            .create(30, ContainerType::Default, SortType::Id)
                            .unwrap();
                        for row in rows.as_array().unwrap() {
                            let mut item =
                                Item::new(int(&row[1]), int(&row[2]), int(&row[3]), definitions);
                            item.index = int(&row[0]);
                            container.items().write()[int(&row[0]) as usize] =
                                Some(Shared::new(item));
                        }
                        container
                    });
                    ActiveFamiliar::Familiar {
                        original_id: int(&d["originalId"]),
                        ticks: int(&d["ticks"]),
                        special_points: int(&d["specialPoints"]),
                        burden,
                        lifepoints: int(&d["lifepoints"]),
                    }
                };
            }
            // The profile state the ported systems reach.
            "item" => {
                let c = match a["container"].as_str().unwrap() {
                    "bank" => &mut core.primary.container,
                    "secondary" => &mut core.secondary.container,
                    "equipment" => &mut core.equipment,
                    "inventory" => &mut core.inventory,
                    other => panic!("unknown container {other}"),
                };
                let slot = int(&a["slot"]);
                let item = (a["null"] != true).then(|| {
                    let charge = a["charge"].as_i64().map_or(1000, |v| v as i32);
                    let mut item = Item::new(int(&a["id"]), int(&a["amount"]), charge, definitions);
                    item.index = slot;
                    Shared::new(item)
                });
                c.items().write()[slot as usize] = item;
            }
            "secondary" => core.use_secondary = a["value"].as_bool().unwrap(),
            "attr" => {
                let v = &a["value"];
                let value = match a["kind"].as_str().unwrap() {
                    "int" => AttributeValue::Int(int(v)),
                    "long" => AttributeValue::Long(v.as_i64().unwrap()),
                    "bool" => AttributeValue::Bool(v.as_bool().unwrap()),
                    "str" => AttributeValue::Text(v.as_str().unwrap().to_owned()),
                    other => panic!("unknown attribute kind {other}"),
                };
                attributes.set(a["key"].as_str().unwrap(), Some(value));
            }
            other => panic!("unknown state action {other}"),
        }
        Ok(())
    }
}

/// Replays one state case and returns what State.kt records for it.
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
    let mut steps = Vec::new();
    for a in request["actions"].as_array().unwrap() {
        let mut out = serde_json::Map::new();
        out.insert("error".into(), Value::Null);
        if let Err(error) = case.run(a, &mut out, &clock) {
            out.insert("error".into(), json!(category(&error)));
        }
        out.insert(
            "trace".into(),
            json!(std::mem::take(&mut case.recorder.lock().unwrap().trace)),
        );
        let mut changed = serde_json::Map::new();
        for (key, value) in case.sections() {
            if case.last.get(&key) != Some(&value) {
                case.last.insert(key.clone(), value.clone());
                changed.insert(key, value);
            }
        }
        out.insert("state".into(), Value::Object(changed));
        steps.push(Value::Object(out));
    }
    json!(steps)
}
