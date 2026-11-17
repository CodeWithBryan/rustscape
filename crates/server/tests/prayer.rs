//! Replays the prayer corpus (fixtures/prayer) through `world::prayer`.
//! Prayer points, static levels and healing run on the native
//! `player::skills::Skills`, varps on the native `player::variables`
//! algorithm; every other service answers from the case the way the
//! oracle's recording adapters do.
use rs09_game_core::random::JavaRandom;
use rs09_server::{
    player::{
        skills::{self, Skills},
        variables::{self, Saved, Values},
    },
    world::{
        combat_pulse::HitsplatType,
        combat_swing::{ArmourSet, BattleState, CombatStyle, Error, Kind, Result, Types},
        doors::Location,
        player_walk::{NodeHandle, State},
        prayer::{
            self, Host, Perk, PrayerEvent, PrayerState, PrayerType, ProjectileRequest, combat,
        },
    },
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/prayer/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/prayer/provenance.json");

fn hex(v: f64) -> String {
    format!("{:x}", v.to_bits())
}

fn unhex(v: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
}

fn style(v: &Value) -> Option<CombatStyle> {
    v.as_str()
        .map(|s| *CombatStyle::ALL.iter().find(|c| c.name() == s).unwrap())
}

fn kind_name(t: PrayerType) -> Value {
    json!(t.name())
}

struct Ctx {
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    current: usize,
    sponge: bool,
}

impl Ctx {
    fn emit(&mut self, event: Value) -> bool {
        self.events.push(event);
        self.calls += 1;
        self.calls != self.fail_at
    }

    fn call(&mut self, event: Value) -> Result<()> {
        if self.emit(event) {
            Ok(())
        } else {
            Err(Error::Binding("injected".into()))
        }
    }
}

impl skills::Context for Ctx {
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
        let event = json!(["perk", self.current, "DAMAGE_SPONG"]);
        if !self.emit(event) {
            return Err(skills::Error::Binding);
        }
        Ok(self.sponge)
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
        unreachable!()
    }
    fn effect(&mut self, _: &mut Skills, effect: skills::Effect) -> skills::Result<()> {
        match effect {
            skills::Effect::Packet(slot) => {
                let event = json!(["skill_packet", self.current, slot]);
                if self.emit(event) {
                    Ok(())
                } else {
                    Err(skills::Error::Binding)
                }
            }
            // Skills::new's initial combat level; not a recorded call.
            skills::Effect::Combat(_) => Ok(()),
            other => unreachable!("{other:?}"),
        }
    }
}

struct Varps<'a> {
    values: Values,
    saved: Saved,
    ctx: &'a mut Ctx,
    player: usize,
}

impl variables::Context for Varps<'_> {
    type Dispatch = ();
    fn values(&mut self, _: &NodeHandle) -> variables::Result<Option<Values>> {
        Ok(Some(self.values.clone()))
    }
    fn saved(&mut self, _: &NodeHandle) -> variables::Result<Option<Saved>> {
        Ok(Some(self.saved.clone()))
    }
    fn dispatch(&mut self, _: &NodeHandle) -> variables::Result<()> {
        Ok(())
    }
    fn send_varp(&mut self, _: (), id: i32, value: i32) -> variables::Result<()> {
        if self.ctx.emit(json!(["send_varp", self.player, id, value])) {
            Ok(())
        } else {
            Err(variables::Error::Binding("injected".into()))
        }
    }
    fn send_varc(&mut self, _: (), _: i16, _: i32) -> variables::Result<()> {
        unreachable!()
    }
    fn definition(
        &mut self,
        _: i32,
    ) -> variables::Result<Option<rs09_server::definitions::varbit::Handle>> {
        unreachable!()
    }
    fn missing_definition(&mut self, _: i32) -> variables::Result<()> {
        unreachable!()
    }
}

