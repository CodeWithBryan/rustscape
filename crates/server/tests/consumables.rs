//! Replays the consumable corpus (fixtures/consumables) through
//! `world::consumables`: the table, the listener with its clocks, every
//! class's consume, the effects, health values, names and doses. The host
//! answers every service from the case the way the oracle's recording
//! adapters do; skills and run energy are the native `Skills` and `Settings`.
//! Every operation's result, error, recorded calls and draws, and the
//! player's state are compared whole.
use rs09_game_core::random::JavaRandom;
use rs09_server::player::{
    settings::{self, Settings},
    skills::{self, Skills},
};
use rs09_server::world::combat_pulse::HitsplatType;
use rs09_server::world::consumables::{
    self, Attr, ContainerKind, Diary, DoseScript, Effect, Error, Host, Item, Kind, Log, Node,
    QueueStrength, Result, RumPulse, Table, TimerArg,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/consumables/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/consumables/provenance.json");

/// Differences between the original and the port, by case group and the
/// reason. Empty: every case matches. A closed gap or a new difference fails.
const GAPS: &[(&str, &str)] = &[];

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

/// The recorder and the context the native skills and settings call back into.
struct Probe {
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
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
        skills::EntityKind::Player
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
        if !self.ok(json!(["perk", "DAMAGE_SPONG"])) {
            return Err(skills::Error::Binding);
        }
        Ok(self.sponge)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        // getExtension(AssistSession.class): never present in the corpus.
        if !self.ok(json!(["extension", "AssistSession"])) {
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
                if self.ok(json!(["skill_packet", slot])) {
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
        // player.getPacketDispatch(): a getter.
        Ok(Some(0))
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        if self.ok(json!(["run_energy_packet"])) {
            Ok(())
        } else {
            Err(settings::Error::Binding)
        }
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

/// A timer the case's `spawnTimer` answers: label, identifier, arguments and
/// (for a `Disease`) its hits left.
struct TimerModel {
    label: String,
    identifier: String,
    args: Vec<Value>,
    hits_left: Option<i32>,
}

struct Queued {
    script: DoseScript,
    strength: QueueStrength,
    next: i32,
    persist: bool,
}

struct Replay<'a> {
    case: &'a Value,
    probe: Probe,
    skills: Skills,
    settings: Settings,
    clocks: [i32; 10],
    tick: i32,
    rng: JavaRandom,
    attrs: Map<String, Value>,
    inventory: Vec<Option<Item>>,
    equipment: Vec<Option<Item>>,
    timers: Vec<TimerModel>,
    timers_pending: Option<TimerModel>,
    labels: usize,
    pulses: Vec<RumPulse>,
    queued: Vec<Queued>,
    familiar: bool,
    active: Option<bool>,
}

fn attr_value(a: &Attr) -> Value {
    match a {
        Attr::Null => Value::Null,
        Attr::Int(v) => json!(v),
        Attr::Bool(v) => json!(v),
        Attr::Str(v) => json!(v),
        Attr::Other(name) => json!(name),
    }
}

fn attr_of(v: &Value) -> Attr {
    match v {
        Value::Null => Attr::Null,
        Value::Bool(b) => Attr::Bool(*b),
        Value::Number(n) => Attr::Int(n.as_i64().unwrap() as i32),
        Value::String(s) => Attr::Str(s.clone()),
        other => panic!("attribute {other}"),
    }
}

fn describe(item: Option<&Item>) -> Value {
    match item {
        None => Value::Null,
        Some(i) => json!([i.id(), i.amount, i.slot]),
    }
}

impl<'a> Replay<'a> {
    fn new(case: &'a Value) -> Self {
        let mut probe = Probe {
            events: Vec::new(),
            calls: 0,
            fail_at: -1,
            sponge: false,
        };
        let mut skills = Skills::new(&mut probe).unwrap();
        for i in 0..24 {
            skills.static_levels[i] = if i == 3 { 10 } else { 1 };
            skills.dynamic_levels[i] = skills.static_levels[i];
        }
        let mut replay = Self {
            case,
            probe,
            skills,
            settings: Settings::default(),
            clocks: [0; 10],
            tick: case["tick"].as_i64().unwrap_or(0) as i32,
            rng: JavaRandom::new(case["seed"].as_i64().unwrap_or(0)),
            attrs: Map::new(),
            inventory: vec![None; 28],
            equipment: vec![None; 14],
            timers: Vec::new(),
            timers_pending: None,
            labels: 0,
            pulses: Vec::new(),
            queued: Vec::new(),
            familiar: false,
            active: None,
        };
        // Skills.prayerPoints and Settings.runEnergy start at their field initialisers.
        replay.skills.prayer_points = 1.0;
        replay.settings.fields.write().run_energy = 100.0;
        replay.apply(&case["player"]);
        replay
    }

    fn label(&mut self) -> String {
        let label = format!("t{}", self.labels);
        self.labels += 1;
        label
    }

    fn apply(&mut self, fields: &Value) {
        for (k, v) in fields.as_object().unwrap() {
            match k.as_str() {
                "static" => {
                    for (i, l) in v.as_object().unwrap() {
                        self.skills.static_levels[i.parse::<usize>().unwrap()] = int(l);
                    }
                }
                "dynamic" => {
                    for (i, l) in v.as_object().unwrap() {
                        self.skills.dynamic_levels[i.parse::<usize>().unwrap()] = int(l);
                    }
                }
                "lp" => self.skills.lifepoints = int(v),
                "prayer" => self.skills.prayer_points = v.as_f64().unwrap(),
                "energy" => self.settings.fields.write().run_energy = v.as_f64().unwrap(),
                "clocks" => {
                    for (i, c) in v.as_object().unwrap() {
                        self.clocks[i.parse::<usize>().unwrap()] = int(c);
                    }
                }
                "attrs" => {
                    for (a, x) in v.as_object().unwrap() {
                        if x.is_null() {
                            self.attrs.remove(a);
                        } else {
                            self.attrs.insert(a.clone(), x.clone());
                        }
                    }
                }
                "inventory" | "equipment" => {
                    for r in v.as_array().unwrap() {
                        let slot = int(&r[0]) as usize;
                        let item = (!r[1].is_null())
                            .then(|| Item::at(int(&r[1]), int(&r[2]), slot as i32));
                        if k == "inventory" {
                            self.inventory[slot] = item;
                        } else {
                            self.equipment[slot] = item;
                        }
                    }
                }
                "timers" => {
                    for r in v.as_array().unwrap() {
                        let identifier = r[0].as_str().unwrap().to_string();
                        let hits_left = (identifier == "disease").then(|| r.get(1).map_or(25, int));
                        let label = self.label();
                        self.timers.push(TimerModel {
                            label,
                            identifier,
                            args: Vec::new(),
                            hits_left,
                        });
                    }
                }
                "sponge" => self.probe.sponge = v.as_bool().unwrap(),
                "familiar" => self.familiar = v.as_bool().unwrap(),
                "active" => self.active = v.as_bool(),
                other => panic!("field {other}"),
            }
        }
    }

    fn container(&mut self, kind: ContainerKind) -> &mut Vec<Option<Item>> {
        match kind {
            ContainerKind::Inventory => &mut self.inventory,
            ContainerKind::Equipment => &mut self.equipment,
        }
    }

    fn slot_index(&mut self, kind: ContainerKind, slot: i32) -> Result<usize> {
        let len = self.container(kind).len();
        usize::try_from(slot)
            .ok()
            .filter(|s| *s < len)
            .ok_or(Error::Index)
    }

    fn take(&mut self, kind: ContainerKind, slot: usize, item: &Item) -> bool {
        let old = self.container(kind)[slot].clone().unwrap();
        self.container(kind)[slot] = if item.amount >= old.amount {
            None
        } else {
            Some(Item::at(old.id(), old.amount - item.amount, slot as i32))
        };
        true
    }

    fn state(&self) -> Value {
        let s = &self.skills;
        let items = |c: &Vec<Option<Item>>| -> Value {
            Value::Array(
                c.iter()
                    .enumerate()
                    .filter_map(|(i, x)| x.as_ref().map(|x| json!([i, x.id(), x.amount, x.slot])))
                    .collect(),
            )
        };
        json!({
            "lp": s.lifepoints,
            "lpIncrease": s.lifepoints_increase,
            "lpUpdate": s.lifepoints_update,
            "dynamic": s.dynamic_levels.to_vec(),
            "prayer": s.prayer_points,
            "energy": self.settings.fields.read().run_energy,
            "clocks": self.clocks.to_vec(),
            "attrs": Value::Object(self.attrs.clone()),
            "inventory": items(&self.inventory),
            "equipment": items(&self.equipment),
            "timers": self.timers.iter().map(|t| json!([t.label, t.identifier, t.args, t.hits_left])).collect::<Vec<_>>(),
            "queued": self.queued.iter().map(|q| json!([q.strength.name(), q.next, q.persist])).collect::<Vec<_>>(),
            "pulses": self.pulses.iter().enumerate().map(|(i, p)| json!([format!("p{i}"), p.delay(), true])).collect::<Vec<_>>(),
        })
    }
}

impl Host for Replay<'_> {
    type SkillsContext = Probe;
    type SettingsContext = Probe;
    type Timer = String;
    type Familiar = ();

    fn skills(&mut self) -> (&mut Skills, &mut Probe) {
        (&mut self.skills, &mut self.probe)
    }
    fn settings(&mut self) -> (Settings, &mut Probe) {
        (self.settings.clone(), &mut self.probe)
    }
    fn clocks(&mut self) -> &mut [i32; 10] {
        &mut self.clocks
    }
    fn item_name(&self, definition: i32) -> String {
        self.case["names"][definition.to_string()]
            .as_str()
            .unwrap_or("null")
            .to_string()
    }
    fn disease_hits_left(&mut self, timer: &String) -> Result<i32> {
        Ok(self
            .timers
            .iter()
            .find(|t| &t.label == timer)
            .unwrap()
            .hits_left
            .unwrap())
    }
    fn set_disease_hits_left(&mut self, timer: &String, value: i32) -> Result<()> {
        self.timers
            .iter_mut()
            .find(|t| &t.label == timer)
            .unwrap()
            .hits_left = Some(value);
        Ok(())
    }
    fn ticks(&mut self) -> Result<i32> {
        self.probe.emit(json!(["ticks", self.tick]))?;
        Ok(self.tick)
    }
    fn attribute(&mut self, key: &str, default: &Attr) -> Result<Attr> {
        self.probe
            .emit(json!(["get_attribute", key, attr_value(default)]))?;
        Ok(match self.attrs.get(key) {
            None | Some(Value::Null) => default.clone(),
            Some(v) => attr_of(v),
        })
    }
    fn set_attribute(&mut self, key: &str, value: Attr) -> Result<()> {
        self.probe
            .emit(json!(["set_attribute", key, attr_value(&value)]))?;
        self.attrs.insert(key.into(), attr_value(&value));
        Ok(())
    }
    fn animate(&mut self, animation: Option<i32>) -> Result<bool> {
        self.probe.emit(json!(["animate", animation]))?;
        Ok(true)
    }
    fn play_audio(&mut self, id: i32, delay: i32, loops: i32) -> Result<()> {
        self.probe.emit(json!(["audio", id, delay, loops, null, 8]))
    }
    fn packet_message(&mut self, text: &str) -> Result<()> {
        self.probe.emit(json!(["packet_message", text]))
    }
    fn send_message(&mut self, text: &str) -> Result<()> {
        self.probe.emit(json!(["message", text]))
    }
    fn manual_hit(&mut self, hit: i32, kind: HitsplatType) -> Result<()> {
        self.probe.emit(json!(["manual_hit", 0, hit, kind.name()]))
    }
    fn send_chat(&mut self, text: &str) -> Result<bool> {
        self.probe.emit(json!(["chat", text]))?;
        Ok(true)
    }
    fn teleport(&mut self, location: [i32; 3]) -> Result<()> {
        self.probe.emit(json!(["teleport", location]))
    }
    fn run_pulse(&mut self, pulse: RumPulse) -> Result<()> {
        // A new pulse object each time the effect or the main pulse runs.
        let label = format!("p{}", self.pulses.len());
        let delay = pulse.delay();
        self.pulses.push(pulse);
        self.probe.emit(json!(["run_pulse", label, delay]))
    }
    fn finish_task(&mut self, diary: Diary, level: i32, task: i32) -> Result<()> {
        self.probe
            .emit(json!(["finish_task", diary.name(), level, task]))
    }
    fn familiar(&mut self) -> Result<Option<()>> {
        self.probe.emit(json!(["familiar"]))?;
        Ok(self.familiar.then_some(()))
    }
    fn update_special_points(&mut self, _: &(), diff: i32) -> Result<()> {
        self.probe.emit(json!(["special_points", diff]))
    }
    fn replace(
        &mut self,
        kind: ContainerKind,
        item: Option<Item>,
        slot: i32,
    ) -> Result<Option<Item>> {
        self.probe.emit(json!([
            "replace",
            kind.name(),
            describe(item.as_ref()),
            slot
        ]))?;
        let item = item.filter(|i| i.amount >= 1);
        let index = self.slot_index(kind, slot)?;
        let old = self.container(kind)[index].take();
        self.container(kind)[index] = item.map(|mut i| {
            i.slot = slot;
            i
        });
        Ok(old)
    }
    fn remove(&mut self, kind: ContainerKind, item: &Item) -> Result<bool> {
        self.probe
            .emit(json!(["remove", kind.name(), describe(Some(item))]))?;
        let found = self
            .container(kind)
            .iter()
            .position(|x| x.as_ref().is_some_and(|x| x.id() == item.id()));
        Ok(match found {
            Some(slot) => self.take(kind, slot, item),
            None => false,
        })
    }
    fn remove_slot(
        &mut self,
        kind: ContainerKind,
        item: &Item,
        slot: i32,
        fire: bool,
    ) -> Result<bool> {
        self.probe.emit(json!([
            "remove_slot",
            kind.name(),
            describe(Some(item)),
            slot,
            fire
        ]))?;
        let index = self.slot_index(kind, slot)?;
        let Some(old) = self.container(kind)[index].clone() else {
            return Ok(false);
        };
        if old.id() != item.id() || old.amount < item.amount {
            return Ok(false);
        }
        Ok(self.take(kind, index, item))
    }
    fn contains(&mut self, kind: ContainerKind, id: i32, amount: i32) -> Result<bool> {
        self.probe
            .emit(json!(["contains", kind.name(), id, amount]))?;
        let total = self
            .container(kind)
            .iter()
            .flatten()
            .filter(|i| i.id() == id)
            .fold(0i32, |t, i| t.wrapping_add(i.amount));
        Ok(total >= amount)
    }
    fn dupe_alert(&mut self, details: &str) -> Result<()> {
        self.probe
            .emit(json!(["player_log", "DUPE_ALERT", details]))
    }
    fn spawn_timer(&mut self, identifier: &str, args: &[TimerArg]) -> Result<Option<String>> {
        let values: Vec<Value> = args
            .iter()
            .map(|a| match a {
                TimerArg::Int(v) => json!(v),
                TimerArg::Bool(v) => json!(v),
            })
            .collect();
        self.probe
            .emit(json!(["spawn_timer", identifier, values]))?;
        let unknown = self.case["unknownTimers"]
            .as_array()
            .is_some_and(|u| u.iter().any(|x| x == identifier));
        if unknown {
            return Ok(None);
        }
        let label = self.label();
        let disease = identifier == "disease";
        self.timers_pending = Some(TimerModel {
            label: label.clone(),
            identifier: identifier.into(),
            args: if disease { Vec::new() } else { values },
            hits_left: disease.then_some(25),
        });
        Ok(Some(label))
    }
    fn register_timer(&mut self, timer: Option<&String>) -> Result<()> {
        self.probe.emit(json!(["register_timer", timer]))?;
        if let Some(label) = timer {
            let model = self
                .timers_pending
                .take()
                .filter(|t| &t.label == label)
                .unwrap();
            self.timers.push(model);
        }
        Ok(())
    }
    fn remove_timer(&mut self, identifier: &str) -> Result<()> {
        self.probe.emit(json!(["remove_timer", identifier]))?;
        self.timers.retain(|t| t.identifier != identifier);
        Ok(())
    }
    fn disease(&mut self) -> Result<Option<String>> {
        self.probe.emit(json!(["get_timer", "Disease"]))?;
        Ok(self
            .timers
            .iter()
            .find(|t| t.hits_left.is_some())
            .map(|t| t.label.clone()))
    }
    fn remove_disease(&mut self) -> Result<()> {
        self.probe.emit(json!(["remove_timer_class", "Disease"]))?;
        self.timers.retain(|t| t.hits_left.is_none());
        Ok(())
    }
    fn active_script_persist(&mut self) -> Result<Option<bool>> {
        self.probe.emit(json!(["active_script"]))?;
        Ok(self.active)
    }
    fn add_to_queue(
        &mut self,
        script: DoseScript,
        strength: QueueStrength,
        next: i32,
        persist: bool,
    ) -> Result<()> {
        self.probe
            .emit(json!(["add_to_queue", strength.name(), next, persist]))?;
        self.queued.push(Queued {
            script,
            strength,
            next,
            persist,
        });
        Ok(())
    }
    fn reset_scripts(&mut self) -> Result<()> {
        self.probe.emit(json!(["reset_scripts"]))
    }
    fn log(&mut self, level: Log, message: &str) -> Result<()> {
        self.probe.emit(json!(["log", level.name(), message]))
    }
    fn delay_next_attack(&mut self, ticks: i32) -> Result<()> {
        self.probe.emit(json!(["delay_next_attack", ticks]))
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        let value = self.rng.next_bound(bound).unwrap();
        self.probe.emit(json!(["next_int", bound, value]))?;
        Ok(value)
    }
}

fn d(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

/// The case's effect object, built with the named class's own constructor.
fn effect(v: &Value) -> Effect {
    let a = v["args"].as_array().cloned().unwrap_or_default();
    let s = |i: usize| a[i].as_str().unwrap().to_string();
    match v["class"].as_str().unwrap() {
        "AchievementEffect" => Effect::Achievement {
            diary: Diary::value_of(&s(0)).unwrap(),
            level: int(&a[1]),
            task: int(&a[2]),
        },
        "AddTimerEffect" => Effect::AddTimer {
            identifier: s(0),
            args: a[1..]
                .iter()
                .map(|x| match x {
                    Value::Bool(b) => TimerArg::Bool(*b),
                    other => TimerArg::Int(int(other)),
                })
                .collect(),
        },
        "CureDiseaseEffect" => Effect::CureDisease,
        "DamageEffect" => Effect::Damage {
            amt: d(&a[0]),
            is_percent: a[1].as_bool().unwrap(),
            base_amount: a.get(2).map_or(0, int),
        },
        "DraynorCabbageEffect" => Effect::DraynorCabbage,
        "DwarvenRockCakeEffect" => Effect::DwarvenRockCake,
        "EnergyEffect" => Effect::energy(int(&a[0])),
        "HealingEffect" => Effect::Healing(int(&a[0])),
        "KebabEffect" => Effect::Kebab,
        "KegOfBeerEffect" => Effect::KegOfBeer,
        "MatureWmbEffect" => Effect::MatureWmb,
        "MultiEffect" => Effect::Multi(a.iter().map(effect).collect()),
        "PercentageHealthEffect" => Effect::percentage_health(int(&a[0])),
        "PercentHeal" => Effect::PercentHeal {
            base: int(&a[0]),
            percent: d(&a[1]),
        },
        "PoisonEffect" => Effect::Poison(int(&a[0])),
        "PoisonKarambwanEffect" => Effect::PoisonKarambwan,
        "PrayerEffect" => Effect::Prayer {
            base: d(&a[0]),
            bonus: d(&a[1]),
            round_up: a.get(2).is_some_and(|x| x.as_bool().unwrap()),
        },
        "RandomEnergyEffect" => Effect::RandomEnergy(int(&a[0]), int(&a[1])),
        "RandomHealthEffect" => Effect::RandomHealth(int(&a[0]), int(&a[1])),
        "RandomPrayerEffect" => Effect::RandomPrayer(int(&a[0]), int(&a[1])),
        "RandomSkillEffect" => Effect::RandomSkill {
            slot: int(&a[0]),
            a: int(&a[1]),
            b: int(&a[2]),
        },
        "RemoveTimerEffect" => Effect::RemoveTimer(s(0)),
        "RestoreEffect" => Effect::Restore {
            base: d(&a[0]),
            bonus: d(&a[1]),
            all_skills: a.get(2).is_some_and(|x| x.as_bool().unwrap()),
        },
        "RestoreSummoningSpecial" => Effect::RestoreSummoningSpecial,
        "RockCakeEffect" => Effect::RockCake,
        "SetAttributeEffect" => {
            if a.len() == 2 {
                Effect::set_attribute(&s(0), attr_of(&a[1]))
            } else {
                Effect::SetAttribute {
                    key: s(0),
                    value: attr_of(&a[1]),
                    is_ticks: a[2].as_bool().unwrap(),
                }
            }
        }
        "SkillEffect" => Effect::Skill {
            slot: int(&a[0]),
            base: d(&a[1]),
            bonus: d(&a[2]),
        },
        "SmellingUgthankiKebabEffect" => Effect::SmellingUgthankiKebab,
        "SummoningEffect" => Effect::Summoning {
            base: d(&a[0]),
            bonus: d(&a[1]),
        },
        "SuperKebabEffect" => Effect::SuperKebab,
        "TroubleBrewingRumEffect" => Effect::TroubleBrewingRum(s(0)),
        "UgthankiKebabEffect" => Effect::UgthankiKebab,
        "WizardsMindBombEffect" => Effect::WizardsMindBomb,
        "ZamorakMixEffect" => Effect::ZamorakMix,
        other => panic!("effect {other}"),
    }
}

/// The effect object's own fields as the oracle reads them by reflection.
fn describe_effect(e: &Effect) -> Value {
    let unit = |class: &str| json!({ "class": class });
    match e {
        Effect::Achievement { diary, level, task } => {
            json!({"class": "AchievementEffect", "diary": diary.name(), "level": level, "task": task})
        }
        Effect::AddTimer { identifier, args } => json!({
            "class": "AddTimerEffect",
            "identifier": identifier,
            "args": args.iter().map(|a| match a { TimerArg::Int(v) => json!(v), TimerArg::Bool(v) => json!(v) }).collect::<Vec<_>>(),
        }),
        Effect::CureDisease => unit("CureDiseaseEffect"),
        Effect::Damage {
            amt,
            is_percent,
            base_amount,
        } => {
            json!({"class": "DamageEffect", "amt": amt, "isPercent": is_percent, "baseAmount": base_amount})
        }
        Effect::DraynorCabbage => unit("DraynorCabbageEffect"),
        Effect::DwarvenRockCake => unit("DwarvenRockCakeEffect"),
        Effect::Energy(amt) => json!({"class": "EnergyEffect", "amt": amt}),
        Effect::Healing(amt) => json!({"class": "HealingEffect", "amt": amt}),
        Effect::Kebab => unit("KebabEffect"),
        Effect::KegOfBeer => unit("KegOfBeerEffect"),
        Effect::MatureWmb => unit("MatureWmbEffect"),
        Effect::Multi(effects) => {
            json!({"class": "MultiEffect", "effects": effects.iter().map(describe_effect).collect::<Vec<_>>()})
        }
        Effect::PercentageHealth(p) => json!({"class": "PercentageHealthEffect", "percentage": p}),
        Effect::PercentHeal { base, percent } => {
            json!({"class": "PercentHeal", "base": base, "percent": percent})
        }
        Effect::Poison(amount) => json!({"class": "PoisonEffect", "amount": amount}),
        Effect::PoisonKarambwan => {
            json!({"class": "PoisonKarambwanEffect", "effect": {"class": "PoisonEffect", "amount": 5}})
        }
        Effect::Prayer {
            base,
            bonus,
            round_up,
        } => json!({"class": "PrayerEffect", "base": base, "bonus": bonus, "roundUp": round_up}),
        Effect::RandomEnergy(a, b) => json!({"class": "RandomEnergyEffect", "a": a, "b": b}),
        Effect::RandomHealth(a, b) => json!({"class": "RandomHealthEffect", "a": a, "b": b}),
        Effect::RandomPrayer(a, b) => json!({"class": "RandomPrayerEffect", "a": a, "b": b}),
        Effect::RandomSkill { slot, a, b } => {
            json!({"class": "RandomSkillEffect", "a": a, "b": b, "skillSlot": slot})
        }
        Effect::RemoveTimer(identifier) => {
            json!({"class": "RemoveTimerEffect", "identifier": identifier})
        }
        Effect::Restore {
            base,
            bonus,
            all_skills,
        } => json!({
            "class": "RestoreEffect",
            "base": base,
            "bonus": bonus,
            "all_skills": all_skills,
            "SKILLS": consumables::effects::RESTORE_SKILLS.to_vec(),
            "ALL_SKILLS": consumables::effects::RESTORE_ALL_SKILLS.to_vec(),
        }),
        Effect::RestoreSummoningSpecial => unit("RestoreSummoningSpecial"),
        Effect::RockCake => unit("RockCakeEffect"),
        Effect::SetAttribute {
            key,
            value,
            is_ticks,
        } => {
            json!({"class": "SetAttributeEffect", "attrString": key, "attrValue": attr_value(value), "isTicks": is_ticks})
        }
        Effect::Skill { slot, base, bonus } => {
            json!({"class": "SkillEffect", "base": base, "bonus": bonus, "skill_slot": slot})
        }
        Effect::SmellingUgthankiKebab => unit("SmellingUgthankiKebabEffect"),
        Effect::Summoning { base, bonus } => {
            json!({"class": "SummoningEffect", "base": base, "bonus": bonus})
        }
        Effect::SuperKebab => unit("SuperKebabEffect"),
        Effect::TroubleBrewingRum(chat) => {
            json!({"class": "TroubleBrewingRumEffect", "forceChatMessage": chat})
        }
        Effect::UgthankiKebab => unit("UgthankiKebabEffect"),
        Effect::WizardsMindBomb => unit("WizardsMindBombEffect"),
        Effect::ZamorakMix => unit("ZamorakMixEffect"),
    }
}

fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Binding(_) => "Binding",
        Error::Null => "Null",
        Error::Type => "Cast",
        Error::Index => "Index",
        Error::Number => "Number",
    }
}

fn op_item(op: &Value) -> Item {
    let a = op["item"].as_array().unwrap();
    Item::at(int(&a[0]), int(&a[1]), int(&a[2]))
}

fn row_of<'t>(table: &'t Table, op: &Value) -> &'t consumables::Consumable {
    &table.rows[table.value_of(op["row"].as_str().unwrap()).unwrap()].consumable
}

