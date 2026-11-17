//! Replays the spell corpus (fixtures/spells) through `world::spells`. The
//! spell objects are the native table's; every service answers from the case
//! the way the oracle's recording adapters do, and every recorded call, draw,
//! result, error and state after each operation is compared.
use rs09_game_core::random::JavaRandom;
use rs09_server::world::combat_swing::{self as cs, BattleState, Types};
use rs09_server::world::spells::{
    self, Animation, CombinationRune, Error, Graphics, Host, MagicStaff, NodeKind, Priority,
    Projectile, ProjectileTarget, Result, Rune, Spell, SpellBlocks, SpellBook, SpellBookManager,
    SpellClass, SpellType, StateView, Value, autocast, book::SpellbookChangeSource, combat, table,
};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/spells/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/spells/provenance.json");

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn unhex(v: &Json) -> f64 {
    f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}

fn gfx(g: Option<Graphics>) -> Json {
    g.map_or(Json::Null, |g| json!([g.id, g.height, g.delay]))
}

fn anim(a: Option<Animation>) -> Json {
    a.map_or(Json::Null, |a| json!([a.id, a.delay, a.priority.name()]))
}

fn parse_graphics(v: &Json) -> Option<Graphics> {
    v.as_array()
        .map(|a| spells::graphics(int(&a[0]), int(&a[1]), int(&a[2])))
}

fn parse_animation(v: &Json) -> Option<Animation> {
    v.as_array().map(|a| Animation {
        id: int(&a[0]),
        delay: int(&a[1]),
        priority: [
            Priority::Low,
            Priority::Mid,
            Priority::High,
            Priority::VeryHigh,
        ]
        .into_iter()
        .find(|p| p.name() == a[2].as_str().unwrap())
        .unwrap(),
    })
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::Int(i) => json!(i),
        Value::Items(items) => {
            json!({ "items": items.iter().map(|(i, a)| json!([i, a])).collect::<Vec<_>>() })
        }
    }
}

fn json_value(v: &Json) -> Value {
    if let Some(items) = v.get("items") {
        Value::Items(
            items
                .as_array()
                .unwrap()
                .iter()
                .map(|a| (int(&a[0]), int(&a[1])))
                .collect(),
        )
    } else {
        Value::Int(int(v))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ref {
    Registered(usize),
    Default(usize),
}

struct Ent {
    kind: NodeKind,
    name: Option<String>,
    location: Option<[i32; 3]>,
    spec: Map<String, Json>,
    attributes: BTreeMap<String, Value>,
    autocast: Option<Ref>,
    inventory: Option<Vec<(i32, i32)>>,
    book: i32,
    god_casts: Option<Vec<i32>>,
    npc_id: i32,
}

struct World {
    spells: Vec<Spell>,
    index: HashMap<(SpellBook, i32), usize>,
    defaults: Vec<Spell>,
    nodes: Vec<Ent>,
    names: Map<String, Json>,
    events: Vec<Json>,
    calls: i64,
    fail_at: i64,
    ticks: i32,
    dev_mode: bool,
    experience_mod: f64,
    math: Option<Vec<Json>>,
    math_at: usize,
    math_base: JavaRandom,
    blocks: SpellBlocks<usize>,
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

    fn label(&self, r: Option<Ref>) -> Json {
        match r {
            None => Json::Null,
            Some(Ref::Default(k)) => json!(["DEFAULT", k]),
            Some(Ref::Registered(i)) => {
                json!([self.spells[i].book.name(), self.spells[i].spell_id])
            }
        }
    }

    fn spell_ref(&self, v: &Json) -> Option<Ref> {
        let a = v.as_array()?;
        if a[0] == "DEFAULT" {
            return Some(Ref::Default(a[1].as_u64().unwrap() as usize));
        }
        let book = SpellBook::from_name(a[0].as_str().unwrap()).unwrap();
        self.index
            .get(&(book, int(&a[1])))
            .map(|i| Ref::Registered(*i))
    }

    fn spec(&self, n: usize) -> &Map<String, Json> {
        &self.nodes[n].spec
    }

    fn flag(&self, n: usize, key: &str, fallback: bool) -> bool {
        match self.spec(n).get(key) {
            None | Some(Json::Null) => fallback,
            Some(v) => v == &Json::Bool(true),
        }
    }

    fn keyed(&self, n: usize, key: &str, slot: i32) -> Option<i32> {
        self.spec(n)
            .get(key)
            .and_then(|m| m.get(slot.to_string()))
            .filter(|v| !v.is_null())
            .map(int)
    }

    fn listed(&self, n: usize, key: &str, name: &str) -> bool {
        self.spec(n)
            .get(key)
            .and_then(|v| v.as_array())
            .is_some_and(|l| l.iter().any(|x| x == name))
    }

    fn apply(&mut self, n: usize, fields: &Map<String, Json>) {
        for (key, v) in fields {
            match key.as_str() {
                "name" => self.nodes[n].name = v.as_str().map(str::to_owned),
                "location" => {
                    self.nodes[n].location = v
                        .as_array()
                        .map(|a| [int(&a[0]), int(&a[1]), int(&a[2]).rem_euclid(4)])
                }
                "inventory" => {
                    self.nodes[n].inventory = Some(
                        v.as_array()
                            .unwrap()
                            .iter()
                            .map(|s| (int(&s[0]), int(&s[1])))
                            .collect(),
                    )
                }
                "attributes" => {
                    for (k, a) in v.as_object().unwrap() {
                        if a.is_null() {
                            self.nodes[n].attributes.remove(k);
                        } else {
                            self.nodes[n].attributes.insert(k.clone(), json_value(a));
                        }
                    }
                }
                "autocast" => self.nodes[n].autocast = self.spell_ref(v),
                "book" => {
                    let book = SpellBook::from_name(v.as_str().unwrap()).unwrap();
                    SpellBookManager::set_spell_book(&mut self.nodes[n].book, book);
                }
                "npcId" => self.nodes[n].npc_id = int(v),
                "godCasts" => {
                    self.nodes[n].god_casts = Some(v.as_array().unwrap().iter().map(int).collect())
                }
                _ => {
                    self.nodes[n].spec.insert(key.clone(), v.clone());
                }
            }
        }
    }

    fn snapshot(&self) -> Json {
        let mut out = Vec::new();
        for e in &self.nodes {
            let mut m = Map::new();
            if e.kind != NodeKind::Node {
                let attributes: Map<String, Json> = e
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), value_json(v)))
                    .collect();
                m.insert("attributes".into(), Json::Object(attributes));
                m.insert("autocast".into(), self.label(e.autocast));
                if let Some(inv) = &e.inventory {
                    m.insert(
                        "inventory".into(),
                        json!(inv.iter().map(|(i, a)| json!([i, a])).collect::<Vec<_>>()),
                    );
                }
                if e.kind == NodeKind::Player {
                    m.insert("book".into(), json!(e.book));
                }
                if let Some(c) = &e.god_casts {
                    m.insert("godCasts".into(), json!(c));
                }
            }
            out.push(Json::Object(m));
        }
        Json::Array(out)
    }

    fn ix(n: Option<&usize>) -> Json {
        n.map_or(Json::Null, |n| json!(n))
    }
}

