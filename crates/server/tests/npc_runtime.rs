//! Replays npc-runtime-oracle: the original NPC constructors, transform,
//! clear and Entity.init/clear through `world::npc_runtime` over a recording
//! host whose `Repository.getNpcs()` is the native `NodeList`.
use rs09_game_core::direction::Direction;
use rs09_server::world::{
    doors::Location,
    npc_runtime::{self as nr, Error, Host, Kind},
    repository::NodeList,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const FIXTURE: &str = include_str!("../../../fixtures/npc-runtime/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/npc-runtime/provenance.json");

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn loc_value(l: Option<&Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l.coordinates()))
}

fn place(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(int(&a[0]), int(&a[1]), int(&a[2])))
}

fn direction_name(d: Direction) -> &'static str {
    match d {
        Direction::North => "NORTH",
        Direction::East => "EAST",
        Direction::South => "SOUTH",
        Direction::West => "WEST",
        Direction::NorthEast => "NORTH_EAST",
        Direction::SouthEast => "SOUTH_EAST",
        Direction::SouthWest => "SOUTH_WEST",
        Direction::NorthWest => "NORTH_WEST",
    }
}

#[derive(Clone)]
struct Definition {
    label: String,
    name: Option<String>,
    size: i32,
}

struct Row {
    kind: Kind,
    name: Option<String>,
    location: Option<Location>,
    index: i32,
    size: i32,
    active: bool,
    direction: Option<Direction>,
    plugin: Option<i64>,
    id: i32,
    original_id: i32,
    definition: Option<Definition>,
    behavior: Option<i32>,
}

struct World {
    rows: Vec<Row>,
    definitions: HashMap<i32, Definition>,
    definition_serial: usize,
    plugin_serial: i64,
    ordinals: Vec<i32>,
    ordinal_calls: usize,
    lifepoints: HashMap<usize, i32>,
    renderable: Vec<usize>,
    npcs: NodeList<usize>,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    hooks: Vec<Value>,
    lookups: i64,
}

impl World {
    fn record(&mut self, row: Value) -> nr::Result<()> {
        self.events.push(row);
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }
    fn define(&mut self, d: &Value) {
        let id = int(&d["id"]);
        if d["absent"] == true {
            self.definitions.remove(&id);
            return;
        }
        let label = format!("d{}", self.definition_serial);
        self.definition_serial += 1;
        self.definitions.insert(
            id,
            Definition {
                label,
                name: d["name"].as_str().map(str::to_owned),
                size: int(&d["size"]),
            },
        );
    }
    /// The chunk base the stand-in's region chunk records.
    fn chunk_base(l: &Location) -> Value {
        let [x, y, z] = l.coordinates();
        let region = ((x >> 6) << 8) | (y >> 6);
        let (cx, cy) = ((x & 63) >> 3, (y & 63) >> 3);
        json!([
            ((region >> 8) << 6) + (cx << 3),
            ((region & 0xff) << 6) + (cy << 3),
            z
        ])
    }
    fn state(&self, i: usize) -> Value {
        let r = &self.rows[i];
        let mut m = json!({
            "kind": if r.kind == Kind::Player { "player" } else { "npc" },
            "name": r.name, "location": loc_value(r.location.as_ref()), "index": r.index, "size": r.size,
            "active": r.active, "direction": r.direction.map(direction_name), "plugin": r.plugin,
            "lifepoints": self.lifepoints.get(&i).copied().unwrap_or(0),
            "renderable": self.renderable.contains(&i),
        });
        if r.kind == Kind::Npc {
            m["id"] = json!(r.id);
            m["originalId"] = json!(r.original_id);
            m["definition"] = json!(r.definition.as_ref().map(|d| d.label.clone()));
            m["behavior"] = json!(r.behavior);
        }
        m
    }
}

fn e(i: &usize) -> String {
    format!("n{i}")
}

