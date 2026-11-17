//! Replays the original dragonfire corpus against the native port
//! (`world::dragonfire` joined to `world::combat_pulse` through its override
//! table). The host below is the special-attacks replay host extended with
//! the dragonfire services: it answers every required service from the case
//! input and from the same small world model the JVM stand-ins keep (items
//! are shared handles with the original packed id and charge), and records the
//! same ordered events. The model is a test input, not a port of the classes
//! it stands in for.
use rs09_game_core::random::JavaRandom;
use rs09_server::player::settings::{self, Settings};
use rs09_server::world::combat_pulse::{
    self as cp, AmmunitionVisuals, HitsplatType, Impact, ImpactState, Prayer, ProjectileRequest,
    PulseState, Specials, Task, WorldPulse, handler, pulse,
};
use rs09_server::world::combat_swing::{
    self as cs, Ammunition, ArmourSet, AttackStyle, BattleState, BoltEffect, CombatStyle, Error,
    Graphics, Handler, HandlerKind, Kind, Perk, RangeData, RangeWeapon, SlayerTask, Target,
    WeaponInterfaces,
};
use rs09_server::world::dragonfire::{
    self as df, DragonProjectile, ProjectileTemplate, Subclass, SwitchAttack, api,
    dragons::{self, DragonClass},
    fire_type::FireType,
    multi, shield,
};
use rs09_server::world::npc_classes::Abstract;
use rs09_server::world::special_attacks::{self as sa, Special};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

const FIXTURE: &str = include_str!("../../../fixtures/dragonfire/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/dragonfire/provenance.json");

/// The handler instances every case makes: the three `CombatStyle`
/// singletons, the three MixedSwingHandlers objects, then the two
/// class-identity stand-ins `Player.getSwingHandler` can answer.
const CHINCHOMPA: usize = 6;
const SALAMANDER: usize = 7;

#[derive(Clone)]
struct Ent(usize);

/// An `Item` object: a shared handle, so a charge set through one reference
/// is seen through every other, with the original packed `idHash`.
#[derive(Clone)]
struct Item(Rc<RefCell<ItemData>>);

struct ItemData {
    reference: String,
    id_hash: i32,
    amount: i32,
    name: String,
    bonuses: Option<Vec<i32>>,
    extras: Value,
    index: i32,
}

impl Item {
    #[allow(clippy::too_many_arguments)]
    fn new(
        reference: String,
        id: i32,
        amount: i32,
        charge: i32,
        name: String,
        bonuses: Option<Vec<i32>>,
        extras: Value,
        index: i32,
    ) -> Self {
        Self(Rc::new(RefCell::new(ItemData {
            reference,
            id_hash: id.wrapping_shl(16) | charge,
            amount,
            name,
            bonuses,
            extras,
            index,
        })))
    }

    fn id(&self) -> i32 {
        (self.0.borrow().id_hash >> 16) & 0xFFFF
    }

    fn charge(&self) -> i32 {
        self.0.borrow().id_hash & 0xFFFF
    }

    fn shown(&self) -> Value {
        let d = self.0.borrow();
        json!([
            d.reference,
            (d.id_hash >> 16) & 0xFFFF,
            d.amount,
            d.id_hash & 0xFFFF,
            d.name,
            d.index
        ])
    }
}

#[derive(Clone)]
struct Spell(usize);

/// A container model: items read once are kept, emptied slots remembered.
#[derive(Default)]
struct Held {
    items: HashMap<i32, Item>,
    emptied: HashSet<i32>,
}

impl Held {
    fn shown(&self) -> Value {
        let mut slots: Vec<&i32> = self.items.keys().collect();
        slots.sort();
        let mut out: Vec<Value> = slots
            .into_iter()
            .map(|s| json!([s, self.items[s].shown()]))
            .collect();
        let mut emptied: Vec<&i32> = self.emptied.iter().collect();
        emptied.sort();
        out.extend(emptied.into_iter().map(|s| json!([s, null])));
        Value::Array(out)
    }
}

/// What the JVM stand-in of one entity keeps besides its case input.
struct Model {
    spec: Value,
    lifepoints: i32,
    recoil: i32,
    prayer_points: f64,
    dead: bool,
    registered: bool,
    armour_set: Option<String>,
    attrs: HashMap<String, Value>,
    equipment: Held,
    inventory: Held,
    level_answer: usize,
}

enum Queued {
    Combat(usize),
    World(Box<WorldPulse<World>>),
}

/// The ordered event record, shared with the native `Settings` context.
struct Recorder {
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    tag: usize,
}

impl Recorder {
    fn emit(&mut self, row: Value) -> cs::Result<()> {
        self.events.push(row);
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }
}

struct World {
    ents: Vec<Model>,
    spells: Vec<Value>,
    spell: Vec<Option<usize>>,
    autocast: Vec<Option<usize>>,
    range: RangeData,
    visuals: HashMap<i32, AmmunitionVisuals>,
    weapon_animation: HashMap<i32, Option<i32>>,
    handlers: Vec<Handler>,
    subclasses: Vec<Option<Subclass<World>>>,
    names: Vec<&'static str>,
    /// The handlers the case made; later ones are the shield's anonymous handlers.
    listed: usize,
    specials: Vec<Specials<World>>,
    pulses: Vec<PulseState<World>>,
    impacts: Vec<ImpactState<World>>,
    xp_mod: f64,
    queue: VecDeque<Queued>,
    tick: i32,
    millis: i64,
    clip: Value,
    pclip: Value,
    uids: Value,
    inactive: Vec<i64>,
    rec: Recorder,
    math_script: Option<Vec<Value>>,
    int_script: Option<Vec<Value>>,
    math_at: usize,
    int_at: usize,
    math: JavaRandom,
    ints: JavaRandom,
    settings: Vec<Option<Settings>>,
    case: Value,
    /// Each entity's dragon class and its own combat action.
    dragon_of: Vec<Option<DragonClass>>,
    combat_of: Vec<Option<usize>>,
    /// `ChromaticDragonBehavior.EGG_RATE`.
    egg_rate: i32,
}

fn int(value: &Value) -> i32 {
    i32::try_from(value.as_i64().unwrap()).unwrap()
}

