//! Replays the player tab corpus (fixtures/player-tabs) through
//! `world::player_tabs`. Every service answers from the case the way the
//! oracle's recording adapters do; every recorded call, result, error and the
//! state after each operation is compared.
use rs09_server::player::persisted::{
    emotes::Manager,
    music::{Player as Unlocked, Song, Songs},
    quests::{Quest, Registry, Repository},
};
use rs09_server::player::save::Level;
use rs09_server::world::animation::Priority;
use rs09_server::world::combat_swing::AttackStyle;
use rs09_server::world::interfaces::{self, Type};
use rs09_server::world::player_tabs::{
    Error, Result, Value,
    emotes::{self, Emote},
    level_up,
    music::{self, MusicPlayer, Playback},
    quests,
    weapon::{self, Animation, Lookup, WeaponInterface},
};
use rs09_server::world::spells::SpellBook;
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/player-tabs/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/player-tabs/provenance.json");
/// The component definitions whose type, walkability and tab index are state.
const WATCHED: [i32; 7] = [92, 90, 310, 319, 406, 797, 740];
const TYPE_NAMES: [&str; 9] = [
    "DEFAULT",
    "OVERLAY",
    "TAB",
    "SINGLE_TAB",
    "DIALOGUE",
    "WINDOW_PANE",
    "CS_CHATBOX",
    "CHATBOX",
    "OVERLAY_B",
];
const QUEST_REPOSITORY: &str = "core.game.node.entity.player.link.quest.QuestRepository";

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}

fn type_name(t: Type) -> &'static str {
    TYPE_NAMES[Type::ALL.iter().position(|x| *x == t).unwrap()]
}

fn animation_json(a: &Animation) -> Json {
    json!([a.id, a.delay, a.priority.name()])
}

fn parse_animation(v: &Json) -> Animation {
    Animation {
        id: int(&v[0]),
        delay: int(&v[1]),
        priority: [
            Priority::Low,
            Priority::Mid,
            Priority::High,
            Priority::VeryHigh,
        ]
        .into_iter()
        .find(|p| p.name() == v[2].as_str().unwrap())
        .unwrap(),
    }
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::Int(i) => json!(i),
        Value::Bool(b) => json!(b),
        Value::Component(c) => json!(["component", c.read().id]),
    }
}