impl Host for World {
    type Node = usize;
    type Spell = Ref;

    fn kind(&self, node: &usize) -> NodeKind {
        self.nodes[*node].kind
    }
    fn same_node(&self, a: &usize, b: &usize) -> bool {
        a == b
    }
    fn same_spell(&self, a: &Ref, b: &Ref) -> bool {
        a == b
    }
    fn spell(&mut self, spell: &Ref) -> &mut Spell {
        match *spell {
            Ref::Registered(i) => &mut self.spells[i],
            Ref::Default(k) => &mut self.defaults[k],
        }
    }
    fn registered(&self, book: SpellBook, button: i32) -> Option<Ref> {
        self.index.get(&(book, button)).map(|i| Ref::Registered(*i))
    }
    fn item_name(&self, id: i32) -> String {
        self.names
            .get(&id.to_string())
            .and_then(|v| v.as_str())
            .unwrap_or("null")
            .to_owned()
    }
    fn experience_mod(&mut self) -> f64 {
        self.experience_mod
    }
    fn spell_book(&mut self, player: &usize) -> &mut i32 {
        &mut self.nodes[*player].book
    }
    fn staff_interface(&mut self, player: &usize) -> bool {
        self.flag(*player, "staffInterface", true)
    }
    fn set_god_cast(&mut self, player: &usize, index: usize, value: i32) {
        self.nodes[*player].god_casts.as_mut().unwrap()[index] = value;
    }

