//! Replays the engine timer corpus (fixtures/content-timers) through
//! `timers::content` on the generic `timers::Manager` and `timers::registry`.
//! The host answers every service from the case the way the oracle's recording
//! adapters do; skills and special energy are the native `Skills` and
//! `Settings`. Every operation's result, error, recorded calls and draws, the
//! entities' state and every labelled timer (its RSTimer fields and its own
//! fields) are compared whole.
#[allow(dead_code)]
#[path = "../../../oracle/inventory/listeners.rs"]
mod listeners;

use rs09_game_core::{
    java_json::{self, Json},
    random::JavaRandom,
};
use rs09_server::definitions::item::Registry as ItemRegistry;
use rs09_server::inventory::container::Factory;
use rs09_server::player::{
    attributes::AttributeLocation,
    core_data::{CoreData, DEFAULT_BANK_SIZE},
    settings::{self, Settings},
    skills::{self, Skills},
};
use rs09_server::timers::{
    Error, Handle, Manager, Result,
    content::{
        self, Arg, EntityKind, Event, EventClass, Hook, Host, Kind, Logout, Natives, State, Tab,
        antimacro,
    },
    registry::{Level, Registry},
};
use rs09_server::world::combat_pulse::HitsplatType;
use rs09_server::world::prayer::PrayerType;
use rs09_server::world::spells::book::{SpellBook, SpellbookChangeSource};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/content-timers/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/content-timers/provenance.json");

/// Differences between the original and the port, by case group and the
/// reason. Empty: every case matches. A closed gap or a new difference fails.
const GAPS: &[(&str, &str)] = &[];

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

/// A hook in an entity's hook map: one of the timers' or one the case adds.
#[derive(Clone)]
enum Hooked {
    Timer(Hook),
    Foreign(String),
}

impl Hooked {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Timer(a), Self::Timer(b)) => a.same(b),
            (Self::Foreign(_), Self::Foreign(_)) => std::ptr::eq(self, other),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Attr {
    Bool(bool),
    Number(i64),
    Npc(usize),
}

struct Ent {
    kind: EntityKind,
    spec: Map<String, Value>,
    skills: Skills,
    settings: Settings,
    book: i32,
    attrs: Vec<(String, Attr)>,
    hooks: Vec<(EventClass, Vec<Hooked>)>,
    logout: Vec<(String, Logout)>,
    manager: Option<Manager>,
}

/// The recorder and the context the native skills and settings call back
/// into.
struct Probe {
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    current: usize,
    kind: EntityKind,
    sponge: bool,
}

impl Probe {
    fn emit(&mut self, event: Value) -> Result<()> {
        self.events.push(event);
        self.calls += 1;
        if self.calls == self.fail_at {
            Err(Error::Binding("injected".into()))
        } else {
            Ok(())
        }
    }
    fn ok(&mut self, event: Value) -> bool {
        self.emit(event).is_ok()
    }
}

impl skills::Context for Probe {
    fn kind(&self) -> skills::EntityKind {
        match self.kind {
            EntityKind::Player => skills::EntityKind::Player,
            EntityKind::Npc | EntityKind::Familiar => skills::EntityKind::Npc,
            EntityKind::Other => skills::EntityKind::Other,
        }
    }
    fn version(&self) -> i32 {
        unreachable!()
    }
    fn tick(&mut self) -> i32 {
        0
    }
    fn world_tick(&mut self) -> i32 {
        unreachable!()
    }
    fn combat_level(&self) -> i32 {
        unreachable!()
    }
    fn summoning_combat_level(&self) -> i32 {
        unreachable!()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        unreachable!()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        if !self.ok(json!(["perk", self.current, "DAMAGE_SPONG"])) {
            return Err(skills::Error::Binding);
        }
        Ok(self.sponge)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        // getExtension(AssistSession.class): never present in the corpus.
        if !self.ok(json!(["extension", self.current, "AssistSession"])) {
            return Err(skills::Error::Binding);
        }
        Ok(None)
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
        unreachable!()
    }
    fn effect(&mut self, _: &mut Skills, effect: skills::Effect) -> skills::Result<()> {
        match effect {
            skills::Effect::Packet(slot) => {
                if self.ok(json!(["skill_packet", self.current, slot])) {
                    Ok(())
                } else {
                    Err(skills::Error::Binding)
                }
            }
            // Skills::new's initial combat level: the test's construction.
            skills::Effect::Combat(_) => Ok(()),
            other => unreachable!("{other:?}"),
        }
    }
}

impl settings::Context for Probe {
    fn varp(&mut self, _: &Settings, id: i32, value: i32) -> settings::Result<()> {
        // setVarp(player, 300, specialEnergy * 10) with save false.
        if self.ok(json!(["varp", self.current, id, value, false])) {
            Ok(())
        } else {
            Err(settings::Error::Binding)
        }
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
        unreachable!()
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!()
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!()
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

struct Npc {
    label: String,
    loot: Option<String>,
    missing: bool,
}

struct World {
    input: Value,
    probe: Probe,
    tick: i32,
    rng: JavaRandom,
    ents: Vec<Ent>,
    labels: Vec<(Handle, String)>,
    auto: usize,
    npcs: Vec<Npc>,
    non_skill: usize,
    registry: Registry,
    /// RandomEvents: name, loot table label, type.
    events_table: Vec<(String, Option<String>, String)>,
}

fn error_name(error: &Error) -> &'static str {
    match error {
        Error::Null => "Null",
        Error::Type => "Cast",
        Error::Number => "Number",
        Error::Index => "Index",
        Error::ConcurrentModification => "ConcurrentModification",
        Error::Binding(what) if what == content::UNINITIALIZED => "Uninitialized",
        // An injected failure, directly or through the native skills/settings.
        Error::Binding(what) if what == "injected" || what == "skills" || what == "settings" => {
            "Binding"
        }
        Error::Binding(what) => panic!("unexpected binding failure: {what}"),
    }
}

impl World {
    fn emit(&mut self, event: Value) -> Result<()> {
        self.probe.emit(event)
    }

    fn spec(&self, e: usize, key: &str) -> Option<&Value> {
        self.ents[e].spec.get(key).filter(|v| !v.is_null())
    }