struct Ent {
    npc: bool,
    location: Option<[i32; 3]>,
    prayer: PrayerState,
    skills: Skills,
    bonuses: Option<Vec<i32>>,
    protect: Option<CombatStyle>,
    head_icon: i32,
    varbits: HashMap<i32, i32>,
    flags: Map<String, Value>,
    values: Values,
    saved: Saved,
    node: NodeHandle,
}

impl Ent {
    fn flag(&self, key: &str, fallback: bool) -> bool {
        match self.flags.get(key) {
            None | Some(Value::Null) => fallback,
            Some(v) => v == &Value::Bool(true),
        }
    }

    fn perk(&self, name: &str) -> bool {
        self.flags
            .get("perks")
            .and_then(Value::as_array)
            .is_some_and(|p| p.iter().any(|v| v == name))
    }

    fn apply(&mut self, fields: &Map<String, Value>) {
        for (key, v) in fields {
            match key.as_str() {
                "kind" => {}
                "location" => {
                    self.location = v.as_array().map(|a| {
                        prayer::location(
                            a[0].as_i64().unwrap() as i32,
                            a[1].as_i64().unwrap() as i32,
                            a[2].as_i64().unwrap() as i32,
                        )
                    })
                }
                "static" => {
                    for (slot, level) in v.as_object().unwrap() {
                        self.skills.static_levels[slot.parse::<usize>().unwrap()] =
                            level.as_i64().unwrap() as i32;
                    }
                }
                "points" => self.skills.prayer_points = unhex(v),
                "lifepoints" => self.skills.lifepoints = v.as_i64().unwrap() as i32,
                "bonuses" => {
                    self.bonuses = v.as_array().map(|a| {
                        a.iter()
                            .map(|n| n.as_i64().unwrap() as i32)
                            .collect::<Vec<_>>()
                    })
                }
                "protect" => self.protect = style(v),
                "headIcon" => self.head_icon = v.as_i64().unwrap() as i32,
                "varbit3909" => {
                    self.varbits.insert(3909, v.as_i64().unwrap() as i32);
                }
                _ => {
                    self.flags.insert(key.clone(), v.clone());
                }
            }
        }
    }

    fn snapshot(&self) -> Value {
        if self.npc {
            return json!({"npc": true});
        }
        let varps: BTreeMap<i32, Option<i32>> = self.values.snapshot().into_iter().collect();
        let saved: BTreeMap<i32, Option<bool>> = self.saved.snapshot().into_iter().collect();
        json!({
            "active": self.prayer.active.iter().map(|t| t.name()).collect::<Vec<_>>(),
            "ticks": self.prayer.prayer_active_ticks,
            "points": hex(self.skills.prayer_points),
            "lifepoints": self.skills.lifepoints,
            "lifepointsUpdate": self.skills.lifepoints_update,
            "headIcon": self.head_icon,
            "varps": varps.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
            "saved": saved.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
            "varbit3909": self.varbits.get(&3909).copied().unwrap_or(0),
        })
    }
}

struct World {
    ents: Vec<Ent>,
    ctx: Ctx,
    rng: JavaRandom,
    pulses: Vec<usize>,
    input: Value,
}

impl World {
    fn locals(&self, key: &str, npc: bool) -> Vec<usize> {
        match self.input.get(key).and_then(Value::as_array) {
            Some(list) => list.iter().map(|v| v.as_u64().unwrap() as usize).collect(),
            None => (0..self.ents.len())
                .filter(|&i| self.ents[i].npc == npc)
                .collect(),
        }
    }
}

impl Host for World {
    type Entity = usize;
    type SkillsContext = Ctx;