impl Host for World {
    type Entity = usize;
    type Definition = Definition;
    fn kind(&self, i: &usize) -> Kind {
        self.rows[*i].kind
    }
    fn location(&self, i: &usize) -> Option<Location> {
        self.rows[*i].location.clone()
    }
    fn set_active(&mut self, i: &usize, active: bool) {
        self.rows[*i].active = active;
    }
    fn set_name(&mut self, i: &usize, name: Option<String>) {
        self.rows[*i].name = name;
    }
    fn set_size(&mut self, i: &usize, size: i32) {
        self.rows[*i].size = size;
    }
    fn set_direction(&mut self, i: &usize, direction: Option<Direction>) {
        self.rows[*i].direction = direction;
    }
    fn id(&self, i: &usize) -> i32 {
        self.rows[*i].id
    }
    fn set_id(&mut self, i: &usize, id: i32) {
        self.rows[*i].id = id;
    }
    fn original_id(&self, i: &usize) -> i32 {
        self.rows[*i].original_id
    }
    fn set_original_id(&mut self, i: &usize, id: i32) {
        self.rows[*i].original_id = id;
    }
    fn definition(&self, i: &usize) -> Option<Definition> {
        self.rows[*i].definition.clone()
    }
    fn set_definition(&mut self, i: &usize, definition: Option<Definition>) {
        self.rows[*i].definition = definition;
    }
    fn definition_name(&self, d: &Definition) -> Option<String> {
        d.name.clone()
    }
    fn definition_size(&self, d: &Definition) -> i32 {
        d.size
    }
    fn has_behavior(&self, i: &usize) -> bool {
        self.rows[*i].behavior.is_some()
    }
    fn definition_for_id(&mut self, id: i32) -> nr::Result<Option<Definition>> {
        self.record(json!(["NPCDefinition.forId", id]))?;
        let found = self.definitions.get(&id).cloned();
        // A case may change the table right after its n-th lookup answered.
        self.lookups += 1;
        for hook in self.hooks.clone() {
            if hook["at"].as_i64() == Some(self.lookups) {
                self.define(&hook["def"]);
            }
        }
        Ok(found)
    }
    fn allocate(&mut self, name: Option<String>, location: Option<Location>) -> nr::Result<usize> {
        // The entity exists once its constructor ran, whether or not the
        // recorded call then fails.
        self.rows.push(Row {
            kind: Kind::Npc,
            name: name.clone(),
            location: location.clone(),
            index: 0,
            size: 1,
            active: true,
            direction: None,
            plugin: None,
            id: 0,
            original_id: 0,
            definition: None,
            behavior: None,
        });
        let made = self.rows.len() - 1;
        if let Err(error) =
            self.record(json!(["Entity.<init>", name, loc_value(location.as_ref())]))
        {
            self.rows.pop();
            return Err(error);
        }
        Ok(made)
    }
    fn new_interact_plugin(&mut self, i: &usize) -> nr::Result<()> {
        let serial = self.plugin_serial;
        self.plugin_serial += 1;
        self.record(json!(["InteractPlugin.<init>", e(i), serial]))?;
        self.rows[*i].plugin = Some(serial);
        Ok(())
    }
    fn behavior_for_id(&mut self, i: &usize, id: i32) -> nr::Result<()> {
        self.record(json!(["NPCBehavior.forId", id]))?;
        self.rows[*i].behavior = Some(id);
        Ok(())
    }
    fn init_config(&mut self, i: &usize) -> nr::Result<()> {
        self.record(json!(["NPC.initConfig", e(i)]))
    }
    fn configure(&mut self, i: &usize) -> nr::Result<()> {
        self.record(json!(["NPC.configure", e(i)]))
    }
    fn interact_default(&mut self, i: &usize) -> nr::Result<()> {
        let serial = self.rows[*i].plugin;
        self.record(json!(["InteractPlugin.setDefault", e(i), serial]))
    }
    fn type_swap_ordinal(&mut self) -> nr::Result<i32> {
        self.record(json!(["EntityFlags.getOrdinal", "NPC", "TypeSwap"]))?;
        if self.ordinals.is_empty() {
            return Ok(5);
        }
        let at = self.ordinal_calls.min(self.ordinals.len() - 1);
        self.ordinal_calls += 1;
        Ok(self.ordinals[at])
    }
    fn unregister_synced(&mut self, i: &usize, ordinal: i32) -> nr::Result<()> {
        self.record(json!(["UpdateMasks.unregisterSynced", e(i), ordinal]))
    }
    fn register_type_swap(&mut self, i: &usize, id: i32, sync: bool) -> nr::Result<()> {
        self.record(json!(["UpdateMasks.register", e(i), "TypeSwap", id, sync]))
    }
    fn lifepoints(&mut self, i: &usize) -> nr::Result<i32> {
        self.record(json!(["Skills.getLifepoints", e(i)]))?;
        Ok(self.lifepoints.get(i).copied().unwrap_or(0))
    }
    fn set_lifepoints(&mut self, i: &usize, lifepoints: i32) -> nr::Result<()> {
        self.record(json!(["Skills.setLifepoints", e(i), lifepoints]))?;
        self.lifepoints.insert(*i, lifepoints);
        Ok(())
    }
    fn add_auto_timers(&mut self, i: &usize) -> nr::Result<()> {
        self.record(json!(["TimerRegistry.addAutoTimers", e(i)]))
    }
    fn chunk_remove_player(&mut self, i: &usize, l: &Location) -> nr::Result<()> {
        self.record(json!([
            "RegionChunk.removePlayer",
            Self::chunk_base(l),
            e(i)
        ]))
    }
    fn tolerances_remove(&mut self, i: &usize, l: &Location) -> nr::Result<()> {
        let [x, y, _] = l.coordinates();
        let name = self.rows[*i].name.clone();
        self.record(json!([
            "Region.tolerances.remove",
            ((x >> 6) << 8) | (y >> 6),
            name
        ]))
    }
    fn chunk_remove_npc(&mut self, i: &usize, l: &Location) -> nr::Result<()> {
        self.record(json!(["RegionChunk.remove", Self::chunk_base(l), e(i)]))
    }
    fn pulse_manager_clear(&mut self, i: &usize) -> nr::Result<()> {
        self.record(json!(["PulseManager.clear", e(i)]))
    }
    fn remove_renderable(&mut self, i: &usize) -> nr::Result<()> {
        self.record(json!(["Repository.removeRenderableNPC", e(i)]))?;
        self.renderable.retain(|n| n != i);
        Ok(())
    }
    fn repository_remove(&mut self, i: &usize) -> nr::Result<()> {
        self.record(json!(["Repository.npcs.remove", e(i)]))?;
        self.npcs.remove(Some(i));
        Ok(())
    }
    fn behavior_removed(&mut self, i: &usize) -> nr::Result<()> {
        let id = self.rows[*i].behavior;
        self.record(json!(["NPCBehavior.onRemoval", id, e(i)]))
    }
}