fn json_value(v: &Json) -> Value {
    match v {
        Json::Bool(b) => Value::Bool(*b),
        other => Value::Int(int(other)),
    }
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// A registered quest's case values beyond the native `Quest`.
struct QuestRow {
    constant: String,
    configs: Vec<i32>,
    custom: Option<Vec<i32>>,
}

struct World {
    tables: Json,
    spec: Map<String, Json>,
    attributes: BTreeMap<String, Value>,
    style_index: i32,
    special_toggled: bool,
    attack_style: Option<AttackStyle>,
    attack_animation: Option<Animation>,
    autocast: Option<i32>,
    tab0: Option<interfaces::Handle>,
    weapon: Option<WeaponInterface>,
    emotes: Manager,
    unlocked: Unlocked,
    playback: Playback,
    songs: Songs,
    registry: Registry,
    repository: Repository,
    rows: HashMap<i32, QuestRow>,
    combat_milestone: i32,
    skill_milestone: i32,
    definitions: interfaces::Registry,
    events: Vec<Json>,
    calls: i64,
    fail_at: i64,
}

impl World {
    fn emit(&mut self, event: Json) -> Result<()> {
        self.events.push(event);
        self.calls += 1;
        if self.calls == self.fail_at {
            Err(Error::Binding("injected".into()))
        } else {
            Ok(())
        }
    }

    fn flag(&self, key: &str) -> bool {
        self.spec.get(key) == Some(&Json::Bool(true))
    }

    fn table(&self, key: &str) -> Json {
        self.spec
            .get(key)
            .cloned()
            .unwrap_or_else(|| self.tables[key].clone())
    }

    fn item(&self, id: i32) -> Option<Json> {
        let items = self.table("items");
        items.get(id.to_string()).cloned()
    }

    fn keyed(&self, key: &str, slot: i32) -> Option<i32> {
        self.spec
            .get(key)
            .and_then(|m| m.get(slot.to_string()))
            .filter(|v| !v.is_null())
            .map(int)
    }

    fn number(&self, key: &str, default: i32) -> i32 {
        self.spec.get(key).map_or(default, int)
    }

    fn quest_ordinal(&self, constant: &str) -> (i32, String) {
        let table = self.tables["quests"].as_array().unwrap();
        let at = table.iter().position(|r| r[0] == constant).unwrap();
        (at as i32, table[at][1].as_str().unwrap().to_owned())
    }

    fn apply(&mut self, fields: &Map<String, Json>) {
        for (key, v) in fields {
            match key.as_str() {
                "attributes" => {
                    for (k, a) in v.as_object().unwrap() {
                        if a.is_null() {
                            self.attributes.remove(k);
                        } else {
                            self.attributes.insert(k.clone(), json_value(a));
                        }
                    }
                }
                "styleIndex" => self.style_index = int(v),
                "specialToggled" => self.special_toggled = v == &Json::Bool(true),
                "attackStyle" => {
                    self.attack_style = v.as_array().map(|l| AttackStyle {
                        style: int(&l[0]),
                        bonus_type: int(&l[1]),
                    })
                }
                "autocast" => self.autocast = v.as_i64().map(|i| i as i32),
                "combatMilestone" => self.combat_milestone = int(v),
                "skillMilestone" => self.skill_milestone = int(v),
                "emotes" => {
                    self.emotes.emotes = v.as_array().unwrap().iter().map(int).collect();
                }
                "music" => {
                    let m = v.as_object().unwrap();
                    if let Some(list) = m.get("unlocked") {
                        self.unlocked.clear_unlocked();
                        for id in list.as_array().unwrap() {
                            let id = int(id);
                            let index = self.songs.for_id(id).unwrap().index;
                            self.unlocked.unlocked.put(index, id);
                        }
                    }
                    if let Some(c) = m.get("current") {
                        self.playback.current_music_id = int(c);
                    }
                    if let Some(p) = m.get("playing") {
                        self.playback.playing = p == &Json::Bool(true);
                    }
                    if let Some(l) = m.get("looping") {
                        self.playback.looping = l == &Json::Bool(true);
                    }
                }
                "questStages" => {
                    for (k, s) in v.as_object().unwrap() {
                        self.repository.stages.put(k.parse().unwrap(), int(s));
                    }
                }
                "questPoints" => {
                    let delta = int(v).wrapping_sub(self.repository.points);
                    self.repository.increment_points(delta);
                }
                _ => {
                    self.spec.insert(key.clone(), v.clone());
                }
            }
        }
    }

    fn snapshot(&self) -> Json {
        let attributes: Map<String, Json> = self
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), value_json(v)))
            .collect();
        let weapon = self.weapon.as_ref().map_or(Json::Null, |w| {
            json!([
                w.current.map(|c| c.name()),
                w.special_bar,
                w.attack_animations
                    .as_ref()
                    .map(|a| a.iter().map(animation_json).collect::<Vec<_>>()),
                w.id()
            ])
        });
        let unlocked: Vec<Json> = self
            .unlocked
            .unlocked
            .iter()
            .map(|(index, id)| json!([index, id]))
            .collect();
        let stages: Vec<Json> = self
            .repository
            .stages
            .iter()
            .map(|(index, stage)| json!([index, stage]))
            .collect();
        let mut definitions = Map::new();
        for id in WATCHED {
            if let Some(d) = self.definitions.definitions.get(&id) {
                let d = d.read();
                definitions.insert(
                    id.to_string(),
                    json!([d.kind.map(type_name), d.walkable, d.tab_index]),
                );
            }
        }
        json!({
            "attributes": attributes,
            "styleIndex": self.style_index,
            "specialToggled": self.special_toggled,
            "attackStyle": self.attack_style.map(|s| json!([s.style, s.bonus_type])),
            "attackAnimation": self.attack_animation.as_ref().map(animation_json),
            "autocast": self.autocast,
            "tab0": self.tab0.as_ref().map(|t| t.read().id),
            "weapon": weapon,
            "emotes": self.emotes.emotes,
            "music": [unlocked, self.playback.current_music_id, self.playback.playing, self.playback.looping],
            "quests": [self.repository.points, stages],
            "milestones": [self.combat_milestone, self.skill_milestone],
            "definitions": definitions,
        })
    }

    fn get_attribute_value(&mut self, key: &str, default: Value) -> Result<Value> {
        self.emit(json!(["get_attribute", key, value_json(&default)]))?;
        Ok(self
            .attributes
            .get(&key.replace("/save:", ""))
            .cloned()
            .unwrap_or(default))
    }

    fn set_attribute_value(&mut self, key: &str, value: Value) -> Result<()> {
        self.emit(json!(["set_attribute", key, value_json(&value)]))?;
        let stored = key.strip_prefix("/save:").unwrap_or(key).to_owned();
        self.attributes.insert(stored, value);
        Ok(())
    }

    fn with_emotes<T>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut Manager) -> Result<T>,
    ) -> Result<T> {
        let mut manager = std::mem::take(&mut self.emotes);
        let result = f(self, &mut manager);
        self.emotes = manager;
        result
    }

    fn with_music<T>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut MusicPlayer<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut unlocked = std::mem::take(&mut self.unlocked);
        let mut playback = std::mem::take(&mut self.playback);
        let songs = std::mem::take(&mut self.songs);
        let result = {
            let mut player = MusicPlayer {
                unlocked: &mut unlocked,
                playback: &mut playback,
                songs: &songs,
            };
            f(self, &mut player)
        };
        self.unlocked = unlocked;
        self.playback = playback;
        self.songs = songs;
        result
    }

    fn with_weapon<T>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut WeaponInterface) -> Result<T>,
    ) -> Result<T> {
        let mut weapon = self.weapon.take().expect("weapon_new first");
        let result = f(self, &mut weapon);
        self.weapon = Some(weapon);
        result
    }
}