fn bits(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

fn hex(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn ints(value: &Value) -> Vec<i32> {
    value.as_array().unwrap().iter().map(int).collect()
}

fn task(value: &Value) -> Option<SlayerTask> {
    let spec = value.as_object()?;
    Some(SlayerTask {
        ids: spec.get("ids").map(ints).unwrap_or_default(),
        level_req: spec.get("levelReq").map_or(1, int),
        dragon: spec.get("dragon") == Some(&Value::Bool(true)),
        undead: spec.get("undead") == Some(&Value::Bool(true)),
        kalphites: spec.get("kalphites") == Some(&Value::Bool(true)),
    })
}

fn tag(entity: Option<&Ent>) -> Value {
    entity.map_or(Value::Null, |e| json!(e.0))
}

fn graphic(g: Option<Graphics>) -> Value {
    g.map_or(Value::Null, |g| json!([g.id, g.height, g.delay]))
}

fn graphics(value: &Value) -> Option<Graphics> {
    let row = value.as_array()?;
    Some(Graphics {
        id: int(&row[0]),
        height: int(&row[1]),
        delay: int(&row[2]),
    })
}

fn class(error: &Error) -> &'static str {
    match error {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Binding(_) => "Binding",
    }
}

fn style(value: &Value) -> Option<CombatStyle> {
    let name = value.as_str()?;
    CombatStyle::ALL.into_iter().find(|s| s.name() == name)
}

fn entity(value: &Value) -> Option<Ent> {
    value.as_u64().map(|i| Ent(i as usize))
}

impl World {
    fn emit(&mut self, row: Value) -> cs::Result<()> {
        self.rec.emit(row)
    }

    fn mark(&mut self, row: Value) {
        self.rec.events.push(row);
    }

    fn spec(&self, e: &Ent) -> &Value {
        &self.ents[e.0].spec
    }

    fn number(&self, e: &Ent, key: &str, fallback: i32) -> i32 {
        match &self.spec(e)[key] {
            Value::Null => fallback,
            value => int(value),
        }
    }

    fn flag(&self, e: &Ent, key: &str) -> bool {
        self.spec(e)[key] == true
    }

    fn unless(&self, e: &Ent, key: &str) -> bool {
        self.spec(e)[key] != false
    }

    fn has(&self, e: &Ent, key: &str, name: &str) -> bool {
        self.spec(e)[key]
            .as_array()
            .is_some_and(|list| list.iter().any(|v| v == name))
    }

    fn keyed(&self, e: &Ent, key: &str, slot: i32, fallback: i32) -> i32 {
        match &self.spec(e)[key][slot.to_string()] {
            Value::Null => fallback,
            value => int(value),
        }
    }

    fn double(&self, e: &Ent, key: &str, fallback: f64) -> f64 {
        match &self.spec(e)[key] {
            Value::Null => fallback,
            value => bits(value),
        }
    }

    fn place(&self, e: &Ent) -> Option<[i32; 3]> {
        match self.spec(e).get("location") {
            None => Some([3200, 3200, 0]),
            Some(Value::Null) => None,
            Some(at) => Some(handler::location(int(&at[0]), int(&at[1]), int(&at[2]))),
        }
    }

    /// The equipment model's silent slot read.
    fn peek(&mut self, e: &Ent, slot: i32) -> Option<Item> {
        let model = &mut self.ents[e.0];
        if model.equipment.emptied.contains(&slot) {
            return None;
        }
        if let Some(item) = model.equipment.items.get(&slot) {
            return Some(item.clone());
        }
        let row = model.spec["equipment"][slot.to_string()].as_array()?;
        let made = Item::new(
            format!("{}:{slot}", e.0),
            int(&row[0]),
            int(&row[1]),
            int(&row[2]),
            row[3].as_str().unwrap().to_owned(),
            row.get(4).filter(|v| !v.is_null()).map(ints),
            row.get(5).cloned().unwrap_or(Value::Null),
            slot,
        );
        model.equipment.items.insert(slot, made.clone());
        Some(made)
    }

    /// The inventory model's silent slot read.
    fn inventory_at(&mut self, e: &Ent, slot: i32) -> Option<Item> {
        let model = &mut self.ents[e.0];
        if model.inventory.emptied.contains(&slot) {
            return None;
        }
        if let Some(item) = model.inventory.items.get(&slot) {
            return Some(item.clone());
        }
        let row = model.spec["inventory"][slot.to_string()].as_array()?;
        let made = Item::new(
            format!("inv:{}:{slot}", e.0),
            int(&row[0]),
            int(&row[1]),
            int(&row[2]),
            row[3].as_str().unwrap().to_owned(),
            None,
            Value::Null,
            slot,
        );
        model.inventory.items.insert(slot, made.clone());
        Some(made)
    }

    /// `EquipmentContainer.store`: the old item's name is kept for the same id.
    fn store_equipment(&mut self, e: &Ent, slot: i32, item: Option<(i32, i32, i32, String)>) {
        match item {
            Some((id, amount, charge, name)) if amount > 0 => {
                let old = self.peek(e, slot);
                let name = match old {
                    Some(old) if old.id() == id => old.0.borrow().name.clone(),
                    _ => name,
                };
                let made = Item::new(
                    format!("{}:{slot}", e.0),
                    id,
                    amount,
                    charge,
                    name,
                    None,
                    Value::Null,
                    slot,
                );
                let held = &mut self.ents[e.0].equipment;
                held.emptied.remove(&slot);
                held.items.insert(slot, made);
            }
            _ => {
                let held = &mut self.ents[e.0].equipment;
                held.emptied.insert(slot);
                held.items.remove(&slot);
            }
        }
    }

    fn attr_entity(&self, e: &Ent, key: &str) -> Option<Ent> {
        entity(&self.ents[e.0].attrs.get(key)?["entity"])
    }

    fn script(&self, e: &Ent, key: &str, state: &mut BattleState<Self>) {
        if let Some(fields) = self.spec(e)[key].as_object() {
            if let Some(v) = fields.get("estimatedHit") {
                state.estimated_hit = int(v);
            }
            if let Some(v) = fields.get("secondaryHit") {
                state.secondary_hit = int(v);
            }
        }
    }

    fn map_flag(map: &Value, z: i32, x: i32, y: i32) -> i32 {
        match &map[format!("{x},{y},{z}")] {
            Value::Null => 0,
            value => int(value),
        }
    }

    fn animation(&self, e: &Ent, key: &str, fallback: i32) -> Option<i32> {
        match self.spec(e).get(key) {
            None => Some(fallback),
            Some(Value::Null) => None,
            Some(value) => Some(int(value)),
        }
    }
}

impl cs::Types for World {
    type Entity = Ent;
    type Item = Item;
    type Spell = Spell;
}

impl cs::Host for World {
    fn kind(&self, e: &Ent) -> Kind {
        match self.spec(e)["kind"].as_str().unwrap_or("player") {
            "player" => Kind::Player,
            "npc" => Kind::Npc,
            "familiar" => Kind::Familiar,
            _ => Kind::Other,
        }
    }
    fn same_spell(&self, a: &Spell, b: &Spell) -> bool {
        a.0 == b.0
    }
    fn item_id(&self, item: &Item) -> i32 {
        item.id()
    }
    fn item_name(&self, item: &Item) -> String {
        item.0.borrow().name.clone()
    }
    fn item_amount(&self, item: &Item) -> i32 {
        item.0.borrow().amount
    }
    fn range_weapon(&self, id: i32) -> Option<Arc<RangeWeapon>> {
        self.range.range_weapon(id)
    }
    fn ammunition(&self, id: i32) -> Option<Arc<Ammunition>> {
        self.range.ammunition(id)
    }
    fn bonuses(&mut self, e: &Ent) -> cs::Result<Option<Vec<i32>>> {
        self.emit(json!(["bonuses", e.0]))?;
        Ok(match self.spec(e).get("bonuses") {
            None => Some(vec![0; 15]),
            Some(Value::Null) => None,
            Some(list) => Some(ints(list)),
        })
    }
    fn attack_style(&mut self, e: &Ent) -> cs::Result<Option<AttackStyle>> {
        self.emit(json!(["attackStyle", e.0]))?;
        Ok(match self.spec(e).get("attackStyle") {
            None => Some(AttackStyle {
                style: 0,
                bonus_type: 2,
            }),
            Some(Value::Null) => None,
            Some(pair) => Some(AttackStyle {
                style: int(&pair[0]),
                bonus_type: int(&pair[1]),
            }),
        })
    }
    fn armour_set(&mut self, e: &Ent) -> cs::Result<Option<ArmourSet>> {
        self.emit(json!(["armourSet", e.0]))?;
        let name = self.ents[e.0].armour_set.clone();
        Ok(name.map(|name| {
            ArmourSet::ALL
                .into_iter()
                .find(|s| s.name() == name)
                .unwrap()
        }))
    }
    fn pulse_victim(&mut self, e: &Ent) -> cs::Result<Option<Ent>> {
        Ok(self.pulses[e.0].victim.clone())
    }
    fn stop_combat_pulse(&mut self, e: &Ent) -> cs::Result<()> {
        pulse::stop(self, e)
    }
    fn update_combat_style(&mut self, e: &Ent) -> cs::Result<()> {
        pulse::update_style(self, e)
    }
    fn spell(&mut self, e: &Ent) -> cs::Result<Option<Spell>> {
        self.emit(json!(["spell", e.0]))?;
        Ok(self.spell[e.0].map(Spell))
    }
    fn clear_spell(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["setSpell", e.0, null]))?;
        self.spell[e.0] = None;
        Ok(())
    }
    fn autocast_spell(&mut self, e: &Ent) -> cs::Result<Option<Spell>> {
        self.emit(json!(["autocast", e.0]))?;
        Ok(self.autocast[e.0].map(Spell))
    }
    fn level(&mut self, e: &Ent, slot: i32) -> cs::Result<i32> {
        self.emit(json!(["level", e.0, slot]))?;
        Ok(self.keyed(e, "levels", slot, 1))
    }
    fn level_with(&mut self, e: &Ent, slot: i32, discard_assist: bool) -> cs::Result<i32> {
        self.emit(json!(["levelAssist", e.0, slot, discard_assist]))?;
        Ok(self.keyed(e, "levels", slot, 1))
    }
    fn static_level(&mut self, e: &Ent, slot: i32) -> cs::Result<i32> {
        self.emit(json!(["staticLevel", e.0, slot]))?;
        Ok(self.keyed(e, "staticLevels", slot, 1))
    }
    fn lifepoints(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["lifepoints", e.0]))?;
        Ok(self.ents[e.0].lifepoints)
    }
    fn maximum_lifepoints(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["maxLifepoints", e.0]))?;
        Ok(self.number(e, "maxLifepoints", 10))
    }
    fn prayer_skill_bonus(&mut self, e: &Ent, slot: i32) -> cs::Result<f64> {
        self.emit(json!(["prayerBonus", e.0, slot]))?;
        Ok(match &self.spec(e)["prayerBonus"][slot.to_string()] {
            Value::Null => 0.0,
            value => bits(value),
        })
    }
    fn protects_from_summoning(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["prayer", e.0, "PROTECT_FROM_SUMMONING"]))?;
        Ok(self.has(e, "prayers", "PROTECT_FROM_SUMMONING"))
    }
    fn has_protection_prayer(&mut self, e: &Ent, style: CombatStyle) -> cs::Result<bool> {
        self.emit(json!(["protection", e.0, style.name()]))?;
        Ok(self.has(e, "protection", style.name()))
    }
    fn perk_active(&mut self, e: &Ent, perk: Perk) -> cs::Result<bool> {
        let name = match perk {
            Perk::PrecisionStrikes => "PRECISION_STRIKES",
            Perk::AccurateMarksman => "ACCURATE_MARKSMAN",
        };
        self.emit(json!(["perk", e.0, name]))?;
        Ok(self.has(e, "perks", name))
    }
    fn wearing_void(&mut self, e: &Ent, style: CombatStyle) -> cs::Result<bool> {
        self.emit(json!(["void", e.0, style.name()]))?;
        Ok(self.has(e, "void", style.name()))
    }
    fn name(&mut self, e: &Ent) -> cs::Result<Option<String>> {
        self.emit(json!(["name", e.0]))?;
        Ok(match self.spec(e).get("name") {
            None => Some("null".to_owned()),
            Some(value) => value.as_str().map(str::to_owned),
        })
    }
    fn id(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["id", e.0]))?;
        Ok(self.number(e, "id", 0))
    }
    fn size(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["size", e.0]))?;
        Ok(self.number(e, "size", 1))
    }
    fn location(&mut self, e: &Ent) -> cs::Result<Option<[i32; 3]>> {
        self.emit(json!(["location", e.0]))?;
        Ok(self.place(e))
    }
    fn level_mod(&mut self, e: &Ent, victim: Option<&Ent>) -> cs::Result<f64> {
        self.emit(json!(["levelMod", e.0, e.0, tag(victim)]))?;
        Ok(self.double(e, "levelMod", 0.0))
    }
    fn equipment(&mut self, e: &Ent, slot: i32) -> cs::Result<Option<Item>> {
        self.emit(json!(["equip", e.0, slot]))?;
        if !(0..14).contains(&slot) {
            return Ok(None);
        }
        Ok(self.peek(e, slot))
    }
    fn equipment_new(&mut self, e: &Ent, slot: i32) -> cs::Result<Item> {
        self.emit(json!(["equipNew", e.0, slot]))?;
        if !(0..14).contains(&slot) {
            return Err(Error::Index);
        }
        Ok(self.peek(e, slot).unwrap_or_else(|| {
            Item::new(
                "new".into(),
                0,
                1,
                1000,
                "item0".into(),
                None,
                Value::Null,
                -1,
            )
        }))
    }
    fn equipment_contains_one(&mut self, e: &Ent, id: i32) -> cs::Result<bool> {
        self.emit(json!(["containsOne", e.0, id]))?;
        Ok(self.has(e, "contains", &id.to_string()))
    }
    fn item_bonuses(&mut self, item: &Item) -> cs::Result<Option<Vec<i32>>> {
        let reference = item.0.borrow().reference.clone();
        self.emit(json!(["itemConfig", reference, "bonuses"]))?;
        Ok(item.0.borrow().bonuses.clone())
    }
    fn slayer_task(&mut self, e: &Ent) -> cs::Result<Option<SlayerTask>> {
        self.emit(json!(["slayerTask", e.0]))?;
        Ok(task(&self.spec(e)["slayerTask"]))
    }
    fn slayer_bonus(&mut self, e: &Ent) -> cs::Result<f64> {
        self.emit(json!(["slayerBonus", e.0]))?;
        Ok(self.double(e, "slayerBonus", 1.0))
    }
    fn in_equipment(&mut self, e: &Ent, id: i32, amount: i32) -> cs::Result<bool> {
        self.emit(json!(["inEquipment", e.0, id, amount]))?;
        Ok(self.has(e, "inEquipment", &id.to_string()))
    }
    fn familiar_pouch(&mut self, e: &Ent) -> cs::Result<Option<i32>> {
        let spec = self.spec(e)["familiar"].clone();
        if spec == "manager" {
            return Ok(None);
        }
        self.emit(json!(["familiar", e.0]))?;
        if spec == "throw" {
            return Err(Error::Binding("familiar".into()));
        }
        if self.flag(e, "familiarPet") {
            return Ok(Some(0));
        }
        Ok(spec.as_i64().map(|pouch| pouch as i32))
    }
    fn npc_task(&mut self, e: &Ent) -> cs::Result<Option<SlayerTask>> {
        self.emit(json!(["npcTask", e.0]))?;
        Ok(task(&self.spec(e)["npcTask"]))
    }
    fn npc_combat_distance(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["combatDistance", e.0]))?;
        Ok(self.number(e, "combatDistance", 0))
    }
    fn send_message(&mut self, e: &Ent, text: &str) -> cs::Result<()> {
        self.emit(json!(["message", e.0, text]))
    }
    fn log_error(&mut self, origin: &'static str, text: &str) -> cs::Result<()> {
        self.emit(json!(["log", origin, "ERR", text]))
    }
    fn play_global_audio(&mut self, location: [i32; 3], id: i32) -> cs::Result<()> {
        self.emit(json!(["audio", location, id, 0, 1, 8]))
    }
    fn graphics(&mut self, e: &Ent, graphics: Graphics) -> cs::Result<()> {
        self.emit(json!([
            "graphics",
            e.0,
            graphics.id,
            graphics.height,
            graphics.delay
        ]))
    }
    fn weapon_interface(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["extension", e.0, "WeaponInterface"]))?;
        Ok(!self.spec(e)["weaponInterface"].is_null())
    }
    fn select_auto_spell(
        &mut self,
        e: &Ent,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> cs::Result<()> {
        self.emit(json!(["selectAutoSpell", e.0, button, adjust, forget]))
    }
    fn spell_meets_requirements(
        &mut self,
        spell: &Spell,
        caster: &Ent,
        message: bool,
        remove: bool,
    ) -> cs::Result<bool> {
        self.emit(json!(["meets", spell.0, caster.0, message, remove]))?;
        Ok(self.spells[spell.0]["meets"] != false)
    }
    fn spell_maximum_impact(
        &mut self,
        spell: &Spell,
        entity: &Ent,
        victim: Option<&Ent>,
        _state: &BattleState<Self>,
    ) -> cs::Result<i32> {
        self.emit(json!(["maxImpact", spell.0, entity.0, tag(victim)]))?;
        Ok(self.spells[spell.0].get("maxImpact").map_or(0, int))
    }
    fn spell_targets(
        &mut self,
        spell: &Spell,
        entity: &Ent,
        victim: Option<&Ent>,
    ) -> cs::Result<Option<Vec<Option<BattleState<Self>>>>> {
        self.emit(json!(["targets", spell.0, entity.0, tag(victim)]))?;
        Ok(Some(vec![Some(BattleState::new(
            Some(entity.clone()),
            victim.cloned(),
        ))]))
    }
    fn spell_is_blitz(&mut self, spell: &Spell) -> cs::Result<bool> {
        self.emit(json!(["spellType", spell.0]))?;
        Ok(self.spells[spell.0]["type"] == "BLITZ")
    }
    fn math_random(&mut self) -> cs::Result<f64> {
        let value = if let Some(script) = &self.math_script {
            self.math_at += 1;
            bits(&script[self.math_at - 1])
        } else {
            self.math.next_double()
        };
        self.emit(json!(["mathRandom", hex(value)]))?;
        Ok(value)
    }
    fn next_int(&mut self, bound: i32) -> cs::Result<i32> {
        let value = if let Some(script) = &self.int_script {
            self.int_at += 1;
            match &script[self.int_at - 1] {
                Value::String(_) => bound - 1,
                number => int(number).rem_euclid(bound),
            }
        } else {
            self.ints.next_bound(bound).unwrap()
        };
        self.emit(json!(["nextInt", bound, value]))?;
        Ok(value)
    }
}