fn table_rows(table: &Table) -> Value {
    Value::Array(
        table
            .rows
            .iter()
            .map(|r| {
                let c = &r.consumable;
                json!([
                    r.name,
                    c.kind.class_name(),
                    c.ids,
                    r.ignore_main_clock,
                    c.animation,
                    c.messages,
                    c.effect.as_ref().map(describe_effect)
                ])
            })
            .collect(),
    )
}

fn run(replay: &mut Replay<'_>, table: &Table, op: &Value) -> Result<Value> {
    Ok(match op["op"].as_str().unwrap() {
        "set" => {
            replay.apply(&op["fields"]);
            Value::Null
        }
        "tick" => {
            replay.tick = int(&op["value"]);
            Value::Null
        }
        "define" => {
            consumables::define_listeners(|kind, options| {
                replay.probe.emit(json!(["on", kind, options]))
            })?;
            Value::Null
        }
        "handle" => {
            let node = if let Some(slot) = op["slot"].as_i64() {
                match replay.inventory[slot as usize].clone() {
                    None => return Ok(json!("empty slot")),
                    Some(item) => Node::Item(item),
                }
            } else if let Some(id) = op["other"].as_i64() {
                Node::Other { id: id as i32 }
            } else {
                Node::Item(op_item(op))
            };
            json!(consumables::handle(table, &node, replay)?)
        }
        "consume" => {
            row_of(table, op).consume(table, &op_item(op), replay)?;
            Value::Null
        }
        "health" => json!(row_of(table, op).health_value(replay)?),
        "formatted" => {
            let item = op_item(op);
            json!(row_of(table, op).formatted_name(&replay.item_name(item.definition)))
        }
        "dose" => {
            let consumable = row_of(table, op);
            if !matches!(consumable.kind, Kind::Potion | Kind::BarbarianMix) {
                return Err(Error::Type);
            }
            let item = op_item(op);
            json!(consumable.dose(&item, &replay.item_name(item.definition))?)
        }
        "lookup" => json!(table.lookup(int(&op["id"])).map(|r| table.rows[r].name)),
        "table" => table_rows(table),
        "potions" => json!(table.potions),
        "effect" => {
            effect(&op["effect"]).activate(replay)?;
            Value::Null
        }
        "effectHealth" => json!(effect(&op["effect"]).health_value(replay)?),
        "describe" => describe_effect(&effect(&op["effect"])),
        "script" => {
            let script = replay
                .queued
                .get(int(&op["index"]) as usize)
                .ok_or(Error::Index)?
                .script
                .clone();
            json!(script.run(replay, op.get("stage").map_or(0, int))?)
        }
        "pulse" => {
            let pulse = replay
                .pulses
                .get(int(&op["index"]) as usize)
                .ok_or(Error::Index)?
                .clone();
            json!(pulse.pulse(replay)?)
        }
        other => panic!("op {other}"),
    })
}