// ------------------------------------------------------------------ hosts

impl weapon::Host for World {
    type Spell = i32;
    fn add_extension(&mut self) -> Result<()> {
        self.emit(json!(["add_extension", "WeaponInterface"]))
    }
    fn set_tab(&mut self, slot: usize, component: &interfaces::Handle) -> Result<()> {
        self.emit(json!(["tabs"]))?;
        assert_eq!(slot, 0);
        self.tab0 = Some(component.clone());
        Ok(())
    }
    fn equipment(&mut self, slot: i32) -> Result<Option<i32>> {
        self.emit(json!(["equipment", slot]))?;
        Ok(self.keyed("worn", slot))
    }
    fn equipment_new(&mut self, slot: i32) -> Result<i32> {
        self.emit(json!(["equipment_new", slot]))?;
        Ok(self.keyed("worn", slot).unwrap_or(0))
    }
    fn item_name(&mut self, id: i32) -> Option<String> {
        // The adapter's definition name: the text "null" for an item the case does not name.
        Some(
            self.item(id)
                .and_then(|i| i["name"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "null".into()),
        )
    }
    fn item_config_int(&mut self, id: i32, key: &str, default: i32) -> Result<i32> {
        self.emit(json!(["item_config", id, key]))?;
        Ok(self
            .item(id)
            .and_then(|i| i.get(key).cloned())
            .map_or(default, |v| int(&v)))
    }
    fn item_config_bool(&mut self, id: i32, key: &str, default: bool) -> Result<bool> {
        self.emit(json!(["item_config", id, key]))?;
        Ok(self
            .item(id)
            .and_then(|i| i.get(key).cloned())
            .map_or(default, |v| v == Json::Bool(true)))
    }
    fn item_config_animations(&mut self, id: i32, key: &str) -> Result<Option<Vec<Animation>>> {
        self.emit(json!(["item_config", id, key]))?;
        Ok(self
            .item(id)
            .and_then(|i| i.get(key).cloned())
            .map(|v| v.as_array().unwrap().iter().map(parse_animation).collect()))
    }
    fn component_definition(&mut self, id: i32) -> interfaces::Def {
        self.definitions.for_id(id)
    }
    fn create_component(&mut self, id: i32) -> interfaces::Handle {
        self.definitions.component(id)
    }
    fn resizable(&mut self) -> Result<bool> {
        self.emit(json!(["resizable"]))?;
        Ok(self.flag("resizable"))
    }
    fn send_interface(&mut self, pane: i32, child: i32, id: i32, walkable: bool) -> Result<()> {
        self.emit(json!(["interface", pane, child, id, walkable]))
    }
    fn interface_config(&mut self, interface: i32, child: i32, hide: bool) -> Result<()> {
        self.emit(json!(["interface_config", interface, child, hide]))
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> Result<()> {
        self.emit(json!(["string", text, interface, child]))
    }
    fn message(&mut self, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn attack_style_index(&mut self) -> Result<i32> {
        self.emit(json!(["style_index"]))?;
        Ok(self.style_index)
    }
    fn toggle_attack_style_index(&mut self, index: i32) -> Result<()> {
        self.emit(json!(["toggle_style_index", index]))?;
        self.style_index = index;
        Ok(())
    }
    fn special_toggled(&mut self) -> Result<bool> {
        self.emit(json!(["special_toggled"]))?;
        Ok(self.special_toggled)
    }
    fn toggle_special_bar(&mut self) -> Result<()> {
        self.emit(json!(["toggle_special_bar"]))?;
        self.special_toggled = !self.special_toggled;
        Ok(())
    }
    fn attack_style(&mut self) -> Result<Option<AttackStyle>> {
        self.emit(json!(["attack_style"]))?;
        Ok(self.attack_style)
    }
    fn set_attack_style(&mut self, style: AttackStyle) -> Result<()> {
        self.emit(json!(["set_attack_style", [style.style, style.bonus_type]]))?;
        self.attack_style = Some(style);
        Ok(())
    }
    fn set_attack_animation(&mut self, animation: Animation) -> Result<()> {
        self.emit(json!(["set_attack_animation", animation_json(&animation)]))?;
        self.attack_animation = Some(animation);
        Ok(())
    }
    fn update_style(&mut self) -> Result<()> {
        self.emit(json!(["update_style"]))
    }
    fn is_npc(&mut self) -> Result<bool> {
        self.emit(json!(["is_npc"]))?;
        Ok(self.flag("npc"))
    }
    fn spell_book(&mut self) -> Result<i32> {
        self.emit(json!(["spell_book"]))?;
        Ok(self.number("book", 192))
    }
    fn autocast(&mut self) -> Result<Option<i32>> {
        self.emit(json!(["autocast"]))?;
        Ok(self.autocast)
    }
    fn spell_id(&mut self, spell: &i32) -> i32 {
        *spell
    }
    fn set_autocast(&mut self, spell: Option<&i32>) -> Result<()> {
        self.emit(json!(["set_autocast", spell]))?;
        self.autocast = spell.copied();
        Ok(())
    }
    fn spell_lookup(&mut self, book: SpellBook, id: i32) -> Result<Lookup<i32>> {
        self.emit(json!(["spell_lookup", book.name(), id]))?;
        let registered = self.table("registered");
        let has = |key: &str| {
            registered
                .get(key)
                .and_then(Json::as_array)
                .is_some_and(|l| l.contains(&json!(id)))
        };
        Ok(if has(book.name()) {
            Lookup::Combat(id)
        } else if has(&format!("{}:plain", book.name())) {
            Lookup::Other
        } else {
            Lookup::None
        })
    }
    fn get_attribute(&mut self, key: &str, default: Value) -> Result<Value> {
        self.get_attribute_value(key, default)
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> Result<()> {
        self.set_attribute_value(key, value)
    }
    fn remove_attribute(&mut self, key: &str) -> Result<()> {
        self.emit(json!(["remove_attribute", key]))?;
        self.attributes.remove(key);
        Ok(())
    }
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varp", id, value, save]))
    }
    fn open_tab(&mut self, component: &interfaces::Handle) -> Result<()> {
        let id = component.read().id;
        self.emit(json!(["open_tab", id]))
    }
}

impl level_up::Host for World {
    fn get_attribute(&mut self, key: &str, default: Value) -> Result<Value> {
        self.get_attribute_value(key, default)
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> Result<()> {
        self.set_attribute_value(key, value)
    }
    fn static_level(&mut self, slot: i32) -> Result<i32> {
        self.emit(json!(["static_level", slot]))?;
        Ok(self.keyed("levels", slot).unwrap_or(1))
    }
    fn increment_prayer_points(&mut self, amount: f64) -> Result<()> {
        self.emit(json!(["prayer_points", hex(amount)]))
    }
    fn total_level(&mut self) -> Result<i32> {
        self.emit(json!(["total_level"]))?;
        Ok(self.number("totalLevel", 32))
    }
    fn combat_milestone(&mut self) -> Result<i32> {
        self.emit(json!(["combat_milestone"]))?;
        Ok(self.combat_milestone)
    }
    fn set_combat_milestone(&mut self, value: i32) -> Result<()> {
        self.emit(json!(["set_combat_milestone", value]))?;
        self.combat_milestone = value;
        Ok(())
    }
    fn skill_milestone(&mut self) -> Result<i32> {
        self.emit(json!(["skill_milestone"]))?;
        Ok(self.skill_milestone)
    }
    fn set_skill_milestone(&mut self, value: i32) -> Result<()> {
        self.emit(json!(["set_skill_milestone", value]))?;
        self.skill_milestone = value;
        Ok(())
    }
    fn combat_level(&mut self) -> Result<i32> {
        self.emit(json!(["combat_level"]))?;
        Ok(self.number("combatLevel", 3))
    }
    fn jingle(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["music", id, true]))
    }
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varp", id, value, save]))
    }
    fn graphic(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["graphic", id]))
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> Result<()> {
        self.emit(json!(["string", text, interface, child]))
    }
    fn message(&mut self, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn is_artificial(&mut self) -> bool {
        self.flag("artificial")
    }
    fn username(&mut self) -> String {
        self.spec
            .get("username")
            .and_then(Json::as_str)
            .unwrap_or("tracer")
            .to_owned()
    }
    fn news(&mut self, text: &str, style: Option<(i32, &str)>) -> Result<()> {
        match style {
            None => self.emit(json!(["news", text, null, null])),
            Some((icon, color)) => self.emit(json!(["news", text, icon, color])),
        }
    }
    fn skillcape_trim(&mut self) -> Result<()> {
        self.emit(json!(["skillcape_trim"]))
    }
    fn emote_unlock(&mut self, emote: Emote) -> Result<bool> {
        self.with_emotes(|w, m| emotes::unlock(w, m, emote))
    }
    fn open_chatbox(&mut self, id: i32) -> Result<()> {
        let component = self.definitions.component(id);
        let id = component.read().id;
        self.emit(json!(["open_chatbox", id]))
    }
}

impl emotes::Host for World {
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varp", id, value, save]))
    }
    fn is_locked(&mut self, key: &str) -> Result<bool> {
        self.emit(json!(["is_locked", key]))?;
        Ok(self.flag("emoteLocked"))
    }
    fn lock(&mut self, key: &str, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", key, ticks]))
    }
    fn lock_all(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["lock_all", ticks]))
    }
    fn is_attacking(&mut self) -> Result<bool> {
        self.emit(json!(["is_attacking"]))?;
        Ok(self.flag("attacking"))
    }
    fn in_combat(&mut self) -> Result<bool> {
        self.emit(json!(["in_combat"]))?;
        Ok(self.flag("inCombat"))
    }
    fn debug(&mut self, text: &str) -> Result<()> {
        self.emit(json!(["debug", text]))
    }
    fn rights(&mut self) -> Result<i32> {
        self.emit(json!(["rights"]))?;
        let name = self
            .spec
            .get("rights")
            .and_then(Json::as_str)
            .unwrap_or("REGULAR_PLAYER");
        Ok(["REGULAR_PLAYER", "PLAYER_MODERATOR", "ADMINISTRATOR"]
            .iter()
            .position(|r| *r == name)
            .unwrap() as i32)
    }
    fn pulse_clear(&mut self) -> Result<()> {
        self.emit(json!(["pulse_clear"]))
    }
    fn dialogue(&mut self, messages: &[&str]) -> Result<()> {
        self.emit(json!(["dialogue", messages]))
    }
    fn varrock_diary_complete(&mut self, level: i32, index: i32) -> Result<bool> {
        self.emit(json!(["diary_complete", "VARROCK", level, index]))?;
        Ok(self.flag("diaryComplete"))
    }
    fn varrock_diary_update(&mut self, level: i32, index: i32, complete: bool) -> Result<()> {
        self.emit(json!(["diary_update", "VARROCK", level, index, complete]))
    }
    fn get_attribute(&mut self, key: &str, default: Value) -> Result<Value> {
        self.get_attribute_value(key, default)
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> Result<()> {
        self.set_attribute_value(key, value)
    }
    fn equipment(&mut self, slot: i32) -> Result<Option<i32>> {
        self.emit(json!(["equipment", slot]))?;
        Ok(self.keyed("worn", slot))
    }
    fn animate(&mut self, animation: i32, graphics: Option<i32>) -> Result<()> {
        self.emit(json!([
            "animate",
            [animation, 0, "MID"],
            graphics.map(|g| json!([g, 0, 0]))
        ]))
    }
    fn animation_duration(&mut self, animation: i32) -> Result<i32> {
        self.emit(json!(["duration", animation]))?;
        Ok(self.keyed("durations", animation).unwrap_or(1))
    }
    fn graphic(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["graphic", id]))
    }
    fn animation(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animation", id]))
    }
    fn message(&mut self, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn jingle(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["music", id, true]))
    }
    fn content_play(&mut self, emote: Emote) -> Result<()> {
        panic!("the corpus does not drive the content override of {emote:?}")
    }
}