    fn flag(&self, e: usize, key: &str) -> bool {
        self.spec(e, key).and_then(Value::as_bool).unwrap_or(false)
    }

    fn label(&mut self, timer: &Handle) -> String {
        if let Some((_, name)) = self.labels.iter().find(|(t, _)| t.same(timer)) {
            return name.clone();
        }
        let name = format!("t{}", self.auto);
        self.auto += 1;
        self.labels.push((timer.clone(), name.clone()));
        name
    }

    fn name_timer(&mut self, timer: &Handle, name: &str) {
        if !self.labels.iter().any(|(t, _)| t.same(timer)) {
            self.labels.push((timer.clone(), name.to_owned()));
        }
    }

    fn timer(&self, op: &Value) -> Option<Handle> {
        let name = op["timer"].as_str()?;
        self.labels
            .iter()
            .find(|(_, n)| n == name)
            .map(|(t, _)| t.clone())
    }

    fn hook_label(&mut self, hook: &Hooked) -> String {
        match hook {
            Hooked::Foreign(name) => format!("foreign:{name}"),
            Hooked::Timer(Hook::PrayerActivated) => "PrayerActivatedHook".into(),
            Hooked::Timer(Hook::PrayerDeactivated) => "PrayerDeactivatedHook".into(),
            Hooked::Timer(Hook::SpellCast(t)) => format!("{}:spellCastHook", self.label(t)),
            Hooked::Timer(Hook::SpellbookChange(t)) => {
                format!("{}:spellBookChangeHook", self.label(t))
            }
            Hooked::Timer(Hook::DialogueClose(t)) => format!("{}:dialogueCloseHook", self.label(t)),
        }
    }

    fn attr(&self, e: usize, key: &str) -> Option<&Attr> {
        self.ents[e]
            .attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    fn set_attr(&mut self, e: usize, key: &str, value: Option<Attr>) {
        let attrs = &mut self.ents[e].attrs;
        match value {
            None => attrs.retain(|(k, _)| k != key),
            Some(value) => match attrs.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => attrs.push((key.to_owned(), value)),
            },
        }
    }

    fn attr_json(&self, value: &Attr) -> Value {
        match value {
            Attr::Bool(b) => json!(b),
            Attr::Number(n) => json!(n),
            Attr::Npc(i) => json!({"npc": self.npcs[*i].label}),
        }
    }

    fn apply(&mut self, e: usize, fields: &Map<String, Value>) {
        for (key, v) in fields {
            let ent = &mut self.ents[e];
            match key.as_str() {
                "static" => {
                    for (slot, level) in v.as_object().unwrap() {
                        ent.skills.static_levels[slot.parse::<usize>().unwrap()] = int(level);
                    }
                }
                "dynamic" => {
                    for (slot, level) in v.as_object().unwrap() {
                        ent.skills.dynamic_levels[slot.parse::<usize>().unwrap()] = int(level);
                    }
                }
                "lp" => ent.skills.lifepoints = int(v),
                "lastTrained" => ent.skills.last_trained_skill = int(v),
                "lastXpGain" => ent.skills.last_xp_gain = int(v),
                "special" if ent.kind.is_player() => {
                    ent.settings.fields.write().special_energy = int(v)
                }
                "book" if ent.kind.is_player() => ent.book = int(v),
                "attrs" => {
                    for (k, a) in v.as_object().unwrap() {
                        let value = match a {
                            Value::Null => None,
                            Value::Bool(b) => Some(Attr::Bool(*b)),
                            Value::Number(n) => Some(Attr::Number(n.as_i64().unwrap())),
                            Value::Object(o) => {
                                Some(Attr::Npc(o["npc"].as_u64().unwrap() as usize))
                            }
                            other => panic!("attribute {other}"),
                        };
                        self.set_attr(e, k, value);
                    }
                }
                _ => {
                    ent.spec.insert(key.clone(), v.clone());
                }
            }
        }
    }

    fn arg(&self, v: &Value) -> Arg<usize> {
        match v {
            Value::Number(n) => match n.as_i64() {
                // JSON integers read back as Long, then toInt() (an Int).
                Some(n) => Arg::Int(n as i32),
                None => Arg::Other,
            },
            Value::Bool(b) => Arg::Bool(*b),
            Value::Object(o) if o.contains_key("entity") => Arg::Entity(int(&o["entity"]) as usize),
            _ => Arg::Other,
        }
    }

    fn args(&self, op: &Value) -> Vec<Arg<usize>> {
        op["args"]
            .as_array()
            .map(|a| a.iter().map(|v| self.arg(v)).collect())
            .unwrap_or_default()
    }

    fn fields(&self, timer: &Handle) -> Value {
        let state: State<usize, String> = content::state(timer).unwrap();
        match state {
            State::None => json!({}),
            State::SkillRestore { since, restore } => {
                json!({"since": since.to_vec(), "restore": restore.to_vec()})
            }
            State::Poison { source, severity } => json!({
                "source": source.map_or(json!("uninitialized"), |s| json!(s)),
                "severity": severity
            }),
            State::Disease { hits_left } => json!({"hitsLeft": hits_left}),
            State::Remaining { ticks } => json!({"ticksRemaining": ticks}),
            State::Frozen { apply_immunity } => json!({"applyImmunity": apply_immunity}),
            State::Heal {
                remaining,
                per_tick,
            } => json!({"healRemaining": remaining, "healPerTick": per_tick}),
            State::AntiMacro {
                paused,
                next_random,
            } => json!({"paused": paused, "nextRandom": next_random}),
        }
    }

    fn describe_timer(&self, timer: &Handle) -> Value {
        let fields = self.fields(timer);
        let t = timer.read();
        json!([
            t.class_name,
            t.identifier,
            t.run_interval,
            t.next_execution,
            t.last_execution,
            t.is_soft,
            t.is_auto,
            t.flags.iter().map(|f| format!("{f:?}")).collect::<Vec<_>>(),
            t.persistent,
            fields
        ])
    }