impl cp::Host for World {
    type HandlerRef = usize;
    type Animation = i32;
    type Ground = ();

    fn handler(&mut self, handler: &usize) -> &mut Handler {
        &mut self.handlers[*handler]
    }
    fn is_salamander(&self, handler: &usize) -> bool {
        *handler == SALAMANDER
    }
    fn handler_class_name(&self, handler: &usize) -> String {
        self.names[*handler].into()
    }
    fn overrides(&self, handler: &usize) -> Option<cp::Overrides<Self>> {
        self.subclasses[*handler]
            .as_ref()
            .map(df::overrides::<Self>)
    }
    fn specials(&mut self, handler: &usize) -> &mut Specials<Self> {
        &mut self.specials[*handler]
    }
    fn same_entity(&self, a: &Ent, b: &Ent) -> bool {
        a.0 == b.0
    }
    fn is_ai_player(&self, e: &Ent) -> bool {
        self.spec(e)["kind"] == "aiplayer"
    }
    fn identity_hash(&self, e: &Ent) -> i32 {
        self.number(e, "hash", 31 * e.0 as i32 + 7)
    }
    fn combat_pulse(&mut self, e: &Ent) -> &mut PulseState<Self> {
        &mut self.pulses[e.0]
    }
    fn impact_handler(&mut self, e: &Ent) -> &mut ImpactState<Self> {
        &mut self.impacts[e.0]
    }
    fn experience_mod(&mut self) -> &mut f64 {
        &mut self.xp_mod
    }
    fn item_charge(&self, item: &Item) -> i32 {
        item.charge()
    }
    fn animation_id(&self, animation: &i32) -> i32 {
        *animation
    }
    fn ammunition_visuals(&self, item_id: i32) -> AmmunitionVisuals {
        self.visuals[&item_id]
    }
    fn range_weapon_animation(&self, item_id: i32) -> Option<i32> {
        self.weapon_animation[&item_id]
    }
    fn attribute_flag(&mut self, e: &Ent, key: &str) -> cs::Result<bool> {
        self.emit(json!(["attr", e.0, key]))?;
        Ok(self.ents[e.0].attrs.get(key) == Some(&Value::Bool(true)))
    }
    fn autocast_component(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["attr", e.0, "autocast_component"]))?;
        Ok(self.ents[e.0].attrs.contains_key("autocast_component"))
    }
    fn has_attribute(&mut self, e: &Ent, key: &str) -> cs::Result<bool> {
        self.emit(json!(["attrHas", e.0, key]))?;
        Ok(self.ents[e.0].attrs.contains_key(key))
    }
    fn remove_attribute(&mut self, e: &Ent, key: &str) -> cs::Result<()> {
        self.emit(json!(["removeAttr", e.0, key]))?;
        self.ents[e.0].attrs.remove(key);
        Ok(())
    }
    fn random_event_npc(&mut self, e: &Ent) -> cs::Result<Option<Ent>> {
        self.emit(json!(["attr", e.0, "re-npc"]))?;
        Ok(self.attr_entity(e, "re-npc"))
    }
    fn combat_attacker(&mut self, e: &Ent) -> cs::Result<Option<Ent>> {
        self.emit(json!(["attr", e.0, "combat-attacker"]))?;
        Ok(self.attr_entity(e, "combat-attacker"))
    }
    fn set_combat_time(&mut self, e: &Ent, millis: i64) -> cs::Result<()> {
        self.emit(json!(["setAttr", e.0, "combat-time", millis - self.millis]))?;
        self.ents[e.0]
            .attrs
            .insert("combat-time".into(), json!(millis));
        Ok(())
    }
    fn set_combat_attacker(&mut self, e: &Ent, attacker: &Ent) -> cs::Result<()> {
        self.emit(json!(["setAttr", e.0, "combat-attacker", attacker.0]))?;
        self.ents[e.0]
            .attrs
            .insert("combat-attacker".into(), json!({"entity": attacker.0}));
        Ok(())
    }
    fn set_combat_stop(&mut self, e: &Ent, tick: i32) -> cs::Result<()> {
        self.emit(json!(["setAttr", e.0, "combat-stop", tick]))?;
        self.ents[e.0]
            .attrs
            .insert("combat-stop".into(), json!(tick));
        Ok(())
    }
    fn set_fam_exp(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["setAttr", e.0, "fam-exp", true]))?;
        self.ents[e.0].attrs.insert("fam-exp".into(), json!(true));
        Ok(())
    }
    fn duel_ammo(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["attr", e.0, "duel:ammo"]))?;
        Ok(self.ents[e.0].attrs.contains_key("duel:ammo"))
    }
    fn duel_ammo_add(&mut self, e: &Ent, _drop: ()) -> cs::Result<()> {
        self.emit(json!(["attr", e.0, "duel:ammo"]))
    }
    fn index(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["index", e.0]))?;
        Ok(self.number(e, "index", e.0 as i32))
    }
    fn client_index(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["clientIndex", e.0]))?;
        Ok(self.number(e, "clientIndex", e.0 as i32))
    }
    fn is_active(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["active", e.0]))?;
        Ok(self.unless(e, "active"))
    }
    fn center_location(&mut self, e: &Ent) -> cs::Result<[i32; 3]> {
        self.emit(json!(["center", e.0]))?;
        let at = self.place(e).ok_or(Error::Null)?;
        let offset = self.number(e, "size", 1) >> 1;
        Ok(handler::location(at[0] + offset, at[1] + offset, at[2] % 4))
    }
    fn closest_occupied_tile(
        &mut self,
        e: &Ent,
        other: Option<[i32; 3]>,
    ) -> cs::Result<Option<[i32; 3]>> {
        self.emit(json!(["closestTile", e.0, other]))?;
        let at = self.place(e).ok_or(Error::Null)?;
        let size = self.number(e, "size", 1);
        let corner = handler::location(at[0] + size, at[1] + size, at[2] % 4);
        let mut occupied = vec![];
        for x in at[0]..corner[0] {
            for y in at[1]..corner[1] {
                occupied.push(handler::location(x, y, at[2]));
            }
        }
        let mut closest = at;
        if occupied.len() > 1 {
            let mut lowest = 9999.0;
            for tile in occupied {
                let dist = cs::distance(tile, other.ok_or(Error::Null)?);
                if dist < lowest {
                    lowest = dist;
                    closest = tile;
                }
            }
        }
        Ok(Some(closest))
    }
    fn swing_handler(&mut self, e: &Ent, swing: bool) -> cs::Result<usize> {
        // The dragon classes' override answers the NPC's own combat action.
        if let Some(r) = self.combat_of[e.0] {
            return Ok(r);
        }
        self.emit(json!(["swingHandler", e.0, swing]))?;
        if cs::Host::kind(self, e).is_player() {
            return Ok(
                match sa::player_swing_handler(self, e, swing, [&0, &1, &2])? {
                    sa::Chosen::Handler(r) => r,
                    sa::Chosen::Chinchompa => CHINCHOMPA,
                    sa::Chosen::Salamander => SALAMANDER,
                },
            );
        }
        let spec = self.spec(e);
        let chosen = if swing && spec.get("handlerSwing").is_some() {
            &spec["handlerSwing"]
        } else {
            &spec["handler"]
        };
        Ok(chosen.as_u64().unwrap_or(0) as usize)
    }
    fn entity_attackable(
        &mut self,
        victim: &Ent,
        attacker: &Ent,
        style: Option<CombatStyle>,
        message: bool,
    ) -> cs::Result<bool> {
        let name = style.map(CombatStyle::name);
        self.emit(json!(["attackable", victim.0, attacker.0, name, message]))?;
        Ok(self.unless(victim, "attackable"))
    }
    fn face(&mut self, e: &Ent, target: Option<&Ent>) -> cs::Result<()> {
        self.emit(json!(["face", e.0, tag(target)]))
    }
    fn face_temporary(&mut self, e: &Ent, target: Option<&Ent>, ticks: i32) -> cs::Result<()> {
        self.emit(json!(["faceTemporary", e.0, tag(target), ticks]))
    }
    fn attack_animation(&mut self, e: &Ent) -> cs::Result<Option<i32>> {
        self.emit(json!(["attackAnim", e.0]))?;
        Ok(self.animation(e, "attackAnim", 422))
    }
    fn defence_animation(&mut self, e: &Ent) -> cs::Result<Option<i32>> {
        self.emit(json!(["defenceAnim", e.0]))?;
        Ok(self.animation(e, "defenceAnim", 404))
    }
    fn combat_animation(&mut self, e: &Ent, index: i32) -> cs::Result<Option<i32>> {
        self.emit(json!(["combatAnim", e.0, index]))?;
        match self.spec(e)["combatAnims"].as_array() {
            None => Ok(None),
            Some(list) => match list.get(index as usize) {
                None => Err(Error::Index),
                Some(Value::Null) => Ok(None),
                Some(value) => Ok(Some(int(value))),
            },
        }
    }
    fn animate(&mut self, e: &Ent, animation: Option<&i32>) -> cs::Result<()> {
        self.emit(json!(["animate", e.0, animation]))
    }
    fn visualize_entity(
        &mut self,
        e: &Ent,
        animation: Option<&i32>,
        graphics: Option<Graphics>,
    ) -> cs::Result<()> {
        self.emit(json!(["visualize", e.0, animation, graphic(graphics)]))
    }
    fn entity_on_impact(
        &mut self,
        victim: &Ent,
        attacker: Option<&Ent>,
        _state: &mut BattleState<Self>,
    ) -> cs::Result<()> {
        self.emit(json!(["onImpact", victim.0, tag(attacker)]))?;
        if self.flag(victim, "retaliate") && attacker.is_some() {
            pulse::attack(self, victim, attacker)?;
        }
        Ok(())
    }
    fn send_impact(&mut self, e: &Ent, state: &mut BattleState<Self>) -> cs::Result<()> {
        self.emit(json!(["sendImpact", e.0]))?;
        self.script(e, "sendImpact", state);
        Ok(())
    }
    fn check_impact(&mut self, e: &Ent, state: &mut BattleState<Self>) -> cs::Result<()> {
        self.emit(json!(["checkImpact", e.0]))?;
        self.script(e, "checkImpact", state);
        Ok(())
    }
    fn full_restore(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["fullRestore", e.0]))
    }
    fn entity_formatted_hit(
        &mut self,
        e: &Ent,
        _state: &BattleState<Self>,
        hit: i32,
    ) -> cs::Result<f64> {
        self.emit(json!(["formattedHit", e.0, hit]))?;
        Ok(f64::from(hit) * self.double(e, "hitFactor", 1.0))
    }
    fn familiar_adjust_battle_state(
        &mut self,
        e: &Ent,
        _state: &mut BattleState<Self>,
    ) -> cs::Result<()> {
        self.emit(json!(["familiarAdjust", e.0]))
    }
    fn check_weapon_degrades(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["weaponDegrades", e.0]))
    }
    fn prayer_active(&mut self, e: &Ent, prayer: Prayer) -> cs::Result<bool> {
        self.emit(json!(["prayer", e.0, prayer.name()]))?;
        Ok(self.has(e, "prayers", prayer.name()))
    }
    fn prayer_points(&mut self, e: &Ent) -> cs::Result<f64> {
        self.emit(json!(["prayerPoints", e.0]))?;
        Ok(self.ents[e.0].prayer_points)
    }
    fn decrement_prayer_points(&mut self, e: &Ent, amount: f64) -> cs::Result<()> {
        self.emit(json!(["decPrayer", e.0, hex(amount)]))?;
        let model = &mut self.ents[e.0];
        model.prayer_points -= amount;
        if model.prayer_points < 0.0 {
            model.prayer_points = 0.0;
        }
        Ok(())
    }
    fn protect_style(&mut self, e: &Ent) -> cs::Result<Option<CombatStyle>> {
        self.emit(json!(["protectStyle", e.0]))?;
        Ok(style(&self.spec(e)["protectStyle"]))
    }
    fn degrade(&mut self, player: &Ent, victim: &Ent, weapon: bool) -> cs::Result<()> {
        self.emit(json!(["degrade", player.0, victim.0, weapon]))
    }
    fn apply_poison(&mut self, victim: &Ent, source: &Ent, severity: i32) -> cs::Result<()> {
        self.emit(json!(["poison", victim.0, source.0, severity]))
    }
    fn npc_poisonous(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["poisonous", e.0]))?;
        Ok(self.flag(e, "poisonous"))
    }
    fn npc_poison_severity(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["poisonSeverity", e.0]))?;
        Ok(self.number(e, "poisonSeverity", -1))
    }
    fn is_ironman(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["ironman", e.0]))?;
        Ok(self.flag(e, "ironman"))
    }
    fn familiar_owner(&mut self, e: &Ent) -> cs::Result<Option<Ent>> {
        self.emit(json!(["owner", e.0]))?;
        Ok(entity(&self.spec(e)["owner"]))
    }
    fn familiar_attack_style(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["famAttackStyle", e.0]))?;
        Ok(self.number(e, "famAttackStyle", 0))
    }
    fn npc_xp_multiplier(&mut self, npc: &Ent, player: &Ent) -> cs::Result<f64> {
        self.emit(json!(["xpMultiplier", npc.0, player.0]))?;
        Ok(self.double(npc, "xpMultiplier", 1.0))
    }
    fn add_experience(
        &mut self,
        e: &Ent,
        slot: i32,
        experience: f64,
        player_mod: bool,
    ) -> cs::Result<()> {
        self.emit(json!(["addXp", e.0, slot, hex(experience), player_mod]))
    }
    fn spell_id(&mut self, spell: &Spell) -> cs::Result<i32> {
        self.emit(json!(["spellId", spell.0]))?;
        Ok(self.spells[spell.0].get("spellId").map_or(1, int))
    }
    fn spell_fire_effect(
        &mut self,
        spell: &Spell,
        e: &Ent,
        victim: Option<&Ent>,
        _state: &mut BattleState<Self>,
    ) -> cs::Result<()> {
        self.emit(json!(["fireEffect", spell.0, e.0, tag(victim)]))
    }
    fn spell_visualize(&mut self, spell: &Spell, e: &Ent, victim: Option<&Ent>) -> cs::Result<()> {
        self.emit(json!(["spellVisualize", spell.0, e.0, tag(victim)]))
    }
    fn spell_visualize_impact(
        &mut self,
        spell: &Spell,
        e: Option<&Ent>,
        victim: Option<&Ent>,
        _state: &mut BattleState<Self>,
    ) -> cs::Result<()> {
        self.emit(json!(["spellImpact", spell.0, tag(e), tag(victim)]))
    }
    fn spell_add_experience(&mut self, spell: &Spell, e: &Ent, hit: i32) -> cs::Result<()> {
        self.emit(json!(["spellXp", spell.0, e.0, hit]))
    }
    fn equipment_replace(
        &mut self,
        e: &Ent,
        item: Option<(i32, i32, i32)>,
        slot: i32,
    ) -> cs::Result<()> {
        let shown = item.map(|(id, amount, charge)| json!([id, amount, charge]));
        self.emit(json!(["equipReplace", e.0, shown, slot]))?;
        let made = item.map(|(id, amount, charge)| (id, amount, charge, format!("item{id}")));
        self.store_equipment(e, slot, made);
        Ok(())
    }
    fn equipment_contains(&mut self, e: &Ent, id: i32, amount: i32) -> cs::Result<bool> {
        self.emit(json!(["contains", e.0, id, amount]))?;
        Ok(self.has(e, "contains", &id.to_string()))
    }
    fn clipping_flag(&mut self, z: i32, x: i32, y: i32) -> cs::Result<i32> {
        self.emit(json!(["clip", z, x, y]))?;
        Ok(Self::map_flag(&self.clip, z, x, y))
    }
    fn projectile_flag(&mut self, z: i32, x: i32, y: i32) -> cs::Result<i32> {
        self.emit(json!(["pclip", z, x, y]))?;
        Ok(Self::map_flag(&self.pclip, z, x, y))
    }
    fn ground_item_increase(
        &mut self,
        id: i32,
        amount: i32,
        location: [i32; 3],
        player: &Ent,
    ) -> cs::Result<Option<()>> {
        self.emit(json!(["groundIncrease", id, amount, location, player.0]))?;
        Ok((!self.flag(player, "dropNull")).then_some(()))
    }
    fn item_male_worn_model(&mut self, item: &Item) -> cs::Result<i32> {
        let reference = item.0.borrow().reference.clone();
        self.emit(json!(["wornModel", reference]))?;
        Ok(match &item.0.borrow().extras["model"] {
            Value::Null => -1,
            value => int(value),
        })
    }
    fn item_attack_audios(&mut self, item: &Item) -> cs::Result<Option<Vec<Option<i32>>>> {
        let reference = item.0.borrow().reference.clone();
        self.emit(json!(["itemAudios", reference]))?;
        Ok(item.0.borrow().extras["audios"].as_array().map(|list| {
            list.iter()
                .map(|v| if v.is_null() { None } else { Some(int(v)) })
                .collect()
        }))
    }
    fn is_moving(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["moving", e.0]))?;
        Ok(self.flag(e, "moving"))
    }
    fn is_running_both(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["runningBoth", e.0]))?;
        Ok(self.flag(e, "runningBoth"))
    }
    fn run_dir(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["runDir", e.0]))?;
        Ok(self.number(e, "runDir", -1))
    }
    fn walking_reset(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["walkReset", e.0]))
    }
    fn movement_locked(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["movementLocked", e.0]))?;
        Ok(self.flag(e, "movementLocked"))
    }
    fn interaction_locked(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["interactionLocked", e.0]))?;
        Ok(self.flag(e, "interactionLocked"))
    }
    fn close_component(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["closeComponent", e.0]))
    }
    fn open_weapon_tab(&mut self, e: &Ent, present: bool) -> cs::Result<()> {
        self.emit(json!(["openTab", e.0, present]))
    }
    fn attack(&mut self, e: &Ent, victim: &Ent) -> cs::Result<()> {
        self.emit(json!(["attack", e.0, victim.0]))?;
        pulse::attack(self, e, Some(victim))
    }
    fn update_weapon_interface(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["updateInterface", e.0]))
    }
    fn debug(&mut self, e: &Ent, text: &str) -> cs::Result<()> {
        self.emit(json!(["debug", e.0, text]))
    }
    fn weapon_interfaces(&mut self, e: &Ent) -> cs::Result<Option<WeaponInterfaces>> {
        self.emit(json!(["weaponInterfaces", e.0]))?;
        let name = self.spec(e)["weaponInterface"].as_str().unwrap();
        Ok(WeaponInterfaces::ALL.into_iter().find(|w| w.name() == name))
    }
    fn is_dead(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["isDead", e.0]))?;
        Ok(self.ents[e.0].dead)
    }
    fn region_active(&mut self, region_id: i32) -> cs::Result<bool> {
        self.emit(json!(["regionActive", region_id]))?;
        Ok(!self.inactive.contains(&i64::from(region_id)))
    }
    fn combat_time_out(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["combatTimeOut", e.0]))?;
        Ok(self.number(e, "combatTimeOut", 10))
    }
    fn attack_speed(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["attackSpeed", e.0]))?;
        Ok(self.number(e, "attackSpeed", 4))
    }
    fn ticks(&mut self) -> cs::Result<i32> {
        self.emit(json!(["ticks"]))?;
        Ok(self.tick)
    }
    fn miasmic_active(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["miasmic", e.0]))?;
        Ok(self.flag(e, "miasmic"))
    }
    fn npc_hidden(&mut self, npc: &Ent, player: &Ent) -> cs::Result<bool> {
        self.emit(json!(["hidden", npc.0, player.0]))?;
        Ok(self.flag(npc, "hidden"))
    }
    fn npc_can_start_combat(&mut self, npc: &Ent, victim: &Ent) -> cs::Result<bool> {
        self.emit(json!(["canStartCombat", npc.0, victim.0]))?;
        Ok(self.unless(npc, "canStartCombat"))
    }
    fn movement_update_path(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["movementUpdatePath", e.0]))
    }
    fn movement_set_last(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["movementSetLast", e.0, null]))
    }
    fn movement_set_destination(&mut self, e: &Ent, victim: Option<&Ent>) -> cs::Result<()> {
        self.emit(json!(["movementSetDestination", e.0, tag(victim)]))
    }
    fn interface_close(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["ifaceClose", e.0]))
    }
    fn interface_close_chatbox(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["ifaceCloseChatbox", e.0]))
    }
    fn is_wilderness(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["wilderness", e.0]))?;
        Ok(self.flag(e, "wilderness"))
    }
    fn check_skull(&mut self, e: &Ent, victim: &Ent) -> cs::Result<()> {
        self.emit(json!(["checkSkull", e.0, victim.0]))
    }
    fn pulse_manager_is_moving(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["isMovingPulse", e.0]))?;
        if let Some(given) = self.spec(e).get("movingPulse") {
            return Ok(given == true);
        }
        Ok(self.ents[e.0].registered && self.pulses[e.0].running)
    }
    fn pulse_manager_clear(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["pulseClear", e.0]))?;
        if self.ents[e.0].registered {
            pulse::stop(self, e)?;
        }
        Ok(())
    }
    fn pulse_manager_run(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["pulseRun", e.0]))?;
        self.ents[e.0].registered = true;
        pulse::start(self, e)?;
        if self.pulses[e.0].running {
            self.queue.push_back(Queued::Combat(e.0));
        }
        Ok(())
    }
    fn current_time_millis(&mut self) -> cs::Result<i64> {
        Ok(self.millis)
    }
    fn combat_level(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["combatLevel", e.0]))?;
        Ok(self.number(e, "combatLevel", 3))
    }
    fn player_send_message(&mut self, e: &Ent, text: &str) -> cs::Result<()> {
        self.emit(json!(["sendMessage", e.0, text]))
    }
    fn update_level(&mut self, e: &Ent, slot: i32, amount: i32, maximum: i32) -> cs::Result<i32> {
        self.emit(json!(["updateLevel", e.0, slot, amount, maximum]))?;
        let model = &mut self.ents[e.0];
        let answer = model.spec["updateLevelReturns"]
            .as_array()
            .and_then(|list| list.get(model.level_answer))
            .map(int);
        if answer.is_some() {
            model.level_answer += 1;
        }
        Ok(answer.unwrap_or(0))
    }
    fn on_attack(&mut self, e: &Ent, victim: &Ent) -> cs::Result<()> {
        self.emit(json!(["onAttack", e.0, victim.0]))
    }
    fn remove_weak_scripts(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["removeWeakScripts", e.0]))
    }
    fn set_armour_set(&mut self, e: &Ent, set: Option<ArmourSet>) -> cs::Result<()> {
        let name = set.map(ArmourSet::name);
        self.emit(json!(["setArmourSet", e.0, name]))?;
        self.ents[e.0].armour_set = name.map(str::to_owned);
        Ok(())
    }
    fn submit(&mut self, pulse: WorldPulse<Self>) -> cs::Result<()> {
        self.emit(json!(["submit", label(&pulse.task), pulse.delay]))?;
        self.queue.push_back(Queued::World(Box::new(pulse)));
        Ok(())
    }
    fn before_attack_finalized(
        &mut self,
        npc: &Ent,
        victim: &Ent,
        _state: &mut BattleState<Self>,
    ) -> cs::Result<()> {
        self.emit(json!(["beforeAttackFinalized", npc.0, victim.0]))
    }
    fn npc_audio(&mut self, e: &Ent, index: i32) -> cs::Result<Option<i32>> {
        self.emit(json!(["npcAudio", e.0, index]))?;
        Ok(match &self.spec(e)["audio"][index as usize] {
            Value::Null => None,
            value => Some(int(value)),
        })
    }
    fn play_hurt_audio(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["hurtAudio", e.0, 0]))
    }
    fn attack_style_index(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["attackStyleIndex", e.0]))?;
        Ok(self.number(e, "attackStyleIndex", 0))
    }
    fn npc_combat_graphics(&mut self, e: &Ent) -> cs::Result<Option<Vec<Option<Graphics>>>> {
        self.emit(json!(["combatGraphics", e.0]))?;
        Ok(match self.spec(e).get("combatGraphics") {
            None => Some(vec![None; 3]),
            Some(Value::Null) => None,
            Some(list) => Some(list.as_array().unwrap().iter().map(graphics).collect()),
        })
    }
    fn send_projectile(&mut self, request: ProjectileRequest<'_, Self>) -> cs::Result<()> {
        let row = match request {
            ProjectileRequest::AmmunitionCopy {
                ammunition_id,
                source,
                victim,
                speed,
            } => {
                let id = self.visuals[&ammunition_id].projectile_id.unwrap();
                json!(["projectile", "copy", id, source.0, tag(victim), hex(speed)])
            }
            ProjectileRequest::Create {
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                speed,
                angle,
            } => json!([
                "projectile",
                "create",
                source.0,
                tag(victim),
                id,
                start_height,
                end_height,
                start_delay,
                speed,
                angle
            ]),
            ProjectileRequest::Ranged {
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                angle,
            } => json!([
                "projectile",
                "ranged",
                source.0,
                tag(victim),
                id,
                start_height,
                end_height,
                start_delay,
                angle
            ]),
        };
        self.emit(row)
    }
    fn skills_hit(&mut self, e: &Ent, damage: i32) -> cs::Result<i32> {
        self.emit(json!(["hit", e.0, damage]))?;
        let model = &mut self.ents[e.0];
        model.lifepoints = model.lifepoints.wrapping_sub(damage);
        let mut left = 0;
        if model.lifepoints < 0 {
            left = model.lifepoints.wrapping_neg();
            model.lifepoints = 0;
        }
        Ok(left)
    }
    fn heal(&mut self, e: &Ent, amount: i32) -> cs::Result<i32> {
        self.emit(json!(["heal", e.0, amount]))?;
        let maximum = self.number(e, "maxLifepoints", 10);
        let model = &mut self.ents[e.0];
        model.lifepoints = model.lifepoints.wrapping_add(amount);
        let mut left = 0;
        if model.lifepoints > maximum {
            left = model.lifepoints - maximum;
            model.lifepoints = maximum;
        }
        Ok(left)
    }
    fn dynamic_levels(&mut self, e: &Ent) -> cs::Result<Option<Vec<i32>>> {
        self.emit(json!(["dynamicLevels", e.0]))?;
        Ok(Some(
            (0..24).map(|k| self.keyed(e, "levels", k, 1)).collect(),
        ))
    }
    fn update_run_energy(&mut self, e: &Ent, amount: f64) -> cs::Result<()> {
        self.emit(json!(["runEnergy", e.0, hex(amount)]))
    }
    fn uid(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["uid", e.0]))?;
        Ok(self.number(e, "uid", e.0 as i32))
    }
    fn has_familiar(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["hasFamiliar", e.0]))?;
        Ok(self.flag(e, "hasFamiliar"))
    }
    fn familiar_is_pet(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["familiar", e.0]))?;
        Ok(self.flag(e, "familiarPet"))
    }
    fn start_death(&mut self, e: &Ent, killer: Option<&Ent>) -> cs::Result<()> {
        self.emit(json!(["startDeath", e.0, tag(killer)]))?;
        self.ents[e.0].dead = true;
        Ok(())
    }
    fn player_by_uid(&mut self, uid: i32) -> cs::Result<Option<Ent>> {
        self.emit(json!(["playerByUid", uid]))?;
        Ok(entity(&self.uids[uid.to_string()]))
    }
    fn start_redemption(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["startRedemption", e.0]))
    }
    fn zone_type(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["zoneType", e.0]))?;
        Ok(self.number(e, "zoneType", 0))
    }
    fn ring_of_life_teleport(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["ringOfLife", e.0, 2570, 0, true, true]))?;
        Ok(self.flag(e, "ringOfLife"))
    }
    fn recoil_damage(&mut self, e: &Ent) -> cs::Result<i32> {
        self.emit(json!(["recoilDamage", e.0]))?;
        Ok(self.ents[e.0].recoil)
    }
    fn set_recoil_damage(&mut self, e: &Ent, damage: i32) -> cs::Result<()> {
        self.emit(json!(["setRecoilDamage", e.0, damage]))?;
        self.ents[e.0].recoil = damage;
        Ok(())
    }
    fn log_class_error(&mut self, origin: &str, text: &str) -> cs::Result<()> {
        self.emit(json!(["log", origin, "ERR", text]))
    }
    fn print_stack_trace(&mut self, error: &Error) -> cs::Result<()> {
        self.emit(json!(["printStackTrace", class(error)]))
    }
    fn bolt_can_fire(&mut self, _: BoltEffect, _: &BattleState<Self>) -> cs::Result<bool> {
        panic!("the corpus does not drive bolt effect overrides")
    }
    fn bolt_impact(&mut self, _: BoltEffect, _: &mut BattleState<Self>) -> cs::Result<()> {
        panic!("the corpus does not drive bolt effect overrides")
    }
}