impl music::Host for World {
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varp", id, value, save]))
    }
    fn access_mask(
        &mut self,
        hash: i32,
        child: i32,
        interface: i32,
        offset: i32,
        length: i32,
    ) -> Result<()> {
        self.emit(json!([
            "access_mask",
            hash,
            child,
            interface,
            offset,
            length
        ]))
    }
    fn message(&mut self, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn music(&mut self, id: i32, secondary: bool) -> Result<()> {
        self.emit(json!(["music", id, secondary]))
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> Result<()> {
        self.emit(json!(["string", text, interface, child]))
    }
    fn is_debug(&mut self) -> Result<bool> {
        self.emit(json!(["is_debug"]))?;
        Ok(self.flag("debug"))
    }
    fn ticks(&mut self) -> Result<i32> {
        self.emit(json!(["ticks"]))?;
        Ok(self.number("ticks", 0))
    }
    fn emote_unlocked(&mut self, emote: Emote) -> Result<bool> {
        Ok(self.emotes.is_unlocked(emote.ordinal()))
    }
    fn emote_unlock(&mut self, emote: Emote) -> Result<bool> {
        self.with_emotes(|w, m| emotes::unlock(w, m, emote))
    }
    fn emote_lock(&mut self, emote: Emote) -> Result<bool> {
        self.with_emotes(|w, m| emotes::lock(w, m, emote))
    }
}

impl quests::Host for World {
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varp", id, value, save]))
    }
    fn varbit(&mut self, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["varbit", id, value, save]))
    }
    fn quest_config(&mut self, quest: &Quest, stage: i32) -> Result<Vec<i32>> {
        let ordinal = self
            .rows
            .iter()
            .find(|(_, r)| r.constant == self.quest_name_of(quest))
            .map(|(o, _)| *o)
            .unwrap();
        let row = &self.rows[&ordinal];
        match &row.custom {
            Some(config) => {
                let config = config.clone();
                let constant = row.constant.clone();
                self.emit(json!(["custom_config", constant, stage]))?;
                Ok(config)
            }
            None => quests::base_config(&row.configs, stage),
        }
    }
    fn update_varps(&mut self, quest: &Quest) -> Result<()> {
        let constant = self.quest_name_of(quest);
        self.emit(json!(["update_varps", constant]))
    }
}