    fn ticks(&mut self) -> Result<i32> {
        self.emit(json!(["ticks"]))?;
        Ok(self.ticks)
    }
    fn dev_mode(&mut self) -> Result<bool> {
        self.emit(json!(["dev_mode"]))?;
        Ok(self.dev_mode)
    }
    fn name(&mut self, node: &usize) -> Result<Option<String>> {
        self.emit(json!(["name", node]))?;
        Ok(self.nodes[*node].name.clone())
    }
    fn location(&mut self, node: &usize) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["location", node]))?;
        Ok(self.nodes[*node].location)
    }
    fn level(&mut self, entity: &usize, slot: i32) -> Result<i32> {
        self.emit(json!(["level", entity, slot]))?;
        Ok(self.keyed(*entity, "levels", slot).unwrap_or(1))
    }
    fn level_with(&mut self, entity: &usize, slot: i32, discard: bool) -> Result<i32> {
        self.emit(json!(["level_with", entity, slot, discard]))?;
        Ok(self.keyed(*entity, "levels", slot).unwrap_or(1))
    }
    fn add_experience(
        &mut self,
        entity: &usize,
        slot: i32,
        experience: f64,
        player_mod: bool,
    ) -> Result<()> {
        self.emit(json!([
            "add_experience",
            entity,
            slot,
            hex(experience),
            player_mod
        ]))
    }
    fn add_experience_default(&mut self, entity: &usize, slot: i32, experience: f64) -> Result<()> {
        self.emit(json!(["add_experience2", entity, slot, hex(experience)]))
    }
    fn equipment(&mut self, player: &usize, slot: i32) -> Result<Option<i32>> {
        self.emit(json!(["equipment", player, slot]))?;
        Ok(self.keyed(*player, "worn", slot))
    }
    fn equipment_new(&mut self, player: &usize, slot: i32) -> Result<i32> {
        self.emit(json!(["equipment_new", player, slot]))?;
        Ok(self.keyed(*player, "worn", slot).unwrap_or(0))
    }
    fn inventory_amount(&mut self, player: &usize, id: i32) -> Result<i32> {
        self.emit(json!(["inventory_amount", player, id]))?;
        let inv = self.nodes[*player].inventory.as_ref().unwrap();
        Ok(inv
            .iter()
            .filter(|s| s.0 == id)
            .fold(0i32, |t, s| t.wrapping_add(s.1)))
    }
    fn inventory_remove(&mut self, player: &usize, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["inventory_remove", player, id, amount]))?;
        // The oracle's stand-in removal: from the first slot holding the id,
        // emptied slots leave the list.
        let inv = self.nodes[*player].inventory.as_mut().unwrap();
        let mut left = amount;
        let mut k = 0;
        while k < inv.len() && left > 0 {
            if inv[k].0 != id {
                k += 1;
                continue;
            }
            let take = left.min(inv[k].1);
            inv[k].1 -= take;
            left -= take;
            if inv[k].1 == 0 {
                inv.remove(k);
            } else {
                k += 1;
            }
        }
        Ok(left == 0)
    }
    fn send_message(&mut self, player: &usize, text: &str) -> Result<()> {
        self.emit(json!(["send_message", player, text]))
    }
    fn packet_message(&mut self, player: &usize, text: &str) -> Result<()> {
        self.emit(json!(["message", player, text]))
    }
    fn interface_config(
        &mut self,
        player: &usize,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        self.emit(json!(["interface_config", player, interface, child, hide]))
    }
    fn dispatch_cast(
        &mut self,
        player: &usize,
        book: SpellBook,
        spell_id: i32,
        target: Option<&usize>,
    ) -> Result<()> {
        self.emit(json!([
            "dispatch",
            player,
            "SpellCastEvent",
            book.name(),
            spell_id,
            World::ix(target)
        ]))
    }
    fn face_location(&mut self, entity: &usize, location: Option<[i32; 3]>) -> Result<()> {
        self.emit(json!(["face_location", entity, location]))
    }
    fn face_temporary(&mut self, entity: &usize, target: Option<&usize>, ticks: i32) -> Result<()> {
        self.emit(json!(["face_temporary", entity, World::ix(target), ticks]))
    }
    fn face(&mut self, entity: &usize, target: Option<&usize>) -> Result<()> {
        self.emit(json!(["face", entity, World::ix(target)]))
    }
    fn graphics(&mut self, entity: &usize, graphics: Option<Graphics>) -> Result<()> {
        self.emit(json!(["graphics", entity, gfx(graphics)]))
    }
    fn animate(&mut self, entity: &usize, animation: Option<Animation>) -> Result<()> {
        self.emit(json!(["animate", entity, anim(animation)]))
    }
    fn force_animation(&mut self, entity: &usize, animation: Option<Animation>) -> Result<()> {
        self.emit(json!(["force_animation", entity, anim(animation)]))
    }
    fn global_audio(
        &mut self,
        location: [i32; 3],
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> Result<()> {
        self.emit(json!(["global_audio", location, id, delay, loops, radius]))
    }
    fn projectile_magic(
        &mut self,
        source: &usize,
        victim: Option<&usize>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        angle: i32,
    ) -> Result<Projectile> {
        self.emit(json!([
            "projectile_magic",
            source,
            World::ix(victim),
            id,
            start_height,
            end_height,
            start_delay,
            angle
        ]))?;
        Ok(Projectile {
            label: "magic".into(),
            projectile_id: id,
            start_height,
            end_height,
            start_delay,
            speed: -1,
            angle,
            distance: -1,
        })
    }
    fn send_projectile(
        &mut self,
        projectile: &Projectile,
        source: &usize,
        target: ProjectileTarget<'_, usize>,
        npc: bool,
        base: i32,
        modified: i32,
    ) -> Result<()> {
        let (victim, location) = match target {
            ProjectileTarget::Entity(v) => (World::ix(v), Json::Null),
            ProjectileTarget::Location(l) => (Json::Null, json!(l)),
        };
        self.emit(json!([
            "projectile",
            projectile.label,
            source,
            victim,
            location,
            npc,
            base,
            modified
        ]))
    }
    fn attribute(&mut self, entity: &usize, key: &str, default: Value) -> Result<Value> {
        self.emit(json!(["get_attribute", entity, key, value_json(&default)]))?;
        Ok(self.nodes[*entity]
            .attributes
            .get(&key.replace("/save:", ""))
            .cloned()
            .unwrap_or(default))
    }
    fn set_attribute(&mut self, entity: &usize, key: &str, value: Value) -> Result<()> {
        self.emit(json!(["set_attribute", entity, key, value_json(&value)]))?;
        let key = key.strip_prefix("/save:").unwrap_or(key);
        self.nodes[*entity].attributes.insert(key.to_owned(), value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &usize, key: &str) -> Result<()> {
        self.emit(json!(["remove_attribute", entity, key]))?;
        self.nodes[*entity].attributes.remove(key);
        Ok(())
    }
    fn autocast(&mut self, entity: &usize) -> Result<Option<Ref>> {
        self.emit(json!(["autocast", entity]))?;
        Ok(self.nodes[*entity].autocast)
    }
    fn set_autocast(&mut self, entity: &usize, spell: Option<&Ref>) -> Result<()> {
        let label = self.label(spell.copied());
        self.emit(json!(["set_autocast", entity, label]))?;
        self.nodes[*entity].autocast = spell.copied();
        Ok(())
    }
    fn set_spell(&mut self, entity: &usize, spell: &Ref) -> Result<()> {
        let label = self.label(Some(*spell));
        self.emit(json!(["set_spell", entity, label]))
    }
    fn is_attacking(&mut self, entity: &usize) -> Result<bool> {
        self.emit(json!(["is_attacking", entity]))?;
        Ok(self.flag(*entity, "attacking", false))
    }
    fn set_victim(&mut self, entity: &usize, victim: Option<&usize>) -> Result<()> {
        self.emit(json!(["set_victim", entity, World::ix(victim)]))
    }
    fn update_style(&mut self, entity: &usize) -> Result<()> {
        self.emit(json!(["update_style", entity]))
    }
    fn pulse_start(&mut self, entity: &usize) -> Result<()> {
        self.emit(json!(["pulse_start", entity]))
    }
    fn pulse_attack(&mut self, entity: &usize, victim: Option<&usize>) -> Result<()> {
        self.emit(json!(["pulse_attack", entity, World::ix(victim)]))
    }
    fn attack_style(&mut self, entity: &usize) -> Result<Option<i32>> {
        self.emit(json!(["attack_style", entity]))?;
        Ok(self
            .spec(*entity)
            .get("style")
            .filter(|v| !v.is_null())
            .map(int))
    }
    fn attack_style_index(&mut self, player: &usize) -> Result<i32> {
        self.emit(json!(["attack_style_index", player]))?;
        Ok(self.spec(*player).get("styleIndex").map_or(0, int))
    }
    fn magic_animation(&mut self, entity: &usize) -> Result<Option<Animation>> {
        self.emit(json!(["magic_animation", entity]))?;
        Ok(self
            .spec(*entity)
            .get("magicAnimation")
            .and_then(parse_animation))
    }
    fn attack_animation(&mut self, entity: &usize) -> Result<Option<Animation>> {
        self.emit(json!(["attack_animation", entity]))?;
        Ok(self
            .spec(*entity)
            .get("attackAnimation")
            .and_then(parse_animation))
    }
    fn multi_zone(&mut self, entity: &usize) -> Result<bool> {
        self.emit(json!(["multi_zone", entity]))?;
        Ok(self.flag(*entity, "multi", false))
    }
    fn bonuses(&mut self, entity: &usize) -> Result<Option<Vec<i32>>> {
        self.emit(json!(["bonuses", entity]))?;
        Ok(self
            .spec(*entity)
            .get("bonuses")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(int).collect()))
    }
    fn in_zone(&mut self, entity: &usize, name: &str) -> Result<bool> {
        self.emit(json!(["in_zone", entity, name]))?;
        Ok(self.listed(*entity, "zones", name))
    }
    fn timer_active(&mut self, entity: &usize, name: &str) -> Result<bool> {
        self.emit(json!(["timer_active", entity, name]))?;
        Ok(self.listed(*entity, "timers", name))
    }
    fn god_casts(&mut self, player: &usize) -> Result<Option<Vec<i32>>> {
        self.emit(json!(["god_casts", player]))?;
        Ok(self.nodes[*player].god_casts.clone())
    }
    fn npc_id(&mut self, npc: &usize) -> Result<i32> {
        self.emit(json!(["npc_id", npc]))?;
        Ok(self.nodes[*npc].npc_id)
    }
    fn npc_task(&mut self, npc: &usize) -> Result<Option<bool>> {
        self.emit(json!(["npc_task", npc]))?;
        Ok(self.spec(*npc).get("task").and_then(|v| v.as_bool()))
    }
    fn combat_graphics(&mut self, npc: &usize) -> Result<Option<Vec<Option<Graphics>>>> {
        self.emit(json!(["combat_graphics", npc]))?;
        Ok(self
            .spec(*npc)
            .get("combatGraphics")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(parse_graphics).collect()))
    }
    fn multi_targets(&mut self, target: &usize, attacker: &usize) -> Result<Vec<usize>> {
        self.emit(json!(["multi_targets", target, attacker, "MAGIC"]))?;
        Ok(self
            .spec(*target)
            .get("multiTargets")
            .and_then(|v| v.as_array())
            .map_or_else(Vec::new, |a| {
                a.iter().map(|v| v.as_u64().unwrap() as usize).collect()
            }))
    }
    fn open_tab(&mut self, player: &usize, id: i32) -> Result<()> {
        self.emit(json!(["open_tab", player, id]))
    }
    fn set_varbit(&mut self, player: &usize, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["set_varbit", player, id, value, save]))
    }
    fn component_visibility(
        &mut self,
        player: &usize,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        self.emit(json!([
            "component_visibility",
            player,
            interface,
            child,
            hide
        ]))
    }
    fn set_attack_style(&mut self, player: &usize, button: i32) -> Result<bool> {
        self.emit(json!(["set_attack_style", player, button]))?;
        Ok(true)
    }
    fn math_random(&mut self) -> Result<f64> {
        let v = match &self.math {
            Some(script) => {
                let v = unhex(&script[self.math_at]);
                self.math_at += 1;
                v
            }
            None => self.math_base.next_double(),
        };
        self.emit(json!(["math_random", hex(v)]))?;
        Ok(v)
    }
}