impl settings::Context for Recorder {
    fn varp(&mut self, _: &Settings, id: i32, value: i32) -> settings::Result<()> {
        let tag = self.tag;
        self.emit(json!(["varp", tag, id, value]))
            .map_err(|_| settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!("Settings sessions are not reached")
    }
    fn send(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: rs09_protocol::game::Frame,
    ) -> settings::Result<()> {
        unreachable!("Settings frames are not reached")
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        unreachable!("run energy is not reached")
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, text: &str) -> settings::Result<()> {
        let tag = self.tag;
        self.emit(json!(["message", tag, text]))
            .map_err(|_| settings::Error::Binding)
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        unreachable!("Settings strings are not reached")
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!("Settings properties are not reached")
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!("retaliation is not reached")
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!("retaliation is not reached")
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!("ironman is not reached")
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        unreachable!("containers are not reached")
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        unreachable!("containers are not reached")
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        unreachable!("weights are not reached")
    }
}

impl sa::Host for World {
    type SettingsContext = Recorder;
    type Timer = String;

    fn special_of(&self, _: &usize) -> Option<Special> {
        None
    }
    fn settings(&mut self, player: &Ent) -> (Settings, &mut Recorder) {
        self.rec.tag = player.0;
        let settings = self.settings[player.0]
            .clone()
            .expect("a player's settings");
        (settings, &mut self.rec)
    }
    fn osrs_dragon_axe(&self) -> bool {
        false
    }
    fn new_animation(&self, id: i32, _: sa::Priority) -> i32 {
        id
    }
    fn is_multi_zone(&mut self, _: &Ent) -> cs::Result<bool> {
        unreachable!("multi zones are not reached")
    }
    fn local_entities(&mut self, _: [i32; 3], _: i32) -> cs::Result<Vec<Ent>> {
        unreachable!("local entities are not reached")
    }
    fn local_npcs(&mut self, _: [i32; 3], _: i32) -> cs::Result<Vec<Ent>> {
        unreachable!("local NPCs are not reached")
    }
    fn local_players(&mut self, _: [i32; 3], _: i32) -> cs::Result<Vec<Ent>> {
        unreachable!("local players are not reached")
    }
    fn wilderness_continue_attack(
        &mut self,
        _: &Ent,
        _: &Ent,
        _: CombatStyle,
        _: bool,
    ) -> cs::Result<bool> {
        unreachable!("the wilderness is not reached")
    }
    fn find_npc(&mut self, _: [i32; 3]) -> cs::Result<Option<Ent>> {
        unreachable!("findNPC is not reached")
    }
    fn original_id(&mut self, _: &Ent) -> cs::Result<i32> {
        unreachable!("original ids are not reached")
    }
    fn set_prayer_points(&mut self, _: &Ent, _: f64) -> cs::Result<()> {
        unreachable!("prayer points are not set")
    }
    fn increment_prayer_points(&mut self, _: &Ent, _: f64) -> cs::Result<()> {
        unreachable!("prayer points are not incremented")
    }
    fn api_message(&mut self, player: &Ent, text: &str) -> cs::Result<()> {
        self.emit(json!(["apiMessage", player.0, text]))
    }
    fn send_chat(&mut self, _: &Ent, _: &str) -> cs::Result<()> {
        unreachable!("chat is not reached")
    }
    fn play_audio(&mut self, player: &Ent, id: i32, delay: i32) -> cs::Result<()> {
        self.emit(json!(["playAudio", player.0, id, delay, 1, null, 8]))
    }
    fn play_global_audio_delayed(&mut self, at: [i32; 3], id: i32, delay: i32) -> cs::Result<()> {
        self.emit(json!(["audio", at, id, delay, 1, 8]))
    }
    fn spawn_timer(
        &mut self,
        identifier: &str,
        args: &[sa::TimerArg],
    ) -> cs::Result<Option<String>> {
        let shown: Vec<Value> = args
            .iter()
            .map(|a| match a {
                sa::TimerArg::Int(n) => json!(n),
                sa::TimerArg::Bool(b) => json!(b),
            })
            .collect();
        self.emit(json!(["spawnTimer", identifier, shown]))?;
        let refused = self.case["nullTimers"]
            .as_array()
            .is_some_and(|list| list.iter().any(|v| v == identifier));
        Ok((!refused).then(|| identifier.to_owned()))
    }
    fn register_timer(&mut self, e: &Ent, timer: String) -> cs::Result<()> {
        self.emit(json!([
            "registerTimer",
            e.0,
            "spawned",
            timer,
            0,
            false,
            false,
            []
        ]))
    }
    fn register_special_timer(&mut self, _: &Ent, _: sa::SpecialTimer<Self>) -> cs::Result<()> {
        unreachable!("special timers are not reached")
    }
    fn rights(&mut self, _: &Ent) -> cs::Result<Option<sa::Rights>> {
        unreachable!("rights are not reached")
    }
    fn prayer_on(&mut self, player: &Ent, prayer: &'static str) -> cs::Result<bool> {
        self.emit(json!(["prayer", player.0, prayer]))?;
        Ok(self.has(player, "prayers", prayer))
    }
    fn prayer_toggle(&mut self, _: &Ent, _: &'static str) -> cs::Result<bool> {
        unreachable!("prayers are not toggled")
    }
    fn continue_attack(&mut self, _: &Ent, _: &Ent, _: CombatStyle, _: bool) -> cs::Result<bool> {
        unreachable!("continueAttack is not reached")
    }
    fn combat_stop(&mut self, _: &Ent) -> cs::Result<i32> {
        unreachable!("combat-stop is not read")
    }
    fn lock_movement(&mut self, _: &Ent, _: i32) -> cs::Result<()> {
        unreachable!("movement locks are not reached")
    }
    fn set_stun_clock(&mut self, _: &Ent, _: i32) -> cs::Result<()> {
        unreachable!("stuns are not reached")
    }
    fn dumb_path(&mut self, _: &Ent, _: [i32; 3]) -> cs::Result<bool> {
        unreachable!("dumb paths are not reached")
    }
    fn add_path(&mut self, _: &Ent, _: i32, _: i32) -> cs::Result<()> {
        unreachable!("walking paths are not reached")
    }
    fn definition_name(&mut self, _: i32) -> cs::Result<String> {
        unreachable!("definition names are not reached")
    }
    fn chain_victims(&mut self, _: &Ent) -> cs::Result<Option<Vec<Ent>>> {
        unreachable!("chain hits are not reached")
    }
    fn set_chain_victims(&mut self, _: &Ent, _: Vec<Ent>) -> cs::Result<()> {
        unreachable!("chain hits are not reached")
    }
    fn chain_victims_mut(&mut self, _: &Ent) -> Option<&mut Vec<Ent>> {
        unreachable!("chain hits are not reached")
    }
    fn submit_special(&mut self, _: sa::SpecialPulse<Self>) -> cs::Result<()> {
        unreachable!("special pulses are not reached")
    }
    fn send_special_projectile(&mut self, _: sa::SpecialProjectile<'_, Self>) -> cs::Result<()> {
        unreachable!("special projectiles are not reached")
    }
}

fn template_row(t: &ProjectileTemplate<World>) -> Vec<Value> {
    vec![
        tag(t.source.as_ref()),
        tag(t.victim.as_ref()),
        json!(t.id),
        json!(t.start_height),
        json!(t.end_height),
        json!(t.start_delay),
        json!(t.speed),
        json!(t.angle),
        json!(t.distance),
    ]
}

impl df::Host for World {
    fn subclass(&self, handler: &usize) -> Option<&Subclass<Self>> {
        self.subclasses[*handler].as_ref()
    }
    fn subclass_mut(&mut self, handler: &usize) -> Option<&mut Subclass<Self>> {
        self.subclasses[*handler].as_mut()
    }
    fn style_handler(&self, style: CombatStyle) -> usize {
        match style {
            CombatStyle::Melee => 0,
            CombatStyle::Range => 1,
            CombatStyle::Magic => 2,
        }
    }
    fn new_handler(&mut self, base: Handler, subclass: Subclass<Self>) -> usize {
        let name = match &subclass {
            Subclass::Dragonfire(d) if d.shield.is_some() => "",
            Subclass::Dragonfire(_) => "DragonfireSwingHandler",
            Subclass::Multi(_) => "MultiSwingHandler",
        };
        self.handlers.push(base);
        self.subclasses.push(Some(subclass));
        self.names.push(name);
        self.specials.push(Specials::default());
        self.handlers.len() - 1
    }
    fn set_item_charge(&mut self, item: &Item, charge: i32) {
        let mut data = item.0.borrow_mut();
        data.id_hash = (((data.id_hash >> 16) & 0xFFFF) << 16) | charge;
    }
    fn item_slot(&self, item: &Item) -> i32 {
        item.0.borrow().index
    }
    fn better_dfs(&self) -> bool {
        self.case["betterDfs"] == true
    }
    fn timer_active(&mut self, e: &Ent, identifier: &str) -> cs::Result<bool> {
        self.emit(json!(["timerActive", e.0, identifier]))?;
        Ok(self.has(e, "timers", identifier))
    }
    fn dragonfire_immunity(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["timerActive", e.0, "DragonFireImmunity"]))?;
        Ok(self.has(e, "timers", "DragonFireImmunity"))
    }
    fn npc_dragonfire_protection(&mut self, e: &Ent, fire: bool) -> cs::Result<i32> {
        // The dragon classes override it with every token.
        if self.dragon_of[e.0].is_some() {
            return Ok(dragons::PROTECTION);
        }
        self.emit(json!(["dragonfireProtection", e.0, fire]))?;
        Ok(self.number(e, "dragonfireProtection", 0))
    }
    fn has_fire_resistance(&mut self, e: &Ent) -> cs::Result<bool> {
        self.emit(json!(["fireResistance", e.0]))?;
        Ok(self.flag(e, "fireResistance"))
    }
    fn set_attribute_value(&mut self, e: &Ent, key: &str, value: df::Value) -> cs::Result<()> {
        let shown = match value {
            df::Value::Int(n) => json!(n),
            df::Value::Bool(b) => json!(b),
        };
        self.emit(json!(["setAttr", e.0, key, shown]))?;
        self.ents[e.0].attrs.insert(key.to_owned(), shown);
        Ok(())
    }
    fn update_bonuses(&mut self, player: &Ent) -> cs::Result<()> {
        self.emit(json!(["updateBonuses", player.0]))
    }
    fn face_location(&mut self, e: &Ent, at: Option<[i32; 3]>) -> cs::Result<()> {
        self.emit(json!(["faceLocation", e.0, at]))
    }
    fn send_dragon_projectile(&mut self, request: DragonProjectile<'_, Self>) -> cs::Result<()> {
        let mut row = vec![json!("projectile")];
        match request {
            DragonProjectile::Copy {
                template,
                source,
                victim,
                speed,
            } => {
                row.push(json!("copyTemplate"));
                row.extend(template_row(template));
                row.extend([json!(source.0), tag(victim), json!(hex(speed))]);
            }
            DragonProjectile::Transform {
                template,
                source,
                victim,
                npc,
                base_speed,
                modified_speed,
            } => {
                row.push(json!("transform"));
                row.extend(template_row(template));
                row.extend([
                    json!(source.0),
                    tag(victim),
                    json!(npc),
                    json!(base_speed),
                    json!(modified_speed),
                ]);
            }
        }
        self.emit(Value::Array(row))
    }
    fn set_varp(&mut self, player: &Ent, index: i32, value: i32) -> cs::Result<()> {
        self.emit(json!(["setVarp", player.0, index, value, false]))
    }
    fn is_locked(&mut self, e: &Ent, key: &str) -> cs::Result<bool> {
        self.emit(json!(["isLocked", e.0, key]))?;
        Ok(self.has(e, "locked", key))
    }
    fn lock_key(&mut self, e: &Ent, key: &str, ticks: i32) -> cs::Result<()> {
        self.emit(json!(["lockKey", e.0, key, ticks]))
    }
    fn delay_entity(&mut self, e: &Ent, ticks: i32) -> cs::Result<()> {
        self.emit(json!(["delayEntity", e.0, ticks]))
    }
    fn replace_slot(
        &mut self,
        player: &Ent,
        slot: i32,
        item: df::ItemValue,
        current: Option<df::ItemValue>,
        container: df::Container,
    ) -> cs::Result<()> {
        let (id, amount, charge) = item;
        let current = current.map(|(i, a, c)| json!([i, a, c]));
        self.emit(json!([
            "replaceSlot",
            player.0,
            slot,
            [id, amount, charge],
            current,
            container.name()
        ]))?;
        let name = format!("item{id}");
        match container {
            df::Container::Equipment => {
                self.store_equipment(player, slot, Some((id, amount, charge, name)));
            }
            df::Container::Inventory => {
                let made = Item::new(
                    format!("inv:{}:{slot}", player.0),
                    id,
                    amount,
                    charge,
                    name,
                    None,
                    Value::Null,
                    slot,
                );
                let held = &mut self.ents[player.0].inventory;
                held.emptied.remove(&slot);
                held.items.insert(slot, made);
            }
        }
        Ok(())
    }
    fn inventory_item(&mut self, player: &Ent, slot: i32) -> cs::Result<Option<Item>> {
        self.emit(json!(["invGet", player.0, slot]))?;
        if !(0..28).contains(&slot) {
            return Ok(None);
        }
        Ok(self.inventory_at(player, slot))
    }
    fn drop_item(&mut self, player: &Ent, item: &Item) -> cs::Result<bool> {
        let row = json!([
            "drop",
            player.0,
            item.0.borrow().reference,
            item.id(),
            item.0.borrow().amount,
            item.charge()
        ]);
        self.emit(row)?;
        Ok(true)
    }
    fn register_item_plugin(&mut self, id: i32) -> cs::Result<()> {
        self.emit(json!(["itemPlugin", id]))
    }
    fn listen(&mut self, ids: &[i32], option: &str) -> cs::Result<()> {
        self.emit(json!(["on", ids, 0, [option]]))
    }
}