impl World {
    /// The `Quests` constant of a registered quest, by its display name.
    fn quest_name_of(&self, quest: &Quest) -> String {
        let table = self.tables["quests"].as_array().unwrap();
        table
            .iter()
            .find(|r| r[1] == quest.name.as_str())
            .map(|r| r[0].as_str().unwrap().to_owned())
            .unwrap()
    }
}

// ------------------------------------------------------------------ replay

fn world(tables: &Json, input: &Json) -> World {
    let mut definitions = interfaces::Registry::default();
    if let Some(defs) = input["definitions"].as_object() {
        for (id, l) in defs {
            let d = definitions.for_id(id.parse().unwrap());
            let mut d = d.write();
            d.kind = l[0]
                .as_str()
                .map(|n| Type::ALL[TYPE_NAMES.iter().position(|t| *t == n).unwrap()]);
            d.walkable = l[1] == Json::Bool(true);
            d.tab_index = int(&l[2]);
        }
    }
    let mut songs = Songs::default();
    for row in input["songs"].as_array().into_iter().flatten() {
        songs.songs.insert(
            int(&row[0]),
            Song {
                name: row[1].as_str().unwrap().to_owned(),
                index: int(&row[2]),
            },
        );
    }
    let mut registry = Registry::default();
    let mut late = Vec::new();
    let mut rows = HashMap::new();
    for row in input["quests"].as_array().into_iter().flatten() {
        let ordinal = int(&row[7]);
        let quest = Quest {
            name: row[8].as_str().unwrap().to_owned(),
            index: int(&row[1]),
            button: int(&row[2]),
            points: int(&row[3]),
        };
        rows.insert(
            ordinal,
            QuestRow {
                constant: row[0].as_str().unwrap().to_owned(),
                configs: row[4].as_array().unwrap().iter().map(int).collect(),
                custom: if row[5] == Json::Bool(true) {
                    Some(row[6].as_array().unwrap().iter().map(int).collect())
                } else {
                    None
                },
            },
        );
        if row[9] == Json::Bool(true) {
            late.push((ordinal, quest));
        } else {
            registry.register(ordinal, quest);
        }
    }
    let repository = Repository::new(&registry);
    for (ordinal, quest) in late {
        registry.register(ordinal, quest);
    }
    let mut w = World {
        tables: tables.clone(),
        spec: Map::new(),
        attributes: BTreeMap::new(),
        style_index: 0,
        special_toggled: false,
        attack_style: None,
        attack_animation: None,
        autocast: None,
        tab0: None,
        weapon: None,
        // `new EmoteManager(player)`: the first 22 constants.
        emotes: Manager::default(),
        unlocked: Unlocked::default(),
        playback: Playback::default(),
        songs,
        registry,
        repository,
        rows,
        combat_milestone: 0,
        skill_milestone: 0,
        definitions,
        events: Vec::new(),
        calls: 0,
        fail_at: -1,
    };
    w.apply(input["player"].as_object().unwrap());
    w
}