fn row_json(s: &Spell) -> Json {
    json!({
        "class": s.class.name(),
        "book": s.book.name(),
        "spellId": s.spell_id,
        "level": s.level,
        "levelRequirement": s.level,
        "experience": hex(s.experience),
        "delay": spells::delay(),
        "runes": s.runes.as_ref().map(|r| r.iter().map(|(i, a)| json!([i, a])).collect::<Vec<_>>()),
        "animation": anim(s.animation),
        "graphic": gfx(s.graphic),
        "audio": [s.audio.id, s.audio.delay, s.audio.loops, s.audio.radius],
        "type": s.kind.name(),
        "accuracy": hex(s.kind.accuracy_mod()),
        "impactAudio": s.impact_audio,
        "endGraphic": gfx(s.end_graphic),
        "projectile": s.projectile.as_ref().map(|p| json!([p.label, p.projectile_id, p.start_height, p.end_height,
            p.start_delay, p.speed, p.angle, p.distance])),
    })
}

fn table_json(w: &World) -> Json {
    let runes: Vec<Json> = Rune::ALL
        .iter()
        .map(|r| json!([r.name(), r.id(), r.id(), 1, 7]))
        .collect();
    let combos: Vec<Json> = CombinationRune::ALL
        .iter()
        .map(|c| {
            json!([
                c.name(),
                c.id(),
                c.types().iter().map(|r| r.name()).collect::<Vec<_>>()
            ])
        })
        .collect();
    let staves: Vec<Json> = MagicStaff::ALL
        .iter()
        .map(|s| json!([s.name(), s.rune_id(), s.staves()]))
        .collect();
    let types: Vec<Json> = SpellType::ALL
        .iter()
        .map(|t| json!([t.name(), t.ordinal(), hex(t.accuracy_mod())]))
        .collect();
    let mut books = Vec::new();
    for book in SpellBook::ALL {
        let mut rows: Vec<(i32, Json)> = w
            .index
            .iter()
            .filter(|((b, _), _)| *b == book)
            .map(|((_, button), i)| (*button, row_json(&w.spells[*i])))
            .collect();
        rows.sort_by_key(|r| r.0);
        let rows: Vec<Json> = rows.into_iter().map(|(b, r)| json!([b, r])).collect();
        books.push(json!([
            book.name(),
            book.ordinal(),
            book.interface_id(),
            rows
        ]));
    }
    let constants = json!([
        gfx(Some(spells::SPLASH_GRAPHIC)),
        anim(Some(spells::AUTOCAST_ANIMATION)),
        [SpellbookChangeSource::ALL.len()]
    ]);
    json!([runes, combos, staves, types, books, constants])
}