/// An operation's failure: the library's, or the oracle's own lookups (a
/// missing entity, a player given to an NPC operation).
enum Fail {
    Lib(Error),
    Index,
}
impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Self::Lib(e)
    }
}

fn entity(w: &World, v: &Value) -> Result<usize, Fail> {
    let i = v.as_u64().unwrap() as usize;
    if i < w.rows.len() {
        Ok(i)
    } else {
        Err(Fail::Index)
    }
}

fn npc(w: &World, v: &Value) -> Result<usize, Fail> {
    let i = entity(w, v)?;
    if w.rows[i].kind == Kind::Npc {
        Ok(i)
    } else {
        Err(Fail::Index)
    }
}

fn op(w: &mut World, o: &[Value]) -> Result<Value, Fail> {
    Ok(match o[0].as_str().unwrap() {
        "new1" => json!(e(&nr::new_npc(w, int(&o[1]))?)),
        "new2" => json!(e(&nr::new_npc_at(w, int(&o[1]), place(&o[2]))?)),
        "new3" => {
            let direction = o[3].as_str().map(|name| {
                Direction::ALL
                    .into_iter()
                    .find(|d| direction_name(*d) == name)
                    .unwrap()
            });
            json!(e(&nr::construct(w, int(&o[1]), place(&o[2]), direction)?))
        }
        "player" => {
            // `new Player(name, location)`: the stand-in's `super(name, location)`.
            let i = w.allocate(o[1].as_str().map(str::to_owned), place(&o[2]))?;
            w.rows[i].kind = Kind::Player;
            json!(e(&i))
        }
        "entityInit" => {
            let i = entity(w, &o[1])?;
            nr::entity_init(w, &i)?;
            Value::Null
        }
        "clear" => {
            let i = entity(w, &o[1])?;
            if w.rows[i].kind == Kind::Npc {
                nr::clear(w, &i)?;
            } else {
                nr::entity_clear(w, &i)?;
            }
            Value::Null
        }
        "transform" => {
            let i = npc(w, &o[1])?;
            nr::transform(w, &i, int(&o[2]))?;
            json!(e(&i))
        }
        "transformHp" => {
            let i = npc(w, &o[1])?;
            nr::transform_with_hp_carryover(w, &i, int(&o[2]))?;
            json!(e(&i))
        }
        "reTransform" => {
            let i = npc(w, &o[1])?;
            nr::re_transform(w, &i)?;
            Value::Null
        }
        "add" => {
            let i = npc(w, &o[1])?;
            w.record(json!(["Repository.npcs.add", e(&i)]))?;
            match w.npcs.add(i) {
                Some(slot) => {
                    w.rows[i].index = slot;
                    json!(true)
                }
                None => json!(false),
            }
        }
        "renderable" => {
            let i = npc(w, &o[1])?;
            w.record(json!(["Repository.addRenderableNPC", e(&i)]))?;
            if !w.renderable.contains(&i) {
                w.renderable.push(i);
            }
            Value::Null
        }
        "move" => {
            let i = entity(w, &o[1])?;
            w.rows[i].location = place(&o[2]);
            Value::Null
        }
        "def" => {
            w.define(&o[1]);
            Value::Null
        }
        "lifepoints" => {
            let i = entity(w, &o[1])?;
            w.lifepoints.insert(i, int(&o[2]));
            Value::Null
        }
        "behavior" => {
            let i = npc(w, &o[1])?;
            w.rows[i].behavior = None;
            Value::Null
        }
        other => panic!("operation {other}"),
    })
}