fn error_text(e: &Error) -> String {
    match e {
        Error::Null => "Null".into(),
        Error::Index => "Index".into(),
        Error::Cast => "Cast".into(),
        Error::Binding(_) => "Binding".into(),
        Error::IllegalState(m) => format!("IllegalState: {m}"),
    }
}

fn emote(v: &Json) -> Option<Emote> {
    v.as_str().map(|name| {
        let at = EMOTE_NAMES.iter().position(|n| *n == name).unwrap();
        Emote::ALL[at]
    })
}

const EMOTE_NAMES: [&str; 45] = [
    "YES",
    "NO",
    "BOW",
    "ANGRY",
    "THINK",
    "WAVE",
    "SHRUG",
    "CHEER",
    "BECKON",
    "JUMP_FOR_JOY",
    "LAUGH",
    "YAWN",
    "DANCE",
    "JIG",
    "SPIN",
    "HEADBANG",
    "CRY",
    "BLOW_KISS",
    "PANIC",
    "RASPBERRY",
    "CLAP",
    "SALUTE",
    "GOBLIN_BOW",
    "GOBLIN_SALUTE",
    "GLASS_BOX",
    "CLIMB_ROPE",
    "LEAN",
    "GLASS_WALL",
    "IDEA",
    "STOMP",
    "FLAP",
    "SLAP_HEAD",
    "ZOMBIE_WALK",
    "ZOMBIE_DANCE",
    "ZOMBIE_HAND",
    "SCARED",
    "BUNNY_HOP",
    "SKILLCAPE",
    "SNOWMAN_DANCE",
    "AIR_GUITAR",
    "SAFETY_FIRST",
    "EXPLORE",
    "TRICK",
    "FREEZE",
    "GIVE_THANKS",
];

fn flag(op: &Json, key: &str) -> bool {
    op[key] == Json::Bool(true)
}

/// The quest a case names: its registered `Quest`, if any.
fn registered<'a>(w: &'a World, constant: &str) -> Option<&'a Quest> {
    let (ordinal, _) = w.quest_ordinal(constant);
    w.registry.get(ordinal)
}