fn node(v: &Json) -> Option<usize> {
    v.as_u64().map(|n| n as usize)
}

/// The combat slices' types, with this test's entity and spell handles.
struct Combat;
impl Types for Combat {
    type Entity = usize;
    type Item = i32;
    type Spell = Ref;
}

fn combat_error(e: cs::Error) -> &'static str {
    match e {
        cs::Error::Null => "Null",
        cs::Error::Index => "Index",
        cs::Error::Binding(s) if s == "java.lang.ClassCastException" => "Cast",
        cs::Error::Binding(_) => "Binding",
    }
}

/// The operations a combat host routes to `spells::combat`, run through
/// those answers; `None` for every other operation.
fn run_combat(
    w: &mut World,
    op: &Map<String, Json>,
) -> Option<std::result::Result<Json, &'static str>> {
    let get = |k: &str| op.get(k).unwrap_or(&Json::Null);
    let flag = |k: &str| get(k) == &Json::Bool(true);
    let e = node(get("e"));
    let name = get("op").as_str().unwrap();
    if !matches!(
        name,
        "meets"
            | "max_impact"
            | "targets"
            | "visualize"
            | "visualize_impact"
            | "add_experience"
            | "select_auto"
    ) {
        return None;
    }
    if name == "max_impact" && e.is_none() {
        return None;
    }
    if let Some(m) = op.get("experienceMod") {
        w.experience_mod = unhex(m);
    }
    let s = w.spell_ref(get("spell"));
    let out = match name {
        "meets" => {
            combat::meets_requirements(w, &s.unwrap(), &e.unwrap(), flag("message"), flag("remove"))
                .map(|v| json!(v))
        }
        "max_impact" => {
            combat::maximum_impact(w, &s.unwrap(), &e.unwrap(), node(get("victim")).as_ref())
                .map(|v| json!(v))
        }
        "targets" => combat::targets::<World, Combat>(
            w,
            &s.unwrap(),
            &e.unwrap(),
            node(get("target")).as_ref(),
        )
        .map(|t| {
            json!(
                t.unwrap()
                    .iter()
                    .map(|b| {
                        let b = b.as_ref().unwrap();
                        json!([b.entity, World::ix(b.victim.as_ref())])
                    })
                    .collect::<Vec<_>>()
            )
        }),
        "visualize" => combat::visualize(w, &s.unwrap(), &e.unwrap(), node(get("target")).as_ref())
            .map(|_| Json::Null),
        "visualize_impact" => {
            let mut state = BattleState::<Combat>::new(e, node(get("target")));
            state.estimated_hit = int(get("hit"));
            state.frozen = flag("frozen");
            combat::visualize_impact(
                w,
                &s.unwrap(),
                e.as_ref(),
                node(get("target")).as_ref(),
                &state,
            )
            .map(|_| json!([state.estimated_hit, state.frozen]))
        }
        "add_experience" => {
            combat::add_experience(w, &s.unwrap(), &e.unwrap(), int(get("hit"))).map(|_| Json::Null)
        }
        _ => combat::select_auto_spell(
            w,
            &node(get("p")).unwrap(),
            int(get("button")),
            flag("adjust"),
            flag("forget"),
        )
        .map(|_| Json::Null),
    };
    Some(out.map_err(combat_error))
}

