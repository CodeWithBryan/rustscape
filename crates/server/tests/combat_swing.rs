//! Replays the original combat-swing corpus against the native port. The host
//! below answers every required service from the case input and records the
//! same ordered events the JVM stand-ins recorded.
use rs09_game_core::random::JavaRandom;
use rs09_server::world::combat_swing::{
    self as cs, Ammunition, ArmourSet, AttackStyle, BattleState, BoltEffect, CombatStyle, Error,
    Flag, Graphics, Handler, HandlerKind, InteractionType, Kind, Perk, RangeData, RangeWeapon,
    SlayerTask, Target, WeaponInterfaces, WeaponType,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const FIXTURE: &str = include_str!("../../../fixtures/combat-swing/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/combat-swing/provenance.json");

#[derive(Clone)]
struct Ent(usize);

#[derive(Clone)]
struct Item {
    reference: String,
    id: i32,
    amount: i32,
    charge: i32,
    name: String,
    bonuses: Option<Vec<i32>>,
}

#[derive(Clone)]
struct Spell(usize);

struct World<'a> {
    entities: &'a [Value],
    spells: &'a [Value],
    spell: Vec<Option<usize>>,
    autocast: Vec<Option<usize>>,
    range: RangeData,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    quiet: bool,
    forced: Option<f64>,
    math_script: Option<&'a Vec<Value>>,
    int_script: Option<&'a Vec<Value>>,
    math_at: usize,
    int_at: usize,
    math: JavaRandom,
    ints: JavaRandom,
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

impl World<'_> {
    fn emit(&mut self, row: Value) -> cs::Result<()> {
        if self.quiet {
            return Ok(());
        }
        self.events.push(row);
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }

    fn spec(&self, e: &Ent) -> &Value {
        &self.entities[e.0]
    }

    fn number(&self, e: &Ent, key: &str, fallback: i32) -> i32 {
        match &self.spec(e)[key] {
            Value::Null => fallback,
            value => int(value),
        }
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

    fn worn(&self, e: &Ent, slot: i32) -> Option<Item> {
        let row = self.spec(e)["equipment"][slot.to_string()].as_array()?;
        Some(Item {
            reference: format!("{}:{slot}", e.0),
            id: int(&row[0]),
            amount: int(&row[1]),
            charge: int(&row[2]),
            name: row[3].as_str().unwrap().to_owned(),
            bonuses: row.get(4).filter(|v| !v.is_null()).map(ints),
        })
    }
}

impl cs::Types for World<'_> {
    type Entity = Ent;
    type Item = Item;
    type Spell = Spell;
}