    fn entity_state(&mut self, e: usize) -> Value {
        let mut m = Map::new();
        let s = &self.ents[e].skills;
        m.insert("lp".into(), json!(s.lifepoints));
        m.insert("lpIncrease".into(), json!(s.lifepoints_increase));
        m.insert("lpUpdate".into(), json!(s.lifepoints_update));
        m.insert("dynamic".into(), json!(s.dynamic_levels.to_vec()));
        if self.ents[e].kind.is_player() {
            m.insert(
                "special".into(),
                json!(self.ents[e].settings.fields.read().special_energy),
            );
            m.insert("book".into(), json!(self.ents[e].book));
            let mut keys: Vec<String> =
                self.ents[e].logout.iter().map(|(k, _)| k.clone()).collect();
            keys.sort();
            m.insert("logout".into(), json!(keys));
        }
        let manager = self.ents[e].manager.take().unwrap();
        for (key, list) in [
            ("active", &manager.active),
            ("pending", &manager.pending),
            ("removing", &manager.removing),
        ] {
            let values: Vec<Option<Handle>> = list.read().values().to_vec();
            let labels: Vec<Value> = values
                .iter()
                .map(|t| t.as_ref().map_or(Value::Null, |t| json!(self.label(t))))
                .collect();
            m.insert(key.into(), json!(labels));
        }
        self.ents[e].manager = Some(manager);
        let attrs: Map<String, Value> = self.ents[e]
            .attrs
            .iter()
            .map(|(k, v)| (k.clone(), self.attr_json(v)))
            .collect();
        m.insert("attrs".into(), Value::Object(attrs));
        let mut hooks = Map::new();
        let lists = self.ents[e].hooks.clone();
        for (class, list) in lists {
            let labels: Vec<String> = list.iter().map(|h| self.hook_label(h)).collect();
            hooks.insert(class.simple_name().into(), json!(labels));
        }
        m.insert("hooks".into(), Value::Object(hooks));
        Value::Object(m)
    }

    fn natives(&mut self, e: usize) -> Natives<'_, World> {
        let registry = self.registry.clone();
        Natives {
            host: self,
            entity: e,
            registry,
        }
    }

    fn event(&self, v: &Value) -> Event<String> {
        let book = |k: &str| SpellBook::from_name(v[k].as_str().unwrap()).unwrap();
        let prayer = || {
            *PrayerType::ALL
                .iter()
                .find(|p| p.name() == v["prayer"].as_str().unwrap())
                .unwrap()
        };
        match v["type"].as_str().unwrap() {
            "PrayerActivated" => Event::PrayerActivated(prayer()),
            "PrayerDeactivated" => Event::PrayerDeactivated(prayer()),
            "SpellCast" => Event::SpellCast {
                book: book("book"),
                spell: int(&v["spell"]),
            },
            "SpellbookChange" => Event::SpellbookChange {
                old: book("old"),
                new: book("new"),
                source: source(v["source"].as_str().unwrap()),
            },
            "DialogueClose" => Event::DialogueClose(v["dialogue"].as_str().unwrap().to_owned()),
            other => panic!("event {other}"),
        }
    }

    fn describe_event(&self, event: &Event<String>) -> Value {
        match event {
            Event::PrayerActivated(t) => json!(["PrayerActivatedEvent", t.name()]),
            Event::PrayerDeactivated(t) => json!(["PrayerDeactivatedEvent", t.name()]),
            Event::SpellCast { book, spell } => json!(["SpellCastEvent", book.name(), spell]),
            Event::SpellbookChange { old, new, source } => {
                json!([
                    "SpellbookChangeEvent",
                    old.name(),
                    new.name(),
                    source_name(*source)
                ])
            }
            Event::DialogueClose(label) => json!(["DialogueCloseEvent", label]),
        }
    }

    fn event_class(name: &str) -> EventClass {
        match name {
            "PrayerActivated" => EventClass::PrayerActivated,
            "PrayerDeactivated" => EventClass::PrayerDeactivated,
            "SpellCast" => EventClass::SpellCast,
            "SpellbookChange" => EventClass::SpellbookChanged,
            "DialogueClose" => EventClass::DialogueClosed,
            other => panic!("{other}"),
        }
    }

    fn hook_into(&mut self, e: usize, class: EventClass, hook: Hooked) {
        let hooks = &mut self.ents[e].hooks;
        let at = match hooks.iter().position(|(c, _)| *c == class) {
            Some(at) => at,
            None => {
                hooks.push((class, Vec::new()));
                hooks.len() - 1
            }
        };
        if !hooks[at].1.iter().any(|h| h.same(&hook)) {
            hooks[at].1.push(hook);
        }
    }
}

fn source(name: &str) -> SpellbookChangeSource {
    match name {
        "ALTAR" => SpellbookChangeSource::Altar,
        "MAGIC_CAPE_PERK" => SpellbookChangeSource::MagicCapePerk,
        "SPELLBOOK_SWAP_CAST" => SpellbookChangeSource::SpellbookSwapCast,
        "SPELLBOOK_SWAP_RESTORE" => SpellbookChangeSource::SpellbookSwapRestore,
        other => panic!("{other}"),
    }
}

fn source_name(source: SpellbookChangeSource) -> &'static str {
    match source {
        SpellbookChangeSource::Altar => "ALTAR",
        SpellbookChangeSource::MagicCapePerk => "MAGIC_CAPE_PERK",
        SpellbookChangeSource::SpellbookSwapCast => "SPELLBOOK_SWAP_CAST",
        SpellbookChangeSource::SpellbookSwapRestore => "SPELLBOOK_SWAP_RESTORE",
    }
}

impl Host for World {
    type Entity = usize;
    type RandomEvent = String;
    type EventNpc = usize;
    type Weapon = i32;
    type Dialogue = String;
    type SkillsContext = Probe;
    type SettingsContext = Probe;