fn label(task: &Task<World>) -> &'static str {
    match task {
        Task::Impact { .. } => "impact",
        Task::SecondHit { .. } => "secondHit",
        Task::ManualHit { .. } => "manualHit",
    }
}

fn state(spec: &Value) -> BattleState<World> {
    let mut b = if spec["ctor"] == false {
        BattleState::default()
    } else {
        BattleState::new(entity(&spec["attacker"]), entity(&spec["victim"]))
    };
    if let Some(v) = spec.get("estimatedHit") {
        b.estimated_hit = int(v);
    }
    if let Some(v) = spec.get("maximumHit") {
        b.maximum_hit = int(v);
    }
    if let Some(v) = spec.get("secondaryHit") {
        b.secondary_hit = int(v);
    }
    if let Some(v) = spec.get("style").filter(|v| !v.is_null()) {
        b.style = style(v);
    }
    if let Some(v) = spec.get("spell").filter(|v| !v.is_null()) {
        b.spell = Some(Spell(v.as_u64().unwrap() as usize));
    }
    if let Some(list) = spec.get("targets") {
        b.targets = Some(
            list.as_array()
                .unwrap()
                .iter()
                .map(|target| match target {
                    Value::Null => None,
                    Value::String(_) => Some(Target::This),
                    nested => Some(Target::State(Box::new(state(nested)))),
                })
                .collect(),
        );
    }
    b
}