fn run(w: &mut World, op: &Json) -> Result<Json> {
    let name = op["op"].as_str().unwrap();
    Ok(match name {
        "set" => {
            w.apply(op["fields"].as_object().unwrap());
            Json::Null
        }
        "weapon_new" => {
            let component = w.definitions.component(weapon::WEAPON_FISTS_SEL);
            let created = WeaponInterface::new(w, component)?;
            w.weapon = Some(created);
            Json::Null
        }
        "weapon_open" => w.with_weapon(|w, x| x.open(w)).map(|_| Json::Null)?,
        "weapon_update" => w
            .with_weapon(|w, x| x.update_interface(w))
            .map(|_| Json::Null)?,
        "weapon_set_style" => {
            json!(w.with_weapon(|w, x| x.set_attack_style(w, int(&op["button"])))?)
        }
        "weapon_select" => {
            w.with_weapon(|w, x| {
                x.select_auto_spell(
                    w,
                    int(&op["button"]),
                    flag(op, "adjust"),
                    flag(op, "forget"),
                )
            })?;
            Json::Null
        }
        "weapon_autospell_id" => {
            json!(w.with_weapon(|w, x| x.autospell_id(w, int(&op["spellId"])))?)
        }
        "weapon_can_autocast" => {
            json!(w.with_weapon(|w, x| x.can_autocast(w, flag(op, "message")))?)
        }
        "weapon_open_select" => w
            .with_weapon(|w, x| x.open_autocast_select(w))
            .map(|_| Json::Null)?,
        "weapon_restore_autocast" => json!(w.with_weapon(|w, x| x.restore_autocast(w))?),
        "weapon_restore_style" => {
            w.with_weapon(|w, x| x.restore_attack_style(w, int(&op["index"])))?;
            Json::Null
        }
        "weapon_interface_for" => {
            let item = op["item"].as_i64().map(|i| i as i32);
            json!(weapon::weapon_interface_for(w, item)?.name())
        }
        "levelup" => {
            level_up::levelup(w, int(&op["slot"]), int(&op["amount"]))?;
            Json::Null
        }
        "milestones" => {
            level_up::handle_milestones(w, int(&op["slot"]), int(&op["amount"]))?;
            Json::Null
        }
        "flashing" => {
            level_up::send_flashing_icons(w, int(&op["slot"]))?;
            Json::Null
        }
        "jingle" => json!(level_up::skill_jingle(w, int(&op["slot"]))?),
        "emote_refresh" => {
            w.with_emotes(emotes::refresh)?;
            Json::Null
        }
        "emote_unlock" => {
            let e = emote(&op["emote"]).ok_or(Error::Null)?;
            json!(w.with_emotes(|w, m| emotes::unlock(w, m, e))?)
        }
        "emote_lock" => {
            let e = emote(&op["emote"]).ok_or(Error::Null)?;
            json!(w.with_emotes(|w, m| emotes::lock(w, m, e))?)
        }
        "emote_is_unlocked" => {
            // isUnlocked(null): contains(null) on a list without null.
            json!(emote(&op["emote"]).is_some_and(|e| w.emotes.is_unlocked(e.ordinal())))
        }
        "emote_save_required" => json!(w.emotes.is_save_required()),
        "emote_handle" => {
            let button = int(&op["button"]);
            w.with_emotes(|w, m| emotes::handle(w, m, button))?;
            Json::Null
        }
        "emote_play" => {
            emotes::play(w, emote(&op["emote"]).unwrap())?;
            Json::Null
        }
        "emote_for_id" => {
            json!(Emote::for_id(int(&op["button"])).map(|e| EMOTE_NAMES[e.ordinal() as usize]))
        }
        "music_init" => w.with_music(|w, m| m.init(w)).map(|_| Json::Null)?,
        "music_unlock" => {
            let id = int(&op["id"]);
            let play = op["play"].as_bool().unwrap_or(true);
            w.with_music(|w, m| m.unlock(w, id, play))?;
            Json::Null
        }
        "music_play" => {
            let id = int(&op["id"]);
            w.with_music(|w, m| m.play(w, id))?;
            Json::Null
        }
        "music_play_default" => w.with_music(|w, m| m.play_default(w)).map(|_| Json::Null)?,
        "music_replay" => w.with_music(|w, m| m.replay(w)).map(|_| Json::Null)?,
        "music_tick" => w.with_music(|w, m| m.tick(w)).map(|_| Json::Null)?,
        "music_toggle_looping" => w
            .with_music(|w, m| m.toggle_looping(w))
            .map(|_| Json::Null)?,
        "music_set_looping" => {
            let looping = flag(op, "looping");
            w.with_music(|w, m| m.set_looping(w, looping))?;
            Json::Null
        }
        "music_refresh" => w.with_music(|w, m| m.refresh_list(w)).map(|_| Json::Null)?,
        "music_has_unlocked" => {
            let id = int(&op["id"]);
            json!(w.with_music(|_, m| Ok(m.has_unlocked(id)))?)
        }
        "music_has_air_guitar" => json!(w.with_music(|_, m| Ok(m.has_air_guitar()))?),
        "music_zone_enter" => {
            let id = int(&op["id"]);
            if op["entity"] == "npc" {
                json!(music::zone_enter(None, w, id)?)
            } else {
                json!(w.with_music(|w, m| music::zone_enter(Some(m), w, id))?)
            }
        }
        "quest_sync" => {
            let repository = w.repository.clone();
            let registry = w.registry.clone();
            quests::syncronize_tab(w, &repository, &registry)?;
            Json::Null
        }
        "quest_set_stage" | "quest_set_stage_nonmonotonic" => {
            let quest = registered(w, op["quest"].as_str().unwrap())
                .cloned()
                .ok_or(Error::Null)?;
            let stage = int(&op["stage"]);
            if name == "quest_set_stage" {
                let mut logs = Vec::new();
                let result = w
                    .repository
                    .set_stage(&quest, stage, "tracer", &mut |level, text| {
                        logs.push((level, text.to_owned()))
                    });
                for (level, text) in logs {
                    emit_log(w, level, &text)?;
                }
                result?;
            } else {
                w.repository.set_stage_nonmonotonic(&quest, stage);
            }
            Json::Null
        }
        "quest_increment" => {
            w.repository.increment_points(int(&op["value"]));
            Json::Null
        }
        "quest_dock" => {
            w.repository.dock_points(int(&op["value"]));
            Json::Null
        }
        "quest_sync_points" => {
            let registry = w.registry.clone();
            w.repository.sync_points(&registry)?;
            Json::Null
        }
        "quest_available" => json!(w.registry.available_points()),
        "quest_for_button" => {
            json!(quests::for_button(&w.registry, int(&op["button"])).map(|q| w.quest_name_of(q)))
        }
        "quest_for_index" => {
            json!(quests::for_index(&w.registry, int(&op["index"])).map(|q| w.quest_name_of(q)))
        }
        "quest_complete_all" => json!(w.repository.has_completed_all(&w.registry)),
        "quest_is_complete" => {
            let (ordinal, display) = w.quest_ordinal(op["quest"].as_str().unwrap());
            let mut logs = Vec::new();
            let result = w
                .repository
                .is_complete(&w.registry, ordinal, &display, &mut |l, t| {
                    logs.push((l, t.to_owned()))
                });
            for (level, text) in logs {
                emit_log(w, level, &text)?;
            }
            json!(result?)
        }
        "quest_has_started" => {
            let (ordinal, _) = w.quest_ordinal(op["quest"].as_str().unwrap());
            json!(w.repository.has_started(&w.registry, ordinal)?)
        }
        "quest_stage" => {
            let (ordinal, _) = w.quest_ordinal(op["quest"].as_str().unwrap());
            json!(w.repository.stage_of(&w.registry, ordinal)?)
        }
        "quest_config" => {
            let quest = registered(w, op["quest"].as_str().unwrap())
                .cloned()
                .ok_or(Error::Null)?;
            json!(quests::Host::quest_config(w, &quest, int(&op["stage"]))?)
        }
        other => panic!("unknown op {other}"),
    })
}