    fn kind(&self, entity: &usize) -> EntityKind {
        self.ents[*entity].kind
    }
    fn skills(&mut self, entity: &usize) -> (&mut Skills, &mut Probe) {
        let ent = &mut self.ents[*entity];
        self.probe.current = *entity;
        self.probe.kind = ent.kind;
        self.probe.sponge = ent
            .spec
            .get("sponge")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        (&mut ent.skills, &mut self.probe)
    }
    fn settings(&mut self, player: &usize) -> (Settings, &mut Probe) {
        self.probe.current = *player;
        self.probe.kind = self.ents[*player].kind;
        (self.ents[*player].settings.clone(), &mut self.probe)
    }
    fn spell_book(&mut self, player: &usize) -> &mut i32 {
        &mut self.ents[*player].book
    }
    fn random_event_name(&self, event: &String) -> String {
        event.clone()
    }
    fn random_event_value_of(&self, name: &str) -> Option<String> {
        self.events_table
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(n, _, _)| n.clone())
    }

    fn ticks(&mut self) -> Result<i32> {
        let tick = self.tick;
        self.emit(json!(["ticks", tick]))?;
        Ok(tick)
    }
    fn modal_open(&mut self, player: &usize) -> Result<bool> {
        self.emit(json!(["modal", player]))?;
        Ok(self.flag(*player, "modal"))
    }
    fn script_delay(&mut self, entity: &usize) -> Result<i32> {
        self.emit(json!(["delay", entity]))?;
        Ok(self.spec(*entity, "delay").map_or(0, int))
    }
    fn remove_weak_scripts(&mut self, entity: &usize) -> Result<()> {
        self.emit(json!(["remove_weak_scripts", entity]))
    }
    fn name(&mut self, entity: &usize) -> Result<Option<String>> {
        self.emit(json!(["name", entity]))?;
        Ok(self
            .spec(*entity, "name")
            .and_then(Value::as_str)
            .map(str::to_owned))
    }
    fn location(&mut self, entity: &usize) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["location", entity]))?;
        Ok(self
            .spec(*entity, "location")
            .map(|l| [int(&l[0]), int(&l[1]), int(&l[2])]))
    }
    fn log(&mut self, level: Level, message: &str) -> Result<()> {
        let level = match level {
            Level::Warn => "WARN",
            Level::Error => "ERR",
        };
        self.emit(json!(["log", level, message]))
    }
    fn exception(&mut self, error: &Error) -> Result<()> {
        self.probe
            .events
            .push(json!(["exception", error_name(error)]));
        Ok(())
    }
    fn send_message(&mut self, player: &usize, text: &str) -> Result<()> {
        self.emit(json!(["message", player, text]))
    }
    fn debug(&mut self, player: &usize, text: &str) -> Result<()> {
        self.emit(json!(["debug", player, text]))
    }
    fn play_audio(&mut self, player: &usize, id: i32, delay: i32, loops: i32) -> Result<()> {
        self.emit(json!(["audio", player, id, delay, loops, null, 8]))
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        let value = self.rng.next_bound(bound).unwrap();
        self.emit(json!(["next_int", bound, value]))?;
        Ok(value)
    }
    fn manual_hit(
        &mut self,
        owner: &usize,
        source: &usize,
        hit: i32,
        kind: HitsplatType,
    ) -> Result<()> {
        self.emit(json!(["manual_hit", owner, source, hit, kind.name()]))
    }
    fn worn(&mut self, player: &usize, slot: i32) -> Result<Option<i32>> {
        self.emit(json!(["equipment", player, slot]))?;
        Ok(self
            .spec(*player, "worn")
            .and_then(|w| w.get(slot.to_string()))
            .map(int))
    }
    fn artificial(&mut self, player: &usize) -> Result<bool> {
        self.emit(json!(["artificial", player]))?;
        Ok(self.flag(*player, "artificial"))
    }
    fn administrator(&mut self, player: &usize) -> Result<bool> {
        self.emit(json!(["rights", player]))?;
        Ok(self.spec(*player, "rights").and_then(Value::as_str) == Some("ADMINISTRATOR"))
    }
    fn uid(&mut self, player: &usize) -> Result<i32> {
        self.emit(json!(["uid", player]))?;
        Ok(self.spec(*player, "uid").map_or(0, int))
    }
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<usize>> {
        self.emit(json!(["player_by_uid", uid]))?;
        Ok(self.input["uids"]
            .get(uid.to_string())
            .map(|i| int(i) as usize))
    }
    fn npc_task(&mut self, npc: &usize) -> Result<Option<i32>> {
        self.emit(json!(["npc_task", npc]))?;
        Ok(self.spec(*npc, "task").map(int))
    }
    fn familiar_owner(&mut self, familiar: &usize) -> Result<Option<usize>> {
        self.emit(json!(["familiar_owner", familiar]))?;
        Ok(self.spec(*familiar, "owner").map(|o| int(o) as usize))
    }
    fn poison_immune(&mut self, entity: &usize) -> Result<bool> {
        self.emit(json!(["poison_immune", entity]))?;
        Ok(self.flag(*entity, "immune"))
    }
    fn set_skull_icon(&mut self, player: &usize, icon: i32) -> Result<()> {
        self.emit(json!(["skull_icon", player, icon]))
    }
    fn set_skulled(&mut self, player: &usize, skulled: bool) -> Result<()> {
        self.emit(json!(["set_skulled", player, skulled]))
    }
    fn reset_skull(&mut self, player: &usize) -> Result<()> {
        self.emit(json!(["reset_skull", player]))
    }
    fn hook(&mut self, entity: &usize, class: EventClass, hook: Hook) -> Result<()> {
        let hooked = Hooked::Timer(hook);
        let label = self.hook_label(&hooked);
        self.emit(json!(["hook", entity, class.simple_name(), label]))?;
        self.hook_into(*entity, class, hooked);
        Ok(())
    }
    fn unhook(&mut self, entity: &usize, hook: &Hook) -> Result<()> {
        let hooked = Hooked::Timer(hook.clone());
        let label = self.hook_label(&hooked);
        self.emit(json!(["unhook", entity, label]))?;
        for (_, list) in &mut self.ents[*entity].hooks {
            if let Some(at) = list.iter().position(|h| h.same(&hooked)) {
                list.remove(at);
            }
        }
        Ok(())
    }
    fn dispatch(
        &mut self,
        entity: &usize,
        event: Event<String>,
        timers: &mut Manager,
        registry: &Registry,
    ) -> Result<()> {
        let description = self.describe_event(&event);
        self.emit(json!(["dispatch", entity, description]))?;
        let class = event.class();
        let Some(list) = self.ents[*entity]
            .hooks
            .iter()
            .find(|(c, _)| *c == class)
            .map(|(_, l)| l.clone())
        else {
            return Ok(());
        };
        for hook in list {
            match hook {
                Hooked::Foreign(name) => {
                    let description = self.describe_event(&event);
                    self.emit(json!(["foreign_hook", name, entity, description]))?;
                }
                Hooked::Timer(hook) => {
                    let mut natives = Natives {
                        host: &mut *self,
                        entity: *entity,
                        registry: registry.clone(),
                    };
                    content::process_hook(&mut natives, timers, &hook, &event)?;
                }
            }
        }
        Ok(())
    }
    fn register_logout_listener(
        &mut self,
        player: &usize,
        key: &str,
        listener: Logout,
    ) -> Result<()> {
        self.emit(json!(["register_logout", player, key]))?;
        let list = &mut self.ents[*player].logout;
        match list.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = listener,
            None => list.push((key.to_owned(), listener)),
        }
        Ok(())
    }
    fn clear_logout_listener(&mut self, player: &usize, key: &str) -> Result<()> {
        self.emit(json!(["clear_logout", player, key]))?;
        self.ents[*player].logout.retain(|(k, _)| k != key);
        Ok(())
    }
    fn weapon_interface(&mut self, player: &usize) -> Result<Option<i32>> {
        self.emit(json!(["extension", player, "WeaponInterface"]))?;
        Ok(self.spec(*player, "weapon").map(int))
    }
    fn autocast_set(&mut self, player: &usize) -> Result<bool> {
        self.emit(json!(["autocast", player]))?;
        Ok(self.flag(*player, "autocast"))
    }
    fn select_auto_spell(
        &mut self,
        player: &usize,
        weapon: &i32,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> Result<()> {
        self.emit(json!([
            "select_auto_spell",
            player,
            weapon,
            button,
            adjust,
            forget
        ]))
    }
    fn open_tab(&mut self, player: &usize, tab: Tab<'_, i32>) -> Result<()> {
        let id = match tab {
            Tab::Book(id) => id,
            Tab::Weapon(id) => *id,
        };
        self.emit(json!(["open_tab", player, id]))
    }
    fn set_varbit(&mut self, player: &usize, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varbit", player, id, value, save]))
    }
    fn attribute_flag(&mut self, entity: &usize, key: &str, default: bool) -> Result<bool> {
        self.emit(json!(["get_attribute", entity, key, default]))?;
        match self.attr(*entity, key) {
            None => Ok(default),
            Some(Attr::Bool(b)) => Ok(*b),
            Some(_) => Err(Error::Type),
        }
    }
    fn remove_attribute(&mut self, entity: &usize, key: &str) -> Result<()> {
        self.emit(json!(["remove_attribute", entity, key]))?;
        self.set_attr(*entity, key, None);
        Ok(())
    }
    fn event_npc(&mut self, player: &usize) -> Result<Option<usize>> {
        self.emit(json!(["get_attribute", player, antimacro::EVENT_NPC, null]))?;
        match self.attr(*player, antimacro::EVENT_NPC) {
            None => Ok(None),
            Some(Attr::Npc(i)) => Ok(Some(*i)),
            Some(_) => Err(Error::Type),
        }
    }
    fn set_event_npc(&mut self, player: &usize, npc: &usize) -> Result<()> {
        let label = self.npcs[*npc].label.clone();
        self.emit(json!(["set_attribute", player, antimacro::EVENT_NPC, {"npc": label}]))?;
        self.set_attr(*player, antimacro::EVENT_NPC, Some(Attr::Npc(*npc)));
        Ok(())
    }
    fn random_events_restricted(&mut self, player: &usize) -> Result<bool> {
        self.emit(json!(["zone_restricted", player, "RANDOM_EVENTS"]))?;
        Ok(self.flag(*player, "restricted"))
    }
    fn skill_random_event(&mut self, skill: i32) -> Result<Option<String>> {
        self.emit(json!(["skill_random", skill]))?;
        Ok(self.input["skillRandom"]
            .get(skill.to_string())
            .and_then(Value::as_str)
            .map(str::to_owned))
    }
    fn non_skill_random_event(&mut self) -> Result<String> {
        self.emit(json!(["non_skill_random"]))?;
        let pool = self.input["nonSkill"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![json!("GENIE")]);
        let name = pool[self.non_skill % pool.len()]
            .as_str()
            .unwrap()
            .to_owned();
        self.non_skill += 1;
        Ok(name)
    }
    fn create_event_npc(&mut self, event: &String, player: &usize) -> Result<usize> {
        let (_, loot, kind) = self
            .events_table
            .iter()
            .find(|(n, _, _)| n == event)
            .cloned()
            .unwrap();
        self.emit(json!(["create_event_npc", event, player, loot, kind]))?;
        let index = self.npcs.len();
        let missing = self.input["spawnMissing"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_u64() == Some(index as u64)));
        self.npcs.push(Npc {
            label: format!("npc{index}"),
            loot,
            missing,
        });
        Ok(index)
    }
    fn spawn_location_missing(&mut self, npc: &usize) -> Result<bool> {
        let label = self.npcs[*npc].label.clone();
        self.emit(json!(["spawn_location", label]))?;
        Ok(self.npcs[*npc].missing)
    }
    fn init_event_npc(&mut self, npc: &usize) -> Result<()> {
        let label = self.npcs[*npc].label.clone();
        self.emit(json!(["init_npc", label]))
    }
    fn terminate_event_npc(&mut self, npc: &usize) -> Result<()> {
        let label = self.npcs[*npc].label.clone();
        self.emit(json!(["terminate_npc", label]))
    }
    fn roll_event_loot(&mut self, npc: &usize, player: &usize) -> Result<Option<Vec<(i32, i32)>>> {
        let Some(table) = self.npcs[*npc].loot.clone() else {
            return Ok(None);
        };
        self.emit(json!(["roll_loot", table, player]))?;
        Ok(Some(
            self.input["loot"]
                .get(&table)
                .and_then(Value::as_array)
                .map(|rows| rows.iter().map(|r| (int(&r[0]), int(&r[1]))).collect())
                .unwrap_or_default(),
        ))
    }
    fn dialogue_ids(&mut self, dialogue: &String) -> Result<Option<Vec<i32>>> {
        self.emit(json!(["dialogue_ids", dialogue]))?;
        Ok(self.input["dialogues"]
            .get(dialogue)
            .and_then(Value::as_array)
            .map(|ids| ids.iter().map(int).collect()))
    }
}