impl cs::Host for World<'_> {
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
        item.id
    }
    fn item_name(&self, item: &Item) -> String {
        item.name.clone()
    }
    fn item_amount(&self, item: &Item) -> i32 {
        item.amount
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
        Ok(self.spec(e)["armourSet"].as_str().map(|name| {
            ArmourSet::ALL
                .into_iter()
                .find(|s| s.name() == name)
                .unwrap()
        }))
    }
    fn pulse_victim(&mut self, e: &Ent) -> cs::Result<Option<Ent>> {
        self.emit(json!(["pulseVictim", e.0]))?;
        Ok(self.spec(e)["pulseVictim"]
            .as_u64()
            .map(|i| Ent(i as usize)))
    }
    fn stop_combat_pulse(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["stopPulse", e.0]))
    }
    fn update_combat_style(&mut self, e: &Ent) -> cs::Result<()> {
        self.emit(json!(["updateStyle", e.0]))
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
        Ok(self.number(e, "lifepoints", 10))
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
        Ok(match self.spec(e).get("location") {
            None => Some([3200, 3200, 0]),
            Some(Value::Null) => None,
            Some(at) => Some([int(&at[0]), int(&at[1]), int(&at[2])]),
        })
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
        Ok(self.worn(e, slot))
    }
    fn equipment_new(&mut self, e: &Ent, slot: i32) -> cs::Result<Item> {
        self.emit(json!(["equipNew", e.0, slot]))?;
        if !(0..14).contains(&slot) {
            return Err(Error::Index);
        }
        Ok(self.worn(e, slot).unwrap_or(Item {
            reference: "new".into(),
            id: 0,
            amount: 1,
            charge: 1000,
            name: "item0".into(),
            bonuses: None,
        }))
    }
    fn equipment_contains_one(&mut self, e: &Ent, id: i32) -> cs::Result<bool> {
        self.emit(json!(["containsOne", e.0, id]))?;
        Ok(self.has(e, "contains", &id.to_string()))
    }
    fn item_bonuses(&mut self, item: &Item) -> cs::Result<Option<Vec<i32>>> {
        self.emit(json!(["itemConfig", item.reference, "bonuses"]))?;
        Ok(item.bonuses.clone())
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
        adjust_attack_style: bool,
        forget_autocast: bool,
    ) -> cs::Result<()> {
        self.emit(json!([
            "selectAutoSpell",
            e.0,
            button,
            adjust_attack_style,
            forget_autocast
        ]))
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
        Ok(match self.spells[spell.0].get("targets") {
            None => Some(vec![Some(BattleState::new(
                Some(entity.clone()),
                victim.cloned(),
            ))]),
            Some(Value::Null) => None,
            Some(list) => Some(
                list.as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        row.as_array().map(|row| {
                            let target = Ent(row[0].as_u64().unwrap() as usize);
                            BattleState::new(Some(entity.clone()), Some(target))
                        })
                    })
                    .collect(),
            ),
        })
    }
    fn spell_is_blitz(&mut self, spell: &Spell) -> cs::Result<bool> {
        self.emit(json!(["spellType", spell.0]))?;
        Ok(self.spells[spell.0]["type"] == "BLITZ")
    }
    fn math_random(&mut self) -> cs::Result<f64> {
        let value = if let Some(forced) = self.forced {
            forced
        } else if let Some(script) = self.math_script {
            self.math_at += 1;
            bits(&script[self.math_at - 1])
        } else {
            self.math.next_double()
        };
        self.emit(json!(["mathRandom", hex(value)]))?;
        Ok(value)
    }
    fn next_int(&mut self, bound: i32) -> cs::Result<i32> {
        let value = if let Some(script) = self.int_script {
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

fn style(value: &Value) -> Option<CombatStyle> {
    let name = value.as_str()?;
    CombatStyle::ALL.into_iter().find(|s| s.name() == name)
}

fn entity(value: &Value) -> Option<Ent> {
    value.as_u64().map(|i| Ent(i as usize))
}

fn state<'a>(spec: &Value) -> BattleState<World<'a>> {
    let mut b = if spec["ctor"] == false {
        BattleState::default()
    } else {
        BattleState::new(entity(&spec["attacker"]), entity(&spec["victim"]))
    };
    let field = |name: &str| spec.get(name).filter(|v| !v.is_null());
    if let Some(v) = spec.get("estimatedHit") {
        b.estimated_hit = int(v);
    }
    if let Some(v) = spec.get("maximumHit") {
        b.maximum_hit = int(v);
    }
    if let Some(v) = spec.get("secondaryHit") {
        b.secondary_hit = int(v);
    }
    if let Some(v) = spec.get("recoilDamage") {
        b.recoil_damage = int(v);
    }
    if let Some(v) = spec.get("poisonDamage") {
        b.poison_damage = int(v);
    }
    if let Some(v) = spec.get("frozen") {
        b.frozen = v == true;
    }
    if let Some(v) = field("armourEffect") {
        b.armour_effect = ArmourSet::ALL.into_iter().find(|s| v == s.name());
    }
    if let Some(v) = field("style") {
        b.style = style(v);
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

fn item(item: Option<&Item>) -> Value {
    item.map_or(Value::Null, |i| {
        json!([i.reference, i.id, i.amount, i.charge, i.name])
    })
}

fn dump(b: &BattleState<World<'_>>) -> Value {
    let targets = b.targets.as_ref().map(|list| {
        list.iter()
            .map(|target| match target {
                None => Value::Null,
                Some(Target::This) => json!("this"),
                Some(Target::State(nested)) => dump(nested),
            })
            .collect::<Vec<_>>()
    });
    let weapon = b.weapon.as_ref().map(|w| {
        json!({
            "id": w.id, "name": w.name, "item": item(w.item.as_ref()),
            "ammunitionSlot": w.ammunition_slot, "ammunition": item(w.ammunition.as_ref()),
            "type": w.kind.map(WeaponType::name),
        })
    });
    json!({
        "attacker": tag(b.entity.as_ref()), "victim": tag(b.victim.as_ref()),
        "estimatedHit": b.estimated_hit, "maximumHit": b.maximum_hit, "secondaryHit": b.secondary_hit,
        "recoilDamage": b.recoil_damage, "poisonDamage": b.poison_damage, "frozen": b.frozen,
        "targets": targets, "weapon": weapon,
        "rangeWeapon": b.range_weapon.as_ref().map(|w| w.item_id),
        "ammunition": b.ammunition.as_ref().map(|a| a.item_id),
        "spell": b.spell.as_ref().map(|s| s.0), "style": b.style.map(CombatStyle::name),
        "armourEffect": b.armour_effect.map(ArmourSet::name), "totalDamage": b.total_damage(),
    })
}

fn graphic(g: Graphics) -> Value {
    json!([g.id, g.height, g.delay])
}

fn tables() -> Value {
    let styles = |w: WeaponInterfaces| {
        w.attack_styles()
            .iter()
            .map(|s| json!([s.style, s.bonus_type]))
            .collect::<Vec<_>>()
    };
    let interfaces = WeaponInterfaces::ALL.map(|w| json!([w.name(), w.interface_id(), styles(w)]));
    let bolts =
        BoltEffect::ALL.map(|b| json!([b.name(), b.item_id(), graphic(b.graphics()), b.sound()]));
    let ids = [
        -1, 0, 9235, 9236, 9237, 9238, 9239, 9240, 9241, 9242, 9243, 9244, 9245, 9246,
    ];
    let lookups = ids.map(|id| json!([id, BoltEffect::for_id(id).map(BoltEffect::name)]));
    let sets = ArmourSet::ALL.map(|a| json!([a.name(), a.end_graphic().map(graphic), a.sets()]));
    let weapon_types = WeaponType::ALL.map(WeaponType::name);
    let flags = Flag::ALL.map(Flag::name);
    let styles = CombatStyle::ALL.map(|s| json!([s.name(), s.protection_prayer(), s.name()]));
    let interactions = InteractionType::ALL.map(InteractionType::name);
    json!({
        "weaponInterfaces": interfaces, "boltEffects": bolts, "boltLookups": lookups,
        "armourSets": sets, "weaponTypes": weapon_types, "flags": flags,
        "combatStyles": styles, "interactionTypes": interactions,
    })
}

/// The native probability, reported the way the JVM bisection reports the
/// original decision's threshold: the least draw that is not accurate.
fn threshold(
    handler: &Handler,
    world: &mut World<'_>,
    op: &Value,
) -> cs::Result<(String, Option<f64>)> {
    world.quiet = true;
    let chance = handler.hit_chance(
        world,
        entity(&op["e"]).as_ref(),
        entity(&op["v"]).as_ref(),
        style(&op["style"]),
        bits(&op["accMod"]),
        bits(&op["defMod"]),
    );
    world.quiet = false;
    Ok(match chance? {
        None => ("nodraw".into(), None),
        Some(c) if c.is_nan() || c == f64::NEG_INFINITY => ("never".into(), None),
        Some(c) => {
            // The sign of a zero probability is unobservable: -0.0 sorts first.
            let c = if c == 0.0 { -0.0 } else { c };
            (hex(c), Some(c))
        }
    })
}

fn run<'a>(
    handler: &mut Handler,
    world: &mut World<'a>,
    op: &Value,
    current: &mut Option<BattleState<World<'a>>>,
) -> cs::Result<Value> {
    let e = entity(&op["e"]);
    let v = entity(&op["v"]);
    let (e, v) = (e.as_ref(), v.as_ref());
    Ok(match op["op"].as_str().unwrap() {
        "accuracy" => json!(handler.calculate_accuracy(world, e)?),
        "defence" => json!(handler.calculate_defence(world, v, entity(&op["a"]).as_ref())?),
        "hit" => json!(handler.calculate_hit(world, e, v, bits(&op["mod"]))?),
        "setMultiplier" => json!(hex(handler.set_multiplier(world, e, int(&op["skill"]))?)),
        "armourSet" => json!(handler.armour_set(world, e)?.map(ArmourSet::name)),
        "accurate" => json!(if op.get("style").is_none() {
            handler.is_accurate_impact_typed(world, e, v)?
        } else if op.get("accMod").is_none() {
            handler.is_accurate_impact(world, e, v, style(&op["style"]), 1.0, 1.0)?
        } else {
            let (a, d) = (bits(&op["accMod"]), bits(&op["defMod"]));
            handler.is_accurate_impact(world, e, v, style(&op["style"]), a, d)?
        }),
        "threshold" => json!(threshold(handler, world, op)?.0),
        "boundary" => {
            let (text, at) = threshold(handler, world, op)?;
            let mut out = json!({"threshold": text});
            if let Some(at) = at {
                let draws = [at.next_down(), at, at.next_up()];
                let mut results = vec![];
                for draw in draws {
                    world.forced = Some(draw);
                    let (a, d) = (bits(&op["accMod"]), bits(&op["defMod"]));
                    let hit = handler.is_accurate_impact(world, e, v, style(&op["style"]), a, d);
                    world.forced = None;
                    results.push(hit?);
                }
                out["draws"] = json!(draws.map(hex));
                out["results"] = json!(results);
            }
            out
        }
        "setType" => {
            handler.style = style(&op["style"]);
            Value::Null
        }
        "swing" => {
            let slot = current.insert(state(&op["state"]));
            json!(handler.swing(world, e, v, slot)?)
        }
        "combatDistance" => json!(handler.combat_distance(world, e, v, int(&op["raw"]))?),
        "formatHit" => json!(handler.format_hit(world, v, int(&op["raw"]))?),
        "dragonfire" => json!(cs::dragonfire_message(
            int(&op["protection"]),
            op["name"].as_str().unwrap()
        )),
        "total" => json!(current.insert(state(&op["state"])).total_damage()),
        "neutralize" => {
            current.insert(state(&op["state"])).neutralize_hits();
            Value::Null
        }
        "tables" => tables(),
        other => panic!("unknown operation {other}"),
    })
}

fn evaluate(case: &Value) -> Value {
    let empty = vec![];
    let entities = case["entities"].as_array().unwrap();
    let spells = case["spells"].as_array().unwrap_or(&empty);
    let mut range = RangeData::default();
    for w in case["range"]["weapons"].as_array().unwrap_or(&empty) {
        range.weapons.insert(
            int(&w["itemId"]),
            Arc::new(RangeWeapon {
                item_id: int(&w["itemId"]),
                attack_speed: w.get("attackSpeed").map_or(4, int),
                ammunition_slot: int(&w["slot"]),
                weapon_type: int(&w["type"]),
                drop_ammo: w.get("dropAmmo") != Some(&Value::Bool(false)),
                ammunition: ints(&w["ammunition"]),
            }),
        );
    }
    for a in case["range"]["ammo"].as_array().unwrap_or(&empty) {
        let id = int(&a["itemId"]);
        range.ammunition.insert(
            id,
            Arc::new(Ammunition {
                item_id: id,
                poison_damage: a.get("poisonDamage").map_or(0, int),
                effect: BoltEffect::for_id(id),
            }),
        );
    }
    let index = |spec: &Value, key: &str| spec[key].as_u64().map(|i| i as usize);
    let rng = &case["rng"];
    let mut world = World {
        entities,
        spells,
        spell: entities.iter().map(|s| index(s, "spell")).collect(),
        autocast: entities.iter().map(|s| index(s, "autocast")).collect(),
        range,
        events: vec![],
        calls: 0,
        fail_at: -1,
        quiet: false,
        forced: None,
        math_script: rng["math"].as_array(),
        int_script: rng["ints"].as_array(),
        math_at: 0,
        int_at: 0,
        math: JavaRandom::new(rng["mathSeed"].as_i64().unwrap_or(0)),
        ints: JavaRandom::new(rng["intSeed"].as_i64().unwrap_or(0)),
    };
    let flags: Vec<Flag> = case["handler"]["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| Flag::ALL.into_iter().find(|f| name == f.name()).unwrap())
        .collect();
    let kind = match case["handler"]["kind"].as_str().unwrap() {
        "melee" => HandlerKind::Melee,
        "range" => HandlerKind::Range,
        _ => HandlerKind::Magic,
    };
    let mut handler = Handler::new(kind, &flags);
    let mut rows = vec![];
    for op in case["ops"].as_array().unwrap() {
        world.events.clear();
        world.calls = 0;
        world.fail_at = op.get("failAt").and_then(Value::as_i64).unwrap_or(-1);
        let mut current = None;
        let outcome = run(&mut handler, &mut world, op, &mut current);
        world.fail_at = -1;
        let mut row = Map::new();
        let (result, error) = match outcome {
            Ok(result) => (result, Value::Null),
            Err(Error::Null) => (Value::Null, json!("Null")),
            Err(Error::Index) => (Value::Null, json!("Index")),
            Err(Error::Binding(_)) => (Value::Null, json!("Binding")),
        };
        row.insert("result".into(), result);
        row.insert("error".into(), error);
        row.insert("events".into(), json!(world.events));
        if op["op"] == "swing" {
            let names: Vec<_> = handler.flags.iter().map(|f| f.name()).collect();
            row.insert("flags".into(), json!(names));
        }
        if op["op"] == "setType" {
            row.insert("type".into(), json!(handler.style.map(CombatStyle::name)));
        }
        if let Some(state) = &current {
            row.insert("state".into(), dump(state));
        }
        rows.push(Value::Object(row));
    }
    Value::Array(rows)
}

#[test]
fn original_combat_swing_calculations_draws_and_state_match() {
    let mut operations = 0;
    for (n, line) in FIXTURE.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = evaluate(&row["input"]);
        let expected = row["expected"].as_array().unwrap();
        let actual = actual.as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "combat swing case {n}");
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                a, e,
                "combat swing case {n} operation {i}: {} of {}",
                row["input"]["ops"][i], row["input"]
            );
            operations += 1;
        }
    }
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["operationCount"], operations);
}

#[test]
fn original_sources_and_fixture_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], 4461);
    assert_eq!(provenance["caseCount"], FIXTURE.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(FIXTURE))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}

/// The flag `RangeSwingHandler.swing` adds around its armour-piercing maximum
/// stays in the handler when that calculation fails, as in the original.
#[test]
fn failed_armour_pierce_leaves_its_flag_in_the_handler() {
    let leaked = FIXTURE
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| {
            row["input"]["handler"]["flags"]
                .as_array()
                .unwrap()
                .is_empty()
                && row["expected"][0]["error"] == "Binding"
                && row["expected"][0]["flags"] == json!(["IGNORE_STAT_REDUCTION"])
        })
        .count();
    assert!(leaked > 0, "the corpus must exercise the leaked flag");
}