/// `log(QuestRepository.class, level, text)`: the persisted repository logs
/// through a callback that cannot fail; the record is emitted after it returns.
fn emit_log(w: &mut World, level: Level, text: &str) -> Result<()> {
    let level = match level {
        Level::Warn => "WARN",
        Level::Err => "ERR",
    };
    w.emit(json!(["log", QUEST_REPOSITORY, level, text]))
}

#[test]
fn provenance_matches_the_corpus() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(provenance["caseCount"], 5041);
}

#[test]
fn native_tabs_replay_the_original_corpus() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let tables = provenance["tables"].clone();
    let mut mismatches = Vec::new();
    let mut steps = 0usize;
    for (number, line) in CORPUS.lines().enumerate() {
        let row: Json = serde_json::from_str(line).unwrap();
        let input = &row["input"];
        let mut w = world(&tables, input);
        if w.snapshot() != row["initial"] {
            mismatches.push(format!("case {number}: initial state {}", w.snapshot()));
            continue;
        }
        let mut previous = w.snapshot();
        for (k, (op, expected)) in input["ops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
            .enumerate()
        {
            w.events.clear();
            w.calls = 0;
            w.fail_at = op["failAt"].as_i64().unwrap_or(-1);
            let outcome = run(&mut w, op);
            let (result, error) = match outcome {
                Ok(v) => (v, Json::Null),
                Err(e) => (Json::Null, json!(error_text(&e))),
            };
            let state = w.snapshot();
            let state_json = if state == previous {
                Json::Null
            } else {
                state.clone()
            };
            previous = state;
            let mine =
                json!({"result": result, "error": error, "events": w.events, "state": state_json});
            if &mine != expected {
                mismatches.push(format!(
                    "case {number} ({}) op {k} {op}:\n  native   {mine}\n  original {expected}",
                    input["group"]
                ));
                break;
            }
            steps += 1;
        }
        if mismatches.len() > 5 {
            break;
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert_eq!(
        steps,
        provenance["coverage"]["actions"].as_u64().unwrap() as usize
    );
}