fn kind_of(name: &str) -> Kind {
    *Kind::ALL.iter().find(|k| k.class_name() == name).unwrap()
}

/// One operation, as the oracle's Support.run.
fn run(world: &mut World, op: &Value) -> Result<Value> {
    let e = op["e"].as_u64().map_or(0, |e| e as usize);
    let class = op["class"].as_str();
    let mut manager = world.ents[e].manager.take().unwrap();
    let result = run_on(world, e, class, &mut manager, op);
    world.ents[e].manager = Some(manager);
    result
}

fn timer_result(world: &mut World, timer: Option<Handle>) -> Value {
    timer.map_or(Value::Null, |t| json!({"timer": world.label(&t)}))
}

fn set_field(world: &mut World, timer: &Handle, field: &str, v: &Value) {
    let n = || int(v);
    match field {
        "interval" => timer.write().run_interval = n(),
        "next" => timer.write().next_execution = n(),
        "last" => timer.write().last_execution = n(),
        _ => {
            let source = (field == "source").then(|| int(v) as usize);
            content::with_state(timer, |s: &mut State<usize, String>| match (field, s) {
                ("severity", State::Poison { severity, .. }) => *severity = n(),
                ("source", State::Poison { source: from, .. }) => *from = source,
                ("hitsLeft", State::Disease { hits_left }) => *hits_left = n(),
                ("ticksRemaining", State::Remaining { ticks }) => *ticks = n(),
                ("applyImmunity", State::Frozen { apply_immunity }) => {
                    *apply_immunity = v.as_bool().unwrap()
                }
                ("healRemaining", State::Heal { remaining, .. }) => *remaining = n(),
                ("healPerTick", State::Heal { per_tick, .. }) => *per_tick = n(),
                ("paused", State::AntiMacro { paused, .. }) => *paused = v.as_bool().unwrap(),
                ("nextRandom", State::AntiMacro { next_random, .. }) => {
                    *next_random = v.as_str().map(str::to_owned)
                }
                ("since", State::SkillRestore { since, .. }) => {
                    since[int(&v[0]) as usize] = int(&v[1])
                }
                ("restore", State::SkillRestore { restore, .. }) => {
                    restore[int(&v[0]) as usize] = int(&v[1])
                }
                (other, _) => panic!("field {other}"),
            })
            .unwrap();
        }
    }
    let _ = world;
}