    fn same_entity(&self, a: &usize, b: &usize) -> bool {
        a == b
    }
    fn is_npc(&self, e: &usize) -> bool {
        self.ents[*e].npc
    }
    fn prayer(&mut self, p: &usize) -> &mut PrayerState {
        &mut self.ents[*p].prayer
    }
    fn skills(&mut self, p: &usize) -> (&mut Skills, &mut Ctx) {
        self.ctx.current = *p;
        self.ctx.sponge = self.ents[*p].perk("DAMAGE_SPONG");
        (&mut self.ents[*p].skills, &mut self.ctx)
    }
    fn set_varp(&mut self, p: &usize, id: i32, value: i32, save: bool) -> Result<()> {
        let e = &self.ents[*p];
        let node = e.node.clone();
        let mut varps = Varps {
            values: e.values.clone(),
            saved: e.saved.clone(),
            ctx: &mut self.ctx,
            player: *p,
        };
        variables::set_varp(&mut varps, Some(&node), id, value, save)
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn get_varbit(&mut self, p: &usize, id: i32) -> Result<i32> {
        self.ctx.call(json!(["get_varbit", p, id]))?;
        Ok(self.ents[*p].varbits.get(&id).copied().unwrap_or(0))
    }
    fn set_varbit(&mut self, p: &usize, id: i32, value: i32, save: bool) -> Result<()> {
        self.ctx.call(json!(["set_varbit", p, id, value, save]))?;
        self.ents[*p].varbits.insert(id, value);
        Ok(())
    }
    fn play_audio(&mut self, p: &usize, id: i32) -> Result<()> {
        self.ctx.call(json!(["play_audio", p, id, 0, 1, null, 8]))
    }
    fn dispatch(&mut self, p: &usize, event: PrayerEvent) -> Result<()> {
        let (class, t) = match event {
            PrayerEvent::Activated(t) => ("PrayerActivatedEvent", t),
            PrayerEvent::Deactivated(t) => ("PrayerDeactivatedEvent", t),
        };
        self.ctx.call(json!(["dispatch", p, class, t.name()]))
    }
    fn head_icon(&mut self, p: &usize) -> Result<i32> {
        self.ctx.call(json!(["head_icon", p]))?;
        Ok(self.ents[*p].head_icon)
    }
    fn set_head_icon(&mut self, p: &usize, icon: i32) -> Result<()> {
        self.ctx.call(json!(["set_head_icon", p, icon]))?;
        self.ents[*p].head_icon = icon;
        Ok(())
    }
    fn sync_appearance(&mut self, p: &usize) -> Result<()> {
        self.ctx.call(json!(["sync", p]))
    }
    fn send_dialogue(&mut self, p: &usize, text: &str) -> Result<()> {
        self.ctx.call(json!(["send_dialogue", p, text]))
    }
    fn has_kings_ransom(&mut self, p: &usize) -> Result<bool> {
        self.ctx
            .call(json!(["has_requirement", p, "KINGS_RANSOM", true]))?;
        Ok(self.ents[*p].flag("kingsRansom", false))
    }
    fn submit_head_icon_pulse(&mut self, p: &usize, delay: i32) -> Result<()> {
        self.ctx.call(json!(["submit", delay]))?;
        self.pulses.push(*p);
        Ok(())
    }
    fn send_message(&mut self, p: &usize, text: &str) -> Result<()> {
        self.ctx.call(json!(["send_message", p, text]))
    }
    fn bonuses(&mut self, p: &usize) -> Result<Option<Vec<i32>>> {
        self.ctx.call(json!(["bonuses", p]))?;
        Ok(self.ents[*p].bonuses.clone())
    }
    fn perk_active(&mut self, p: &usize, perk: Perk) -> Result<bool> {
        let Perk::DivineFavor = perk;
        self.ctx.call(json!(["perk", p, "DIVINE_FAVOR"]))?;
        Ok(self.ents[*p].perk("DIVINE_FAVOR"))
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        let v = self.rng.next_bound(bound).unwrap();
        self.ctx.call(json!(["next_int", bound, v]))?;
        Ok(v)
    }
    fn graphics(&mut self, p: &usize, id: i32) -> Result<()> {
        self.ctx.call(json!(["graphics", p, id]))
    }
    fn location(&mut self, e: &usize) -> Result<Option<[i32; 3]>> {
        self.ctx.call(json!(["location", e]))?;
        Ok(self.ents[*e].location)
    }
    fn send_projectile(&mut self, r: ProjectileRequest) -> Result<()> {
        self.ctx.call(json!([
            "projectile",
            r.start,
            r.destination,
            r.id,
            r.start_height,
            r.end_height,
            r.start_delay,
            r.speed,
            r.angle,
            r.distance
        ]))
    }
    fn manual_hit(
        &mut self,
        owner: &usize,
        source: &usize,
        hit: i32,
        kind: HitsplatType,
    ) -> Result<()> {
        self.ctx
            .call(json!(["manual_hit", owner, source, hit, kind.name()]))
    }
    fn is_multi_zone(&mut self, p: &usize) -> Result<bool> {
        self.ctx.call(json!(["multi_zone", p]))?;
        Ok(self.ents[*p].flag("multi", false))
    }
    fn local_npcs(&mut self, location: [i32; 3], distance: i32) -> Result<Vec<usize>> {
        self.ctx.call(json!(["local_npcs", location, distance]))?;
        Ok(self.locals("localNpcs", true))
    }
    fn local_players(&mut self, location: [i32; 3], distance: i32) -> Result<Vec<usize>> {
        self.ctx
            .call(json!(["local_players", location, distance]))?;
        Ok(self.locals("localPlayers", false))
    }
    fn is_attackable(
        &mut self,
        e: &usize,
        attacker: &usize,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        self.ctx
            .call(json!(["is_attackable", e, attacker, style.name(), message]))?;
        Ok(self.ents[*e].flag("attackable", true))
    }
    fn is_playing(&mut self, p: &usize) -> Result<bool> {
        self.ctx.call(json!(["is_playing", p]))?;
        Ok(self.ents[*p].flag("playing", true))
    }
    fn entity_commence_death(&mut self, p: &usize, killer: Option<&usize>) -> Result<()> {
        self.ctx.call(json!(["commence_death_super", p, killer]))
    }
    fn set_special_energy(&mut self, p: &usize, value: i32) -> Result<()> {
        self.ctx.call(json!(["set_special_energy", p, value]))
    }
    fn update_run_energy(&mut self, p: &usize, amount: f64) -> Result<()> {
        self.ctx.call(json!(["update_run_energy", p, amount]))
    }
    fn entity_full_restore(&mut self, p: &usize) -> Result<()> {
        self.ctx.call(json!(["full_restore_super", p]))
    }
}

struct T;
impl Types for T {
    type Entity = usize;
    type Item = ();
    type Spell = ();
}

fn prayer_type(v: &Value) -> PrayerType {
    PrayerType::by_name(v.as_str().unwrap()).unwrap()
}

fn names(list: Result<Vec<Option<PrayerType>>>) -> Result<Value> {
    Ok(json!(
        list?
            .into_iter()
            .map(|t| t.map(kind_name).unwrap_or(Value::Null))
            .collect::<Vec<_>>()
    ))
}

fn table() -> Value {
    let rows: Vec<Value> = PrayerType::ALL
        .iter()
        .map(|&t| {
            let r = t.row();
            json!({
                "name": t.name(),
                "ordinal": t.ordinal(),
                "level": r.level,
                "drain": r.drain,
                "config": r.config,
                "button": r.button,
                "restriction": r.restriction.name(),
                "secondRestriction": r.second_restriction.map(|c| c.name()),
                "sound": [r.sound, 0, 1, 8],
                "defenceReq": r.defence_req,
                "bonuses": r.bonuses.iter().map(|b| json!([b.skill, hex(b.bonus), 0])).collect::<Vec<_>>(),
            })
        })
        .collect();
    let styles: Vec<Value> = CombatStyle::ALL
        .iter()
        .map(|&s| json!([s.name(), PrayerType::protection(s).name()]))
        .collect();
    json!([rows, styles, prayer::PRAYER_TAB])
}

fn run(w: &mut World, op: &Value) -> Result<Value> {
    let p = op.get("p").and_then(Value::as_u64).unwrap_or(0) as usize;
    let entity = |key: &str| op[key].as_u64().map(|v| v as usize);
    let int = |key: &str| op[key].as_i64().unwrap() as i32;
    Ok(match op["op"].as_str().unwrap() {
        "toggle" => json!(prayer::toggle(w, &p, prayer_type(&op["type"]))?),
        "button" => json!(prayer::tab_button(w, &p, int("button"))?),
        "type_toggle" => json!(prayer::type_toggle(
            w,
            &p,
            prayer_type(&op["type"]),
            op["on"] == true
        )?),
        "permitted" => json!(prayer::type_permitted(w, &p, prayer_type(&op["type"]))?),
        "flag" => {
            prayer::flag(w, &p, prayer_type(&op["type"]))?;
            Value::Null
        }
        "find_next_icon" => {
            prayer::find_next_icon(w, &p, prayer_type(&op["type"]))?;
            Value::Null
        }
        "reset" => {
            prayer::reset(w, &p)?;
            Value::Null
        }
        "tick" => {
            prayer::tick(w, &p)?;
            Value::Null
        }
        "pulse" => {
            let due = std::mem::take(&mut w.pulses);
            let mut out = Vec::new();
            for player in due {
                out.push(json!(prayer::head_icon_pulse(w, &player)?));
            }
            json!(out)
        }
        "redemption" => {
            prayer::start_redemption(w, &p)?;
            Value::Null
        }
        "retribution" => {
            prayer::start_retribution(w, &p, entity("killer").as_ref())?;
            Value::Null
        }
        "commence_death" => {
            prayer::commence_death(w, &p, entity("killer").as_ref())?;
            Value::Null
        }
        "full_restore" => {
            prayer::full_restore(w, &p)?;
            Value::Null
        }
        "skill_bonus" => json!(hex(combat::skill_bonus(&w.ents[p].prayer, int("skill")))),
        "effective" => {
            let level = f64::from(int("level"));
            json!(hex((level
                + combat::skill_bonus(&w.ents[p].prayer, int("skill"))
                    * level)
                .floor()))
        }
        "get" => json!(w.ents[p].prayer.get(prayer_type(&op["type"]))),
        "icon" => json!(w.ents[p].prayer.icon(prayer_type(&op["type"]))),
        "has_icon" => json!(w.ents[p].prayer.has_icon()),
        "by_bonus" => {
            let ids: Vec<i32> = op["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            names(PrayerType::by_bonus(&ids))?
        }
        "melee_types" => names(PrayerType::melee_types())?,
        "range_types" => names(PrayerType::range_types())?,
        "magic_types" => names(PrayerType::magic_types())?,
        "for_button" => json!(PrayerType::for_button(int("button")).map(|t| t.name())),
        "protection" => {
            let e = &w.ents[entity("e").unwrap()];
            let s = style(&op["style"]);
            json!(if e.npc {
                combat::npc_has_protection_prayer(e.protect, s)
            } else {
                combat::player_has_protection_prayer(&e.prayer, s)
            })
        }
        "formatted_hit" => {
            let mut state = BattleState::<T>::new(entity("attacker"), entity("victim"));
            state.style = style(&op["style"]);
            state.armour_effect = op["armour"].as_str().map(|a| match a {
                "VERAC" => ArmourSet::Verac,
                "DHAROK" => ArmourSet::Dharok,
                other => panic!("armour {other}"),
            });
            let ents = &w.ents;
            let hit = combat::formatted_hit(
                &state,
                int("hit"),
                |e| {
                    if ents[*e].npc {
                        Kind::Npc
                    } else {
                        Kind::Player
                    }
                },
                |v, s| {
                    let v = &ents[*v];
                    Ok(if v.npc {
                        combat::npc_has_protection_prayer(v.protect, Some(s))
                    } else {
                        combat::player_has_protection_prayer(&v.prayer, Some(s))
                    })
                },
            )?;
            json!(hex(hit))
        }
        "set" => {
            let e = entity("e").unwrap();
            w.ents[e].apply(op["fields"].as_object().unwrap());
            Value::Null
        }
        "table" => table(),
        other => panic!("unknown op {other}"),
    })
}

fn error(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Binding(m) if m == "ConcurrentModificationException" => "ConcurrentModification",
        Error::Binding(_) => "Binding",
    }
}

fn evaluate(input: &Value) -> Value {
    let mut walk = State::default();
    let mut ctx = Ctx {
        events: Vec::new(),
        calls: 0,
        fail_at: -1,
        current: 0,
        sponge: false,
    };
    let mut ents = Vec::new();
    for (i, spec) in input["entities"].as_array().unwrap().iter().enumerate() {
        let node = walk.add(i as i32 + 1, Location::new(3200, 3200, 0), [1, 2, 3, 4]);
        let mut skills = Skills::new(&mut ctx).unwrap();
        skills.static_levels = [1; 24];
        skills.static_levels[3] = 10;
        let mut e = Ent {
            npc: spec["kind"] == "npc",
            location: None,
            prayer: PrayerState::default(),
            skills,
            bonuses: None,
            protect: None,
            head_icon: -1,
            varbits: HashMap::new(),
            flags: Map::new(),
            values: Values::default(),
            saved: Saved::default(),
            node: walk.retain_node(node).unwrap(),
        };
        e.apply(spec.as_object().unwrap());
        ents.push(e);
    }
    ctx.events.clear();
    let mut w = World {
        ents,
        ctx,
        rng: JavaRandom::new(input["seed"].as_i64().unwrap_or(0)),
        pulses: Vec::new(),
        input: input.clone(),
    };
    let mut steps = Vec::new();
    for op in input["ops"].as_array().unwrap() {
        w.ctx.events = Vec::new();
        w.ctx.calls = 0;
        w.ctx.fail_at = op.get("failAt").and_then(Value::as_i64).unwrap_or(-1);
        let (result, error) = match run(&mut w, op) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(error(&e))),
        };
        steps.push(json!({
            "result": result,
            "error": error,
            "events": std::mem::take(&mut w.ctx.events),
            "state": w.ents.iter().map(Ent::snapshot).collect::<Vec<_>>(),
            "pulses": w.pulses.len(),
        }));
    }
    Value::Array(steps)
}

fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(found) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(found);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (index, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(found) = difference(a, b, format!("{path}/{index}")) {
                return Some(found);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}

#[test]
fn original_prayer_manager_types_tab_and_combat_reads() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        provenance["fixtureSha256"].as_str().unwrap()
    );
    for source in [
        "reference/server/Server/src/main/core/game/node/entity/player/link/prayer/Prayer.java",
        "reference/server/Server/src/main/core/game/node/entity/player/link/prayer/PrayerType.java",
        "reference/server/Server/src/main/core/game/node/entity/player/link/prayer/PrayerCategory.java",
        "reference/server/Server/src/main/content/global/handlers/iface/tabs/PrayerTabInterface.kt",
    ] {
        assert!(
            provenance["sources"][source].is_string(),
            "{source} not pinned"
        );
    }
    let mut cases = 0;
    let mut actions = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = evaluate(&case["input"]);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            panic!(
                "prayer case {index} ({}): {found}\nops: {}",
                case["input"]["group"], case["input"]["ops"]
            );
        }
        cases += 1;
        actions += native.as_array().unwrap().len();
    }
    assert_eq!(cases, provenance["caseCount"].as_u64().unwrap() as usize);
    assert_eq!(
        actions,
        provenance["coverage"]["actions"].as_u64().unwrap() as usize
    );
}