fn dump(b: &BattleState<World>) -> Value {
    let targets = b.targets.as_ref().map(|list| {
        list.iter()
            .map(|target| match target {
                None => Value::Null,
                Some(Target::This) => json!("this"),
                Some(Target::State(nested)) => dump(nested),
            })
            .collect::<Vec<_>>()
    });
    json!({
        "attacker": tag(b.entity.as_ref()), "victim": tag(b.victim.as_ref()),
        "estimatedHit": b.estimated_hit, "maximumHit": b.maximum_hit, "secondaryHit": b.secondary_hit,
        "targets": targets, "style": b.style.map(CombatStyle::name),
        "armourEffect": b.armour_effect.map(ArmourSet::name),
    })
}

fn shown(impact: Option<&Impact<World>>) -> Value {
    impact.map_or(Value::Null, |i| {
        json!([
            tag(i.source.as_ref()),
            i.amount,
            i.style.map(CombatStyle::name),
            i.kind.map(HitsplatType::name)
        ])
    })
}

/// A handler as the snapshot shows it: its index, or "dfs" for the shield's
/// anonymous handler, which no list holds.
fn handler_label(world: &World, r: Option<usize>) -> Value {
    match r {
        None => Value::Null,
        Some(r) if r >= world.listed => json!("dfs"),
        Some(r) => json!(r),
    }
}