fn replay_case(table: &Table, row: &Value) -> Vec<String> {
    let case = &row["input"];
    let mut replay = Replay::new(case);
    let mut previous = Value::Null;
    let mut differences = Vec::new();
    for (k, (op, expected)) in case["ops"]
        .as_array()
        .unwrap()
        .iter()
        .zip(row["expected"].as_array().unwrap())
        .enumerate()
    {
        replay.probe.events.clear();
        replay.probe.calls = 0;
        replay.probe.fail_at = op["failAt"].as_i64().unwrap_or(-1);
        let outcome = run(&mut replay, table, op);
        replay.probe.fail_at = -1;
        let (result, error) = match outcome {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(error_name(&e))),
        };
        let state = replay.state();
        let player = if state == previous {
            Value::Null
        } else {
            state.clone()
        };
        previous = state;
        let actual = json!({"result": result, "error": error, "events": replay.probe.events, "player": player});
        for key in ["result", "error", "events", "player"] {
            if actual[key] != expected[key] {
                differences.push(format!(
                    "op {k} {key}: expected {} got {}",
                    expected[key], actual[key]
                ));
            }
        }
        if !differences.is_empty() {
            break;
        }
    }
    differences
}

#[test]
fn corpus_matches_provenance() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(provenance["fixtureSha256"], json!(digest));
    assert_eq!(provenance["caseCount"], json!(CORPUS.lines().count()));
}

#[test]
fn every_case_replays() {
    let table = Table::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut count = 0;
    for line in CORPUS.lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        count += 1;
        let differences = replay_case(&table, &row);
        if !differences.is_empty() {
            let group = row["input"]["group"].as_str().unwrap().to_string();
            failures.push((
                group,
                format!("{} :: {}", row["input"]["ops"], differences.join(" | ")),
            ));
        }
    }
    let mut unexpected: Vec<&(String, String)> = failures
        .iter()
        .filter(|(g, _)| !GAPS.iter().any(|(gap, _)| gap == g))
        .collect();
    unexpected.truncate(12);
    assert!(
        unexpected.is_empty(),
        "{} of {count} cases differ, first: {unexpected:#?}",
        failures.len()
    );
    for (gap, reason) in GAPS {
        assert!(
            failures.iter().any(|(g, _)| g == gap),
            "gap {gap} ({reason}) is closed"
        );
    }
}