fn object(text: &str) -> Json {
    let value = java_json::parse(text).unwrap();
    assert!(matches!(value, Json::Object(_)));
    value
}

fn run_on(
    world: &mut World,
    e: usize,
    class: Option<&str>,
    manager: &mut Manager,
    op: &Value,
) -> Result<Value> {
    let name = op["op"].as_str().unwrap();
    let args = world.args(op);
    Ok(match name {
        "set" => {
            world.apply(e, op["fields"].as_object().unwrap());
            Value::Null
        }
        "tick" => {
            world.tick = int(&op["value"]);
            Value::Null
        }
        "process" => {
            manager.process(&mut (), &mut world.natives(e))?;
            Value::Null
        }
        "run" => {
            for _ in 0..int(&op["n"]) {
                world.tick = world.tick.wrapping_add(1);
                manager.process(&mut (), &mut world.natives(e))?;
            }
            Value::Null
        }
        "spawn" | "start" => {
            let timer = content::spawn(
                &mut world.natives(e),
                manager,
                kind_of(class.unwrap()),
                &args,
            )?;
            if let Some(label) = op["label"].as_str() {
                world.name_timer(&timer, label);
            }
            if name == "start" {
                manager.register_optional(Some(&timer), &mut (), &mut world.natives(e))?;
            }
            timer_result(world, Some(timer))
        }
        "spawnId" => {
            let registry = world.registry.clone();
            let timer = registry.instance(
                op["id"].as_str().unwrap(),
                &args,
                manager,
                &mut (),
                &mut world.natives(e),
            )?;
            if let (Some(timer), Some(label)) = (&timer, op["label"].as_str()) {
                world.name_timer(timer, label);
            }
            timer_result(world, timer)
        }
        "register" => {
            let timer = world.timer(op);
            manager.register_optional(timer.as_ref(), &mut (), &mut world.natives(e))?;
            Value::Null
        }
        "getOrStart" => {
            let timer = content::get_or_start(
                &mut world.natives(e),
                manager,
                kind_of(class.unwrap()),
                &args,
            )?;
            timer_result(world, Some(timer))
        }
        "getOrStartId" => {
            let registry = world.registry.clone();
            let timer = registry.get_or_start(
                op["id"].as_str().unwrap(),
                &args,
                manager,
                &mut (),
                &mut world.natives(e),
            )?;
            timer_result(world, timer)
        }
        "has" => json!(content::has(manager, kind_of(class.unwrap()))?),
        "hasId" => json!(content::has_identifier(
            manager,
            op["id"].as_str().unwrap()
        )?),
        "get" => {
            let timer = content::get(manager, kind_of(class.unwrap()))?;
            timer_result(world, timer)
        }
        "remove" => {
            let timer = world.timer(op);
            manager.remove(timer.as_ref(), &mut (), &mut world.natives(e))?;
            Value::Null
        }
        "removeClass" => {
            let kind = kind_of(class.unwrap());
            manager.remove_where(|t| kind.matches(t), &mut (), &mut world.natives(e))?;
            Value::Null
        }
        "removeId" => {
            manager.remove_identifier(
                op["id"].as_str().unwrap(),
                &mut (),
                &mut world.natives(e),
            )?;
            Value::Null
        }
        "save" => {
            let mut root = Json::object();
            manager.save(&mut root, &mut (), &mut world.natives(e))?;
            json!(root.text())
        }
        "parse" => {
            let root = object(op["text"].as_str().unwrap());
            manager.parse(&root, &mut (), &mut world.natives(e))?;
            Value::Null
        }
        "timerSave" | "timerParse" => {
            let timer = world.timer(op).ok_or(Error::Null)?;
            if !timer.read().persistent {
                return Err(Error::Type);
            }
            let mut natives = world.natives(e);
            if name == "timerSave" {
                let mut root = Json::object();
                timers_context_save(&mut natives, &timer, &mut root, manager)?;
                json!(root.text())
            } else {
                let root = object(op["text"].as_str().unwrap());
                timers_context_parse(&mut natives, &timer, &root, manager)?;
                Value::Null
            }
        }
        "death" => {
            manager.death(&mut (), &mut world.natives(e))?;
            Value::Null
        }
        "clear" => {
            manager.clear();
            Value::Null
        }
        "auto" => {
            let registry = world.registry.clone();
            registry.add_automatic(manager, &mut (), &mut world.natives(e))?;
            Value::Null
        }
        "dispatch" => {
            let event = world.event(&op["event"]);
            let registry = world.registry.clone();
            world.dispatch(&e, event, manager, &registry)?;
            Value::Null
        }
        "hookForeign" => {
            let class = World::event_class(op["event"].as_str().unwrap());
            let label = op["name"].as_str().unwrap().to_owned();
            world.emit(json!([
                "hook",
                e,
                class.simple_name(),
                format!("foreign:{label}")
            ]))?;
            world.hook_into(e, class, Hooked::Foreign(label));
            Value::Null
        }
        "logout" => {
            let listeners = world.ents[e].logout.clone();
            for (_, listener) in listeners {
                content::logout(&mut world.natives(e), manager, &listener)?;
            }
            Value::Null
        }
        "applyPoison" => {
            let from = int(&op["source"]) as usize;
            content::apply_poison(&mut world.natives(e), manager, &from, int(&op["severity"]))?;
            Value::Null
        }
        "curePoison" => {
            content::cure_poison(&mut world.natives(e), manager)?;
            Value::Null
        }
        "isPoisoned" => json!(content::is_poisoned(manager)?),
        "pause" => {
            antimacro::pause(&mut world.natives(e), manager)?;
            Value::Null
        }
        "unpause" => {
            antimacro::unpause(&mut world.natives(e), manager)?;
            Value::Null
        }
        "forceEvent" => {
            let event = op["event"].as_str().map(str::to_owned);
            antimacro::force_event(&mut world.natives(e), manager, event)?;
            Value::Null
        }
        "eventNpc" => {
            let npc = antimacro::event_npc(&mut world.natives(e))?;
            npc.map_or(Value::Null, |i| json!({"npc": world.npcs[i].label}))
        }
        "terminate" => {
            antimacro::terminate_event_npc(&mut world.natives(e))?;
            Value::Null
        }
        "loot" => {
            let items = antimacro::roll_event_loot(&mut world.natives(e))?;
            json!(
                items
                    .iter()
                    .map(|(id, amount)| json!([id, amount]))
                    .collect::<Vec<_>>()
            )
        }
        "parseArgs" => {
            let command = op["command"].as_str().unwrap_or("revent");
            let (user, event) =
                antimacro::parse_command_args(&*world, op["text"].as_str().unwrap(), command);
            json!([user, event])
        }
        "field" => {
            let timer = world.timer(op).ok_or(Error::Null)?;
            set_field(world, &timer, op["field"].as_str().unwrap(), &op["value"]);
            Value::Null
        }
        "fieldOf" => {
            let timer = content::get(manager, kind_of(class.unwrap()))?.ok_or(Error::Null)?;
            set_field(world, &timer, op["field"].as_str().unwrap(), &op["value"]);
            Value::Null
        }
        other => panic!("unknown op {other}"),
    })
}