fn replay(row: &Value) -> Value {
    let capacity = row.get("capacity").map_or(8, int);
    let mut w = World {
        rows: vec![],
        definitions: HashMap::new(),
        definition_serial: 0,
        plugin_serial: 0,
        ordinals: row
            .get("ordinals")
            .map_or(vec![], |a| a.as_array().unwrap().iter().map(int).collect()),
        ordinal_calls: 0,
        lifepoints: HashMap::new(),
        renderable: vec![],
        npcs: NodeList::new(capacity).unwrap(),
        events: vec![],
        calls: 0,
        fail_at: row.get("failAt").and_then(Value::as_i64).unwrap_or(-1),
        hooks: row
            .get("hooks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        lookups: 0,
    };
    for d in row["defs"].as_array().unwrap() {
        w.define(d);
    }
    let mut steps = vec![];
    for o in row["ops"].as_array().unwrap() {
        let before = w.events.len();
        let mut step = serde_json::Map::new();
        match op(&mut w, o.as_array().unwrap()) {
            Ok(v) => {
                step.insert("result".into(), v);
            }
            Err(f) => {
                step.insert(
                    "error".into(),
                    json!(match f {
                        Fail::Lib(Error::Null) => "Null",
                        Fail::Lib(Error::Binding(_)) => "Binding",
                        Fail::Index => "Index",
                    }),
                );
            }
        }
        step.insert("events".into(), json!(w.events[before..]));
        let all: Vec<_> = (0..w.rows.len()).map(|i| w.state(i)).collect();
        step.insert("entities".into(), json!(all));
        let slots: Vec<_> = (1..=capacity)
            .map(|i| w.npcs.get(i).unwrap().map_or(Value::Null, |n| json!(e(&n))))
            .collect();
        step.insert("repository".into(), json!(slots));
        step.insert("repositorySize".into(), json!(w.npcs.size()));
        steps.push(Value::Object(step));
    }
    json!({"steps": steps, "calls": w.calls})
}

#[test]
fn original_runtime_npc_members_match() {
    let mut count = 0;
    for (n, line) in FIXTURE.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let got = replay(&row);
        if got != row["expected"] {
            let want = row["expected"]["steps"].as_array().unwrap();
            let have = got["steps"].as_array().unwrap();
            for (i, (a, b)) in want.iter().zip(have).enumerate() {
                assert_eq!(b, a, "case {n} {} step {i} {}", row["name"], row["ops"][i]);
            }
            assert_eq!(got, row["expected"], "case {n} {}", row["name"]);
        }
        count += 1;
    }
    assert_eq!(count, 912);
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