/// Everything the ported classes own, after an operation.
fn snapshot(world: &World) -> Value {
    let pulses: Vec<Value> = world
        .pulses
        .iter()
        .map(|p| {
            json!([
                tag(p.victim.as_ref()),
                tag(p.last_victim.as_ref()),
                p.next_attack,
                p.combat_time_out,
                p.style.name(),
                p.running,
                p.delay,
                p.ticks_passed,
                handler_label(world, p.temporary_handler),
                p.checks.iter().map(|c| tag(c.as_ref())).collect::<Vec<_>>()
            ])
        })
        .collect();
    let impacts: Vec<Value> = world
        .impacts
        .iter()
        .map(|i| {
            let queue: Vec<Value> = i.queue.iter().map(|x| shown(Some(x))).collect();
            let npc: Vec<Value> = i
                .npc_order()
                .unwrap()
                .into_iter()
                .map(|at| json!([tag(i.npc_log[at].0.as_ref()), i.npc_log[at].2]))
                .collect();
            let players: Vec<Value> = i
                .player_order()
                .into_iter()
                .map(|uid| {
                    let amount = i.player_log.iter().find(|e| e.0 == uid).unwrap().1;
                    json!([uid, amount])
                })
                .collect();
            json!([i.disabled_ticks, queue, npc, players])
        })
        .collect();
    let queue: Vec<Value> = world
        .queue
        .iter()
        .map(|entry| match entry {
            Queued::Combat(e) => {
                let p = &world.pulses[*e];
                json!([
                    format!("combat:{e}"),
                    p.running,
                    p.delay,
                    p.ticks_passed,
                    null
                ])
            }
            Queued::World(p) => {
                let flag = match &p.task {
                    Task::Impact { impact, .. } => json!(impact),
                    _ => Value::Null,
                };
                json!([label(&p.task), p.running, p.delay, p.ticks_passed, flag])
            }
        })
        .collect();
    let settings: Vec<Value> = world
        .settings
        .iter()
        .map(|s| match s {
            Some(s) => {
                let f = s.fields.read();
                json!([f.special_energy, f.special_toggled])
            }
            None => Value::Null,
        })
        .collect();
    let items: Vec<Value> = world
        .ents
        .iter()
        .map(|m| {
            if m.spec["kind"].as_str().unwrap_or("player") == "player" {
                json!([m.equipment.shown(), m.inventory.shown()])
            } else {
                Value::Null
            }
        })
        .collect();
    let types: Vec<Value> = world.handlers[..world.listed]
        .iter()
        .map(|h| json!(h.style.map(CombatStyle::name)))
        .collect();
    let multis: Vec<Value> = (0..world.listed)
        .filter_map(|r| match &world.subclasses[r] {
            Some(Subclass::Multi(m)) => Some(json!([r, m.current, m.next, m.melee_distance])),
            _ => None,
        })
        .collect();
    let attrs: Vec<Value> = world
        .ents
        .iter()
        .map(|m| {
            json!(
                ["fireBreath", "dfs_spec", "fire_resistance"]
                    .iter()
                    .map(|k| m.attrs.get(*k).cloned().unwrap_or(Value::Null))
                    .collect::<Vec<_>>()
            )
        })
        .collect();
    json!({
        "pulses": pulses, "impacts": impacts, "queue": queue, "settings": settings,
        "items": items, "types": types, "multi": multis, "attrs": attrs, "xpMod": hex(world.xp_mod),
    })
}

/// The modelled runner: a fixed prefix of the queue per tick, unfinished
/// running pulses back at the tail.
fn ticks(world: &mut World, count: i32) {
    for _ in 0..count {
        let now = world.tick;
        world.mark(json!(["tick", now]));
        for _ in 0..world.queue.len() {
            let mut entry = world.queue.pop_front().unwrap();
            let (name, outcome) = match &mut entry {
                Queued::Combat(e) => (format!("combat:{e}"), pulse::update(world, &Ent(*e))),
                Queued::World(p) => (label(&p.task).to_owned(), p.update(world)),
            };
            match outcome {
                Ok(done) => {
                    world.mark(json!(["update", name, done]));
                    let running = match &entry {
                        Queued::Combat(e) => world.pulses[*e].running,
                        Queued::World(p) => p.running,
                    };
                    if !done && running {
                        world.queue.push_back(entry);
                    }
                }
                Err(error) => world.mark(json!(["runnerError", name, class(&error)])),
            }
        }
        world.tick += 1;
    }
}

/// `swing(entity, victim, state)` of a handler instance, as a direct call.
fn handler_swing(
    world: &mut World,
    r: usize,
    e: Option<&Ent>,
    v: Option<&Ent>,
    state: Option<&mut BattleState<World>>,
) -> cs::Result<i32> {
    let Some(state) = state else {
        // Only the dragonfire handlers are swung with a null state.
        return df::handler::swing(world, &r, e, v, None);
    };
    if let Some(member) = cp::Host::overrides(world, &r).and_then(|o| o.swing) {
        return member(world, &r, e, v, state);
    }
    let mut instance = world.handlers[r].clone();
    let delay = instance.swing(world, e, v, state);
    world.handlers[r] = instance;
    delay
}

fn held(world: &mut World, op: &Value) -> Option<Item> {
    let e = entity(&op["e"]).unwrap();
    let slot = int(&op["slot"]);
    if op["where"] == "inventory" {
        world.inventory_at(&e, slot)
    } else {
        world.peek(&e, slot)
    }
}

fn show_item(item: df::ItemValue) -> Value {
    let (id, amount, charge) = item;
    json!(["new", id, amount, charge, format!("item{id}"), -1])
}

fn run(
    world: &mut World,
    listener: &shield::Listener,
    op: &Value,
    current: &mut Option<BattleState<World>>,
) -> cs::Result<Value> {
    let e = entity(&op["e"]);
    let v = entity(&op["v"]);
    let h = op["h"].as_u64().unwrap_or(0) as usize;
    let need = |x: &Option<Ent>| x.clone().ok_or(Error::Null);
    let name = op["op"].as_str().unwrap();
    let given = |current: &mut Option<BattleState<World>>| {
        *current = if op["state"] == "none" || op["state"] == "null" {
            None
        } else {
            Some(state(&op["state"]))
        };
    };
    Ok(match name {
        "swing" => json!(pulse::swing(world, &need(&e)?, v.as_ref(), &h)?),
        "handlerSwing" => {
            given(current);
            json!(handler_swing(
                world,
                h,
                e.as_ref(),
                v.as_ref(),
                current.as_mut()
            )?)
        }
        "visualize" => {
            given(current);
            let state = current.as_ref().ok_or(Error::Null)?;
            handler::visualize(world, &h, &need(&e)?, v.as_ref(), state)?;
            Value::Null
        }
        "visualizeImpact" => {
            given(current);
            let state = current.as_mut().ok_or(Error::Null)?;
            handler::visualize_impact(world, &h, e.as_ref(), v.as_ref(), state)?;
            Value::Null
        }
        "impact" => {
            given(current);
            let state = current.as_mut().ok_or(Error::Null)?;
            handler::impact(world, &h, e.as_ref(), v.as_ref(), state)?;
            Value::Null
        }
        "adjust" => {
            given(current);
            let state = current.as_mut().ok_or(Error::Null)?;
            handler::adjust_battle_state(world, &h, &need(&e)?, &need(&v)?, state)?;
            Value::Null
        }
        "formattedHit" => {
            given(current);
            let state = current.as_ref().ok_or(Error::Null)?;
            let hit = int(&op["hit"]);
            json!(handler::formatted_hit(
                world,
                &h,
                &need(&e)?,
                &need(&v)?,
                state,
                hit
            )?)
        }
        "canSwing" => json!(handler::can_swing(world, &h, &need(&e)?, &need(&v)?)?.name()),
        "accuracy" => json!(handler::calculate_accuracy(world, &h, e.as_ref())?),
        "hit" => json!(handler::calculate_hit(
            world,
            &h,
            e.as_ref(),
            v.as_ref(),
            bits(&op["modifier"])
        )?),
        "defence" => json!(handler::calculate_defence(
            world,
            &h,
            e.as_ref(),
            v.as_ref()
        )?),
        "setMultiplier" => json!(hex(handler::set_multiplier(
            world,
            &h,
            e.as_ref(),
            int(&op["skill"])
        )?)),
        "addExperience" => {
            given(current);
            handler::add_experience(world, &h, e.as_ref(), v.as_ref(), current.as_ref())?;
            Value::Null
        }
        "postSwing" => {
            given(current);
            let state = current.as_ref().ok_or(Error::Null)?;
            handler::post_swing(world, &h, e.as_ref(), v.as_ref(), state)?;
            Value::Null
        }
        "armourSet" => json!(handler::armour_set(world, &h, e.as_ref())?.map(ArmourSet::name)),
        "maxHit" => json!(api::calculate_dragonfire_max_hit(
            world,
            &need(&e)?,
            int(&op["max"]),
            op["wyvern"] == true,
            int(&op["unprotectable"]),
            op["message"] == true
        )?),
        "shieldProtection" => json!(api::has_dragonfire_shield_protection(
            world,
            &need(&e)?,
            op["wyvern"] == true
        )?),
        "protection" => {
            let target = need(&e)?;
            let fire = op["fire"] == true;
            if cs::Host::kind(world, &target).is_player() {
                json!(api::player_dragonfire_protection(world, &target, fire)?)
            } else {
                json!(df::Host::npc_dragonfire_protection(world, &target, fire)?)
            }
        }
        "fireType" => {
            let kind = FireType::ALL
                .into_iter()
                .find(|t| op["type"] == t.name())
                .unwrap();
            let others: Vec<Option<Ent>> = op["others"]
                .as_array()
                .map(|l| l.iter().map(entity).collect())
                .unwrap_or_default();
            let (animation, _) = kind.animation();
            let projectile = kind.projectile_id();
            let done = kind.exec(world, &need(&e)?, &others)?;
            json!([animation, projectile, done])
        }
        "listen" => {
            listener.define_listeners(world)?;
            Value::Null
        }
        "option" => {
            let item = held(world, op).expect("a held item");
            let player = need(&e)?;
            json!(listener.handle(world, op["option"].as_str().unwrap(), &player, &item)?)
        }
        "dragonInfo" => {
            let at = need(&e)?.0;
            let class = world.dragon_of[at].expect("a dragon");
            let r = world.combat_of[at];
            json!([
                class.ids(),
                dragons::PROTECTION,
                dragons::PROTECTION,
                handler_label(world, r),
                handler_label(world, r)
            ])
        }
        "dropTable" => {
            let mut drops: Vec<df::ItemValue> = op["drops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| (int(id), 1, 1000))
                .collect();
            if let Some(rate) = op.get("eggRate") {
                world.egg_rate = int(rate);
            }
            let rate = world.egg_rate;
            dragons::on_drop_table_rolled(world, &need(&e)?, &need(&v)?, &mut drops, rate)?;
            let shown: Vec<Value> = drops.iter().map(|(i, a, c)| json!([i, a, c])).collect();
            json!([shown, dragons::behaviour::ids()])
        }
        "deathItem" => {
            // The plugin is handed the held item and ignores it.
            let _ = held(world, op);
            show_item(shield::death_item())
        }
        "itemPlugin" => {
            shield::item_plugin_new_instance(world)?;
            Value::Null
        }
        "attack" => {
            pulse::attack(world, &need(&e)?, v.as_ref())?;
            Value::Null
        }
        "pulse" => json!(pulse::pulse(world, &need(&e)?)?),
        "setVictim" => {
            pulse::set_victim(world, &need(&e)?, v.as_ref())?;
            Value::Null
        }
        "setStyle" => {
            world.pulses[need(&e)?.0].style = style(&op["style"]).unwrap();
            Value::Null
        }
        "tick" => {
            ticks(world, int(&op["n"]));
            Value::Null
        }
        "setTemp" => {
            let chosen = op["h"].as_u64().map(|h| h as usize);
            world.pulses[need(&e)?.0].temporary_handler = chosen;
            Value::Null
        }
        "set" => {
            let key = op["key"].as_str().unwrap();
            let value = op["value"].clone();
            let at = need(&e)?.0;
            let model = &mut world.ents[at];
            let spec = model.spec.as_object_mut().unwrap();
            if value.is_null() {
                spec.remove(key);
            } else {
                spec.insert(key.to_owned(), value.clone());
            }
            match key {
                "lifepoints" => model.lifepoints = int(&value),
                "dead" => model.dead = value == true,
                "specialToggled" => {
                    world.settings[at]
                        .as_ref()
                        .unwrap()
                        .fields
                        .write()
                        .special_toggled = value == true
                }
                _ => {}
            }
            Value::Null
        }
        "setTick" => {
            world.tick = int(&op["tick"]);
            Value::Null
        }
        other => panic!("unknown operation {other}"),
    })
}

fn make(spec: &Value) -> Handler {
    let flags: Vec<cs::Flag> = spec["flags"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|name| {
                    cs::Flag::ALL
                        .into_iter()
                        .find(|f| name == f.name())
                        .unwrap()
                })
                .collect()
        })
        .unwrap_or_default();
    match spec["kind"].as_str().unwrap() {
        "melee" => Handler::new(HandlerKind::Melee, &flags),
        "range" => Handler::new(HandlerKind::Range, &flags),
        "magic" => Handler::new(HandlerKind::Magic, &flags),
        other => panic!("unknown handler {other}"),
    }
}