fn timers_context_save(
    natives: &mut Natives<'_, World>,
    timer: &Handle,
    root: &mut Json,
    manager: &mut Manager,
) -> Result<()> {
    rs09_server::timers::Context::<()>::save(natives, timer, root, manager, &mut ())
}

fn timers_context_parse(
    natives: &mut Natives<'_, World>,
    timer: &Handle,
    root: &Json,
    manager: &mut Manager,
) -> Result<()> {
    rs09_server::timers::Context::<()>::parse(natives, timer, root, manager, &mut ())
}

fn new_world(case: &Value, table: &[(String, Option<String>, String)]) -> World {
    let mut probe = Probe {
        events: Vec::new(),
        calls: 0,
        fail_at: -1,
        current: 0,
        kind: EntityKind::Player,
        sponge: false,
    };
    let mut ents = Vec::new();
    for spec in case["entities"].as_array().unwrap() {
        let kind = match spec["kind"].as_str().unwrap() {
            "npc" => EntityKind::Npc,
            "familiar" => EntityKind::Familiar,
            _ => EntityKind::Player,
        };
        probe.kind = kind;
        ents.push(Ent {
            kind,
            spec: Map::new(),
            skills: Skills::new(&mut probe).unwrap(),
            settings: Settings::default(),
            book: 192,
            attrs: Vec::new(),
            hooks: Vec::new(),
            logout: Vec::new(),
            manager: Some(Manager::default()),
        });
    }
    let mut world = World {
        input: case.clone(),
        probe,
        tick: case["tick"].as_i64().unwrap_or(0) as i32,
        rng: JavaRandom::new(case["seed"].as_i64().unwrap_or(0)),
        ents,
        labels: Vec::new(),
        auto: 0,
        npcs: Vec::new(),
        non_skill: 0,
        registry: Registry::default(),
        events_table: table.to_vec(),
    };
    for (i, spec) in case["entities"].as_array().unwrap().iter().enumerate() {
        world.apply(i, spec.as_object().unwrap());
    }
    // ClassScanner.loadTimers: once, before the cases run; its log lines are
    // not part of any operation.
    let mut manager = Manager::default();
    content::register_prototypes(&mut world.natives(0), &mut manager).unwrap();
    world.probe.events.clear();
    world.probe.calls = 0;
    world
}