fn run(w: &mut World, op: &Map<String, Json>) -> Result<Json> {
    if let Some(m) = op.get("experienceMod") {
        w.experience_mod = unhex(m);
    }
    let get = |k: &str| op.get(k).unwrap_or(&Json::Null);
    let flag = |k: &str| get(k) == &Json::Bool(true);
    let spell = |w: &World| w.spell_ref(get("spell")).expect("spell");
    let e = node(get("e"));
    Ok(match get("op").as_str().unwrap() {
        "table" => table_json(w),
        "runes_for_id" => json!(Rune::for_id(int(get("id"))).map(|r| r.name())),
        "is_infinite" => {
            let rune = get("rune").as_str().map(|n| Rune::from_name(n).unwrap());
            let weapon = get("weapon").as_i64().map(|v| v as i32);
            let types: Option<Vec<SpellType>> = get("types").as_array().map(|a| {
                a.iter()
                    .map(|t| SpellType::from_name(t.as_str().unwrap()).unwrap())
                    .collect()
            });
            json!(spells::is_infinite(rune, weapon, types.as_deref()))
        }
        "staff_for_id" => json!(MagicStaff::for_id(int(get("rune"))).map(|s| s.name())),
        "eligible_for" => {
            let rune = get("rune").as_str().map(|n| Rune::from_name(n).unwrap());
            json!(
                CombinationRune::eligible_for(rune)
                    .iter()
                    .map(|c| c.name())
                    .collect::<Vec<_>>()
            )
        }
        "type_impact" => {
            let t = SpellType::from_name(get("type").as_str().unwrap()).unwrap();
            let victim = node(get("victim"));
            json!(t.impact_amount(w, e.as_ref(), victim.as_ref(), int(get("base")))?)
        }
        "using_staff" | "utils_using_staff" => json!(spells::using_staff(
            w,
            &node(get("p")).unwrap(),
            int(get("rune"))
        )?),
        "has_runes" => {
            let runes: Vec<(i32, i32)> = get("runes")
                .as_array()
                .unwrap()
                .iter()
                .map(|a| (int(&a[0]), int(&a[1])))
                .collect();
            json!(
                spells::has_runes(w, &node(get("p")).unwrap(), &runes)?.map(|(i, a)| json!([i, a]))
            )
        }
        "meets" => {
            let s = spell(w);
            json!(spells::meets_requirements(
                w,
                &s,
                &e.unwrap(),
                flag("message"),
                flag("remove")
            )?)
        }
        "check_level" => {
            let s = spell(w);
            json!(spells::check_level_requirement(
                w,
                &s,
                &e.unwrap(),
                flag("message")
            )?)
        }
        "add_experience" => {
            let s = spell(w);
            spells::add_experience(w, &s, &e.unwrap(), int(get("hit")))?;
            Json::Null
        }
        "visualize" => {
            let s = spell(w);
            spells::visualize(w, &s, &e.unwrap(), node(get("target")).as_ref())?;
            Json::Null
        }
        "visualize_impact" => {
            let s = spell(w);
            let state = StateView {
                estimated_hit: int(get("hit")),
                frozen: flag("frozen"),
            };
            spells::visualize_impact(w, &s, e.as_ref(), node(get("target")).as_ref(), state)?;
            json!([state.estimated_hit, state.frozen])
        }
        "cast" => {
            let s = spell(w);
            json!(spells::cast(
                w,
                &s,
                &e.unwrap(),
                node(get("target")).as_ref()
            )?)
        }
        "cast_spell" => {
            let book = SpellBook::from_name(get("book").as_str().unwrap()).unwrap();
            json!(spells::cast_spell(
                w,
                &node(get("p")).unwrap(),
                book,
                int(get("button")),
                node(get("target")).as_ref()
            )?)
        }
        "max_impact" => {
            let s = spell(w);
            json!(spells::maximum_impact(
                w,
                &s,
                e.as_ref(),
                node(get("victim")).as_ref()
            )?)
        }
        "targets" => {
            let s = spell(w);
            let t = spells::get_targets(w, &s, &e.unwrap(), node(get("target")).as_ref())?;
            json!(
                t.iter()
                    .map(|(a, b)| json!([a, World::ix(b.as_ref())]))
                    .collect::<Vec<_>>()
            )
        }
        "accuracy" => {
            let s = spell(w);
            json!(hex(w.spell(&s).kind.accuracy_mod()))
        }
        "experience" => {
            let s = spell(w);
            json!(hex(spells::experience(w, &s)))
        }
        "make_default" => {
            let npc = node(get("npc")).unwrap();
            let s = spells::default_combat_spell(w, &npc)?;
            w.defaults.push(s);
            json!(w.defaults.len() - 1)
        }
        "set_book" => {
            let p = node(get("p")).unwrap();
            let book = SpellBook::from_name(get("book").as_str().unwrap()).unwrap();
            SpellBookManager::set_spell_book(w.spell_book(&p), book);
            Json::Null
        }
        "book_update" => {
            SpellBookManager::update(w, &node(get("p")).unwrap())?;
            Json::Null
        }
        "for_interface" => json!(SpellBook::for_interface(int(get("id"))).map(|b| b.name())),
        "get_spell" => {
            let book = SpellBook::from_name(get("book").as_str().unwrap()).unwrap();
            let r = w.registered(book, int(get("button")));
            w.label(r)
        }
        "block_register" => {
            w.blocks
                .register(int(get("spellId")), node(get("node")).unwrap());
            Json::Null
        }
        "is_blocked" => {
            let blocks = std::mem::take(&mut w.blocks);
            let out = blocks.is_blocked(w, int(get("spellId")), &node(get("node")).unwrap());
            w.blocks = blocks;
            json!(out?)
        }
        "autospell_id" => json!(autocast::autospell_id(
            w,
            &node(get("p")).unwrap(),
            int(get("spellId"))
        )?),
        "select_auto" => {
            autocast::select_auto_spell(
                w,
                &node(get("p")).unwrap(),
                int(get("button")),
                flag("adjust"),
                flag("forget"),
            )?;
            Json::Null
        }
        "can_autocast" => json!(autocast::can_autocast(
            w,
            &node(get("p")).unwrap(),
            flag("message")
        )?),
        "set" => {
            w.apply(e.unwrap(), get("fields").as_object().unwrap());
            Json::Null
        }
        "world" => {
            if let Some(t) = op.get("ticks") {
                w.ticks = int(t);
            }
            if let Some(d) = op.get("devMode") {
                w.dev_mode = d == &Json::Bool(true);
            }
            Json::Null
        }
        other => panic!("unknown op {other}"),
    })
}

fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Cast => "Cast",
        Error::Binding(_) => "Binding",
    }
}

fn world(input: &Value2, names: &Map<String, Json>) -> World {
    let mut spells = Vec::new();
    let mut index = HashMap::new();
    for (book, button, spell) in table::combat_spells() {
        index.insert((book, button), spells.len());
        spells.push(spell);
    }
    let mut w = World {
        spells,
        index,
        defaults: Vec::new(),
        nodes: Vec::new(),
        names: names.clone(),
        events: Vec::new(),
        calls: 0,
        fail_at: -1,
        ticks: input.get("ticks").map_or(0, int),
        dev_mode: input.get("devMode") == Some(&Json::Bool(true)),
        experience_mod: input.get("experienceMod").map_or(4.0, unhex),
        math: input.get("math").and_then(|m| m.as_array()).cloned(),
        math_at: 0,
        math_base: JavaRandom::new(input.get("mathSeed").map_or(0, |v| v.as_i64().unwrap())),
        blocks: SpellBlocks::default(),
    };
    for spec in input["entities"].as_array().unwrap() {
        let spec = spec.as_object().unwrap();
        let kind = match spec
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("player")
        {
            "npc" => NodeKind::Npc,
            "node" => NodeKind::Node,
            _ => NodeKind::Player,
        };
        w.nodes.push(Ent {
            kind,
            name: None,
            location: Some([3200, 3200, 0]),
            spec: Map::new(),
            attributes: BTreeMap::new(),
            autocast: None,
            inventory: (kind == NodeKind::Player).then(Vec::new),
            book: SpellBookManager::INITIAL,
            god_casts: None,
            npc_id: 0,
        });
        let n = w.nodes.len() - 1;
        w.apply(n, spec);
    }
    w
}