fn template(spec: &Value) -> Option<ProjectileTemplate<World>> {
    let p = spec.as_array()?;
    Some(ProjectileTemplate::create(
        entity(&p[0]),
        entity(&p[1]),
        int(&p[2]),
        int(&p[3]),
        int(&p[4]),
        int(&p[5]),
        int(&p[6]),
        int(&p[7]),
        int(&p[8]),
    ))
}

fn animation_of(spec: &Value) -> Option<i32> {
    spec.as_i64().map(|n| n as i32)
}

/// One SwitchAttack of a case: a get's attack, the style constructor, or the
/// full constructor.
fn attack_of(
    world: &World,
    made: &HashMap<usize, SwitchAttack<World>>,
    a: &Value,
) -> SwitchAttack<World> {
    if let Some(from) = a.get("from") {
        return made[&(from.as_u64().unwrap() as usize)].clone();
    }
    if let Some(name) = a.get("style") {
        return df::style_attack(world, style(name).unwrap());
    }
    let handler = a["handler"].as_u64().map(|h| h as usize);
    let mut s = SwitchAttack::new(
        handler,
        animation_of(&a["animation"]),
        graphics(&a["start"]),
        graphics(&a["end"]),
        template(&a["projectile"]),
    );
    if let Some(v) = a.get("useHandler") {
        s.use_handler = v == true;
    }
    if let Some(v) = a.get("maximumHit") {
        s.maximum_hit = int(v);
    }
    s
}

fn add_handler(world: &mut World, made: &mut HashMap<usize, SwitchAttack<World>>, h: &Value) {
    match h["kind"].as_str().unwrap() {
        "dragonfire" => {
            let melee = h["meleeRange"] == true;
            let max = int(&h["maximumHit"]);
            let fire = h["fire"] != false;
            if h["via"] == "new" {
                let attack = h
                    .get("attack")
                    .filter(|a| !a.is_null())
                    .map(|a| attack_of(world, made, a));
                df::handler::new(world, melee, max, attack, fire);
                return;
            }
            let at = world.handlers.len();
            // The six-argument get is fiery.
            let fire = if h["via"] == "get6" { true } else { fire };
            let attack = df::handler::get(
                world,
                melee,
                max,
                animation_of(&h["animation"]),
                graphics(&h["start"]),
                graphics(&h["end"]),
                template(&h["projectile"]),
                fire,
            );
            made.insert(at, attack);
        }
        "multi" => {
            let attacks: Vec<SwitchAttack<World>> = h["attacks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| attack_of(world, made, a))
                .collect();
            let melee_distance = h.get("meleeDistance").is_none_or(|v| v == true);
            multi::new(world, melee_distance, attacks).unwrap();
        }
        _ => {
            world.handlers.push(make(h));
            world.subclasses.push(None);
            world.names.push("");
            world.specials.push(Specials::default());
        }
    }
}

fn evaluate(case: &Value) -> Value {
    let empty = vec![];
    let entities = case["entities"].as_array().unwrap_or(&empty);
    let mut handlers = vec![
        Handler::new(HandlerKind::Melee, &[]),
        Handler::new(HandlerKind::Range, &[]),
        Handler::new(HandlerKind::Magic, &[]),
    ];
    let mut names = vec![
        "MeleeSwingHandler",
        "RangeSwingHandler",
        "MagicSwingHandler",
    ];
    for mixed in multi::Mixed::ALL {
        handlers.push(mixed.instance());
        names.push(mixed.class_name());
    }
    handlers.push(Handler::new(HandlerKind::Range, &[]));
    handlers.push(Handler::new(HandlerKind::Range, &[]));
    names.extend(["ChinchompaSwingHandler", "SalamanderSwingHandler"]);
    let index = |spec: &Value, key: &str| spec[key].as_u64().map(|i| i as usize);
    let rng = &case["rng"];
    let ents: Vec<Model> = entities
        .iter()
        .map(|spec| Model {
            spec: spec.clone(),
            lifepoints: spec.get("lifepoints").map_or(10, int),
            recoil: spec.get("recoilDamage").map_or(40, int),
            prayer_points: spec.get("prayerPoints").map_or(0.0, bits),
            dead: spec["dead"] == true,
            registered: false,
            armour_set: spec["armourSet"].as_str().map(str::to_owned),
            attrs: spec["attrs"]
                .as_object()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
            equipment: Held::default(),
            inventory: Held::default(),
            level_answer: 0,
        })
        .collect();
    let settings = entities
        .iter()
        .map(|spec| {
            (spec["kind"].as_str().unwrap_or("player") == "player").then(|| {
                let s = Settings::default();
                if let Some(energy) = spec.get("specialEnergy") {
                    s.fields.write().special_energy = int(energy);
                }
                s.fields.write().special_toggled = spec["specialToggled"] == true;
                s
            })
        })
        .collect();
    let mut pulses: Vec<PulseState<World>> = (0..ents.len())
        .map(|at| PulseState::new(Ent(at), 0))
        .collect();
    for (at, spec) in entities.iter().enumerate() {
        pulses[at].last_victim = entity(&spec["lastVictim"]);
    }
    let count = handlers.len();
    let mut world = World {
        spells: case["spells"].as_array().cloned().unwrap_or_default(),
        spell: entities.iter().map(|s| index(s, "spell")).collect(),
        autocast: entities.iter().map(|s| index(s, "autocast")).collect(),
        range: RangeData::default(),
        visuals: HashMap::new(),
        weapon_animation: HashMap::new(),
        specials: (0..count).map(|_| Specials::default()).collect(),
        subclasses: (0..count).map(|_| None).collect(),
        handlers,
        names,
        listed: 0,
        pulses,
        impacts: ents.iter().map(|_| ImpactState::default()).collect(),
        ents,
        xp_mod: 4.0,
        queue: VecDeque::new(),
        tick: case.get("tick").map_or(0, int),
        millis: 0,
        clip: case["clip"].clone(),
        pclip: case["pclip"].clone(),
        uids: case["uids"].clone(),
        inactive: vec![],
        rec: Recorder {
            events: vec![],
            calls: 0,
            fail_at: -1,
            tag: 0,
        },
        math_script: rng["math"].as_array().cloned(),
        int_script: rng["ints"].as_array().cloned(),
        math_at: 0,
        int_at: 0,
        math: JavaRandom::new(rng["mathSeed"].as_i64().unwrap_or(0)),
        ints: JavaRandom::new(rng["intSeed"].as_i64().unwrap_or(0)),
        settings,
        case: case.clone(),
        dragon_of: entities
            .iter()
            .map(|s| match s["class"].as_str() {
                Some("chromatic") => Some(DragonClass::Chromatic),
                Some("metal") => Some(DragonClass::Metal),
                _ => None,
            })
            .collect(),
        combat_of: vec![None; entities.len()],
        egg_rate: dragons::behaviour::EGG_RATE,
    };
    // Each dragon class's static dragonfire (made with the class), then each
    // dragon's own combat action.
    let mut class_fire: HashMap<DragonClass, SwitchAttack<World>> = HashMap::new();
    for at in 0..entities.len() {
        let Some(class) = world.dragon_of[at] else {
            continue;
        };
        let fire = match class_fire.get(&class) {
            Some(fire) => fire.clone(),
            None => {
                let fire = dragons::dragonfire(&mut world, class);
                class_fire.insert(class, fire.clone());
                fire
            }
        };
        world.combat_of[at] = Some(dragons::combat_action(&mut world, class, &fire).unwrap());
    }
    let mut made = HashMap::new();
    for spec in case["handlers"].as_array().unwrap_or(&empty) {
        add_handler(&mut world, &mut made, spec);
    }
    world.listed = world.handlers.len();
    let listener = shield::Listener::new(&world);
    let mut rows = vec![];
    let mut last = None;
    for op in case["ops"].as_array().unwrap() {
        world.rec.events.clear();
        world.rec.calls = 0;
        world.rec.fail_at = op.get("failAt").and_then(Value::as_i64).unwrap_or(-1);
        let mut current = None;
        let outcome = run(&mut world, &listener, op, &mut current);
        world.rec.fail_at = -1;
        let mut row = Map::new();
        let (result, error) = match outcome {
            Ok(result) => (result, Value::Null),
            Err(error) => (Value::Null, json!(class(&error))),
        };
        row.insert("result".into(), result);
        row.insert("error".into(), error);
        row.insert("events".into(), json!(world.rec.events));
        if let Some(state) = &current {
            row.insert("state".into(), dump(state));
        }
        let owned = snapshot(&world);
        if last.as_ref() != Some(&owned) {
            row.insert("world".into(), owned.clone());
            last = Some(owned);
        }
        rows.push(Value::Object(row));
    }
    Value::Array(rows)
}

#[test]
fn original_dragonfire_decisions_services_and_state_match() {
    let mut operations = 0;
    let mut failures: Vec<String> = vec![];
    let mut kinds = HashSet::new();
    for (n, line) in FIXTURE.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = evaluate(&row["input"]);
        let expected = row["expected"].as_array().unwrap();
        let actual = actual.as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "dragonfire case {n}");
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            if a != e {
                let (ae, ee) = (
                    a["events"].as_array().unwrap(),
                    e["events"].as_array().unwrap(),
                );
                let at = ae.iter().zip(ee).position(|(x, y)| x != y);
                let at = at.unwrap_or(ae.len().min(ee.len()));
                let op = &row["input"]["ops"][i];
                if !kinds.insert(format!("{}/{}", op["op"], op["h"])) || failures.len() >= 12 {
                    operations += 1;
                    continue;
                }
                failures.push(format!(
                    "dragonfire case {n} ({}) operation {i} {}: first differing event {at}: native {:?} original {:?}; native result {} error {} original result {} error {}; native state {} original state {}; native world {} original world {}",
                    row["input"]["group"],
                    row["input"]["ops"][i],
                    ae.get(at),
                    ee.get(at),
                    a["result"],
                    a["error"],
                    e["result"],
                    e["error"],
                    a["state"],
                    e["state"],
                    a["world"],
                    e["world"]
                ));
            }
            operations += 1;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["operationCount"], operations);
}

#[test]
fn original_sources_and_fixture_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], FIXTURE.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(FIXTURE))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in provenance["sources"].as_object().unwrap() {
        let Ok(bytes) = std::fs::read(root.join(name)) else {
            assert!(name.starts_with("reference/"), "{name} is missing");
            continue;
        };
        let bytes: Vec<u8> = bytes.into_iter().filter(|b| *b != b'\r').collect();
        assert_eq!(
            hash,
            &json!(format!("{:x}", Sha256::digest(&bytes))),
            "{name}"
        );
    }
}