fn evaluate(case: &Value, table: &[(String, Option<String>, String)]) -> Vec<Value> {
    let mut world = new_world(case, table);
    let mut steps = Vec::new();
    let mut previous: Vec<Option<Value>> = vec![None; world.ents.len()];
    let mut previous_timers = Value::Null;
    for op in case["ops"].as_array().unwrap() {
        world.probe.events.clear();
        world.probe.calls = 0;
        world.probe.fail_at = op["failAt"].as_i64().unwrap_or(-1);
        let (result, error) = match run(&mut world, op) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(error_name(&e))),
        };
        world.probe.fail_at = -1;
        let events = std::mem::take(&mut world.probe.events);
        let mut states = Vec::new();
        for (i, seen) in previous.iter_mut().enumerate() {
            let state = world.entity_state(i);
            states.push(if seen.as_ref() == Some(&state) {
                Value::Null
            } else {
                state.clone()
            });
            *seen = Some(state);
        }
        let labels = world.labels.clone();
        let timers: Map<String, Value> = labels
            .iter()
            .map(|(t, name)| (name.clone(), world.describe_timer(t)))
            .collect();
        let timers = Value::Object(timers);
        let shown = if timers == previous_timers {
            Value::Null
        } else {
            timers.clone()
        };
        previous_timers = timers;
        steps.push(json!({
            "result": result,
            "error": error,
            "events": events,
            "tick": world.tick,
            "entities": states,
            "timers": shown,
        }));
    }
    steps
}

fn table() -> Vec<(String, Option<String>, String)> {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    provenance["tables"]["randomEvents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row[0].as_str().unwrap().to_owned(),
                row[1].as_str().map(str::to_owned),
                row[2].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn engine_timers_match_the_original_classes() {
    let table = table();
    let mut differences: Vec<(String, String)> = Vec::new();
    let mut first = None;
    for (index, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let case = &row["input"];
        let expected = row["expected"].as_array().unwrap();
        let actual = evaluate(case, &table);
        for (step, (a, b)) in actual.iter().zip(expected).enumerate() {
            if a != b {
                let group = case["group"].as_str().unwrap().to_owned();
                if first.is_none() {
                    let keys: Vec<String> = b
                        .as_object()
                        .unwrap()
                        .keys()
                        .filter(|k| a.get(k.as_str()) != b.get(k.as_str()))
                        .map(|k| {
                            format!(
                                "\n {k}: actual {}\n {k}: expected {}",
                                a[k.as_str()],
                                b[k.as_str()]
                            )
                        })
                        .collect();
                    first = Some(format!(
                        "case {index} ({group}) step {step}, op {}{}",
                        case["ops"][step],
                        keys.concat()
                    ));
                }
                differences.push((group, format!("case {index} step {step}")));
                break;
            }
        }
        assert_eq!(actual.len(), expected.len());
    }
    let unexplained: Vec<_> = differences
        .iter()
        .filter(|(group, _)| !GAPS.iter().any(|(g, _)| g == group))
        .collect();
    assert!(
        unexplained.is_empty(),
        "{} differing cases; first: {}",
        unexplained.len(),
        first.unwrap_or_default()
    );
    for (group, reason) in GAPS {
        assert!(
            differences.iter().any(|(g, _)| g == group),
            "gap closed: {group} ({reason})"
        );
    }
}

#[test]
fn fixtures_pin_the_unchanged_classes() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        provenance["fixtureSha256"]
    );
    assert_eq!(
        CORPUS.lines().count() as u64,
        provenance["caseCount"].as_u64().unwrap()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (source, expected) in provenance["sources"].as_object().unwrap() {
        let path = root.join(source);
        if path.is_file() {
            let data: Vec<u8> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{source}"
            );
        }
    }
    // The native tables agree with the original's constants.
    let names: Vec<&str> = PrayerType::ALL.iter().map(|p| p.name()).collect();
    let original: Vec<&str> = provenance["tables"]["prayerTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(names, original);
    for kind in Kind::ALL {
        let timer = kind.instance();
        assert_eq!(Kind::of(&timer.read()), Some(kind));
    }
}

fn designed(group: &str) -> Value {
    CORPUS
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| {
            r["input"]["group"] == group
                && !r["input"]["ops"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|o| o.get("failAt").is_some())
        })
        .unwrap()
}

fn core_data(definitions: &ItemRegistry) -> CoreData {
    CoreData::new(
        &Factory::new(definitions),
        DEFAULT_BANK_SIZE,
        [0, 1, 2, 3].map(listeners::new),
        Some(AttributeLocation::new(3222, 3218, 0)),
    )
    .unwrap()
}

/// The persisted timers join the save file: `CoreData.save` and `parse` (the
/// `core_data.timers` section PlayerSaver writes and PlayerSaveParser reads)
/// run these timers' save and parse members through the same manager. The
/// written section is the original's `saveTimers` text, and loading that
/// section gives the original's timers.
#[test]
fn persisted_timers_join_the_core_data_section() {
    let table = table();
    let definitions = ItemRegistry::default();
    // Writing: replay up to the first save after processing.
    let row = designed("persistence");
    let case = &row["input"];
    let ops = case["ops"].as_array().unwrap();
    let at = ops
        .iter()
        .enumerate()
        .filter(|(_, o)| o["op"] == "save")
        .nth(1)
        .unwrap()
        .0;
    let mut world = new_world(case, &table);
    for op in &ops[..at] {
        run(&mut world, op).unwrap();
    }
    let mut core = core_data(&definitions);
    let mut manager = world.ents[0].manager.take().unwrap();
    let mut root = Json::object();
    core.save(&mut root, &mut manager, &mut world.natives(0))
        .unwrap();
    let section = root.get("core_data").unwrap().get("timers").unwrap().text();
    assert_eq!(section, row["expected"][at]["result"].as_str().unwrap());
    // Loading: that section, through CoreData.parse, into a fresh player.
    let row = designed("persistence-load");
    let case = &row["input"];
    let mut world = new_world(case, &table);
    let loaded = java_json::parse(case["ops"][0]["text"].as_str().unwrap()).unwrap();
    let mut file = Json::object();
    let mut written = Json::object();
    core_data(&definitions)
        .save(&mut written, &mut Manager::default(), &mut world.natives(0))
        .unwrap();
    let mut section = written.get("core_data").unwrap().clone();
    section.put("timers", loaded).unwrap();
    file.put("core_data", section).unwrap();
    let mut core = core_data(&definitions);
    let mut manager = Manager::default();
    core.parse(
        Some(&file),
        &definitions,
        &mut manager,
        &mut world.natives(0),
    )
    .unwrap();
    manager.process(&mut core, &mut world.natives(0)).unwrap();
    let mut saved = Json::object();
    manager
        .save(&mut saved, &mut core, &mut world.natives(0))
        .unwrap();
    assert_eq!(saved.text(), row["expected"][2]["result"].as_str().unwrap());
}