type Value2 = Map<String, Json>;

/// Replays every row; with `combat`, the operations a combat host routes to
/// `spells::combat` go through those answers. Returns cases, actions, routed
/// operations and the mismatches.
fn replay(combat: bool) -> (usize, usize, usize, Vec<String>) {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let names = provenance["itemNames"].as_object().unwrap().clone();
    let mut cases = 0;
    let mut actions = 0;
    let mut routed = 0;
    let mut mismatches = Vec::new();
    for (number, line) in CORPUS.lines().enumerate() {
        let row: Json = serde_json::from_str(line).unwrap();
        let input = row["input"].as_object().unwrap();
        let mut w = world(input, &names);
        cases += 1;
        assert_eq!(w.snapshot(), row["initial"], "case {number}: initial state");
        let mut previous = w.snapshot();
        for (k, (op, expected)) in input["ops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
            .enumerate()
        {
            actions += 1;
            let op = op.as_object().unwrap();
            w.events.clear();
            w.calls = 0;
            w.fail_at = op.get("failAt").map_or(-1, |v| v.as_i64().unwrap());
            let routed_result = if combat { run_combat(&mut w, op) } else { None };
            let (result, error) = match routed_result {
                Some(Ok(v)) => {
                    routed += 1;
                    (v, Json::Null)
                }
                Some(Err(e)) => {
                    routed += 1;
                    (Json::Null, json!(e))
                }
                None => match run(&mut w, op) {
                    Ok(v) => (v, Json::Null),
                    Err(e) => (Json::Null, json!(error_name(&e))),
                },
            };
            let state = w.snapshot();
            let state_json = if state == previous {
                Json::Null
            } else {
                state.clone()
            };
            previous = state;
            let got = json!({
                "result": result,
                "error": error,
                "events": w.events,
                "state": state_json,
                "experienceMod": hex(w.experience_mod),
                "mathDraws": w.math_at,
            });
            if &got != expected {
                mismatches.push(format!(
                    "case {number} ({}) op {k} {}:\n  expected {expected}\n  got      {got}",
                    input["group"],
                    Json::Object(op.clone())
                ));
                break;
            }
        }
    }
    (cases, actions, routed, mismatches)
}

#[test]
fn spell_corpus_replays_natively() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        provenance["fixtureSha256"].as_str().unwrap()
    );
    for source in [
        "reference/server/Server/src/main/core/game/node/entity/combat/spell/MagicSpell.java",
        "reference/server/Server/src/main/content/global/skill/magic/SpellUtils.kt",
        "oracle/spells/Support.java",
        "oracle/spells/cases.py",
    ] {
        assert!(
            provenance["sources"][source].is_string(),
            "{source} is not hashed"
        );
    }
    let (cases, actions, _, mismatches) = replay(false);
    assert!(
        mismatches.is_empty(),
        "{} mismatching cases; first:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(cases, provenance["caseCount"].as_u64().unwrap() as usize);
    assert_eq!(
        actions,
        provenance["coverage"]["actions"].as_u64().unwrap() as usize
    );
    println!("{cases} spell cases, {actions} actions replayed");
}

/// The same corpus with the operations a combat host routes to
/// `spells::combat` answered there, in the combat slices' shapes and error
/// type.
#[test]
fn spell_corpus_replays_through_the_combat_answers() {
    let (cases, actions, routed, mismatches) = replay(true);
    assert!(
        mismatches.is_empty(),
        "{} mismatching cases; first:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(routed > 10_000, "{routed}");
    println!("{cases} spell cases, {actions} actions, {routed} through spells::combat");
}

#[test]
fn table_registers_every_combat_spell_once() {
    let spells = table::combat_spells();
    let mut seen = std::collections::HashSet::new();
    for (book, button, spell) in &spells {
        assert!(
            seen.insert((*book, *button)),
            "{book:?} {button} registered twice"
        );
        assert_eq!(spell.spell_id, *button);
        assert_ne!(spell.class, SpellClass::Default);
    }
    assert_eq!(spells.len(), 53);
    let registry = table::persisted_registry();
    assert_eq!(registry.spells.len(), 53);
    assert!(registry.spells.values().all(|s| s.combat));
}
