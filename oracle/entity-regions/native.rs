//! Test-only region/chunk/entity/activity/clock services around production kernels.
use rs09_server::{
    definitions::item::Shared,
    world::{
        chunk::{HandleChunk, RegionChunk},
        doors::Location,
        entity_regions::{self as spatial, Context, Error, Kind, Result, Select},
        player_walk::{NodeHandle, State},
        region,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
struct Row {
    handle: NodeHandle,
    kind: Kind,
    name: String,
}
struct Model {
    input: Value,
    state: State,
    nodes: BTreeMap<i32, Row>,
    chunks: BTreeMap<String, HandleChunk>,
    regions: BTreeMap<i32, region::Handle>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: BTreeSet<usize>,
    clock: usize,
}
fn loc(v: &Value) -> Option<Location> {
    v.as_array().map(|a| {
        Location::new(
            a[0].as_i64().unwrap() as i32,
            a[1].as_i64().unwrap() as i32,
            a[2].as_i64().unwrap() as i32,
        )
    })
}
fn coord(l: &Option<Location>) -> Value {
    json!(l.as_ref().map(|l| l.coordinates()))
}
fn num(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn key(l: &Location) -> String {
    let [x, y, z] = l.coordinates();
    format!("{},{},{}", x >> 3, y >> 3, z)
}
impl Model {
    fn id(&self, h: &NodeHandle) -> i32 {
        self.nodes
            .iter()
            .find(|(_, r)| r.handle == *h)
            .unwrap()
            .0
            .to_owned()
    }
    fn handle(&self, a: &Value) -> Option<NodeHandle> {
        a.as_i64().map(|id| self.nodes[&(id as i32)].handle.clone())
    }
    fn raw_chunk(&mut self, l: Location) -> HandleChunk {
        let [x, y, z] = l.coordinates();
        self.chunks
            .entry(key(&l))
            .or_insert_with(|| {
                Shared::new(RegionChunk::new(
                    Location::new((x >> 3) << 3, (y >> 3) << 3, z),
                    0,
                ))
            })
            .clone()
    }
    fn raw_region(&mut self, l: Location) -> region::Handle {
        let [x, y, _] = l.coordinates();
        self.regions
            .entry(((x >> 6) << 8) | (y >> 6))
            .or_insert_with(|| Shared::new(region::Region::new(x >> 6, y >> 6)))
            .clone()
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut e = vec![json!(event)];
        e.extend(args);
        self.events.push(json!(e));
        let n = self.calls.entry(event.into()).or_default();
        *n += 1;
        let n = *n;
        let hs = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for (i, h) in hs.iter().enumerate() {
            if h["event"] == event
                && h["at"].as_i64().unwrap_or(1) == n as i64
                && self.hooks.insert(i)
            {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.input["failures"][event].as_i64() == Some(n as i64) {
            return Err(Error::Binding("Failure".into()));
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let node = self.handle(&a["node"]);
        match a["op"].as_str().unwrap() {
            "move" => {
                spatial::move_entity(
                    self,
                    &node.ok_or(Error::Null)?,
                    loc(&a["src"]),
                    loc(&a["dst"]).ok_or(Error::Null)?,
                )?;
            }
            "query" => {
                let select = match a["select"].as_str().unwrap_or("all") {
                    "players" => Select::Players,
                    "npcs" => Select::Npcs,
                    _ => Select::All,
                };
                let r = spatial::local(
                    self,
                    loc(&a["location"]).ok_or(Error::Null)?,
                    num(&a["distance"]),
                    select,
                )?;
                return Ok(json!(r.iter().map(|n| self.id(n)).collect::<Vec<_>>()));
            }
            "add" | "remove" => {
                let add = a["op"] == "add";
                let c = self.raw_chunk(loc(&a["chunk"]).ok_or(Error::Null)?);
                let players = a["list"] == "players";
                if a["raw"] == true {
                    let list = if players {
                        c.read().players.clone()
                    } else {
                        c.read().npcs.clone()
                    };
                    if add {
                        list.add(node);
                    } else {
                        list.remove(&node);
                    }
                } else if players {
                    if add {
                        spatial::add_player(self, &c, node)?;
                    } else {
                        spatial::remove_player(self, &c, node)?;
                    }
                } else if add {
                    spatial::add_npc(&c, node);
                } else {
                    spatial::remove_npc(&c, node);
                }
            }
            "clear" => {
                let c = self.raw_chunk(loc(&a["chunk"]).ok_or(Error::Null)?);
                if a["list"] == "players" {
                    c.read().players.clear();
                } else {
                    c.read().npcs.clear();
                }
            }
            "set" => {
                let h = node.ok_or(Error::Null)?;
                let id = self.id(&h);
                let q = self.state.actor(h.node).unwrap().queue.clone();
                if a.get("location").is_some() {
                    q.state.write().entity.location = loc(&a["location"]);
                }
                if a.get("footprint").is_some() {
                    q.state.write().foot_print = loc(&a["footprint"]);
                }
                if let Some(s) = a["name"].as_str() {
                    self.nodes.get_mut(&id).unwrap().name = s.into();
                }
            }
            "rebase" => {
                self.raw_chunk(loc(&a["chunk"]).ok_or(Error::Null)?)
                    .write()
                    .current_base = loc(&a["base"]).ok_or(Error::Null)?
            }
            "tolerance" => {
                self.raw_region(loc(&a["region"]).ok_or(Error::Null)?)
                    .read()
                    .tolerances
                    .write()
                    .insert(a["name"].as_str().unwrap().into(), a["value"].as_i64());
            }
            "tolerated" => {
                let r = self.raw_region(loc(&a["region"]).ok_or(Error::Null)?);
                return Ok(json!(spatial::is_tolerated(
                    self,
                    &r,
                    &node.ok_or(Error::Null)?
                )?));
            }
            _ => return Err(Error::Binding("unknown action".into())),
        }
        Ok(Value::Null)
    }
}
impl Context for Model {
    fn chunk(&mut self, l: Location) -> Result<HandleChunk> {
        let c = self.raw_chunk(l.clone());
        self.emit("chunk", vec![json!(l.coordinates())])?;
        Ok(c)
    }
    fn region(&mut self, l: Location) -> Result<region::Handle> {
        let r = self.raw_region(l.clone());
        self.emit("region", vec![json!(l.coordinates())])?;
        Ok(r)
    }
    fn kind(&mut self, h: &NodeHandle) -> Result<Kind> {
        Ok(self.nodes[&self.id(h)].kind)
    }
    fn location(&mut self, h: &NodeHandle) -> Result<Option<Location>> {
        let l = self.state.actor(h.node).unwrap().queue.entity().location;
        let id = self.id(h);
        self.emit("location", vec![json!(id)])?;
        Ok(l)
    }
    fn as_player(&mut self, h: &NodeHandle) -> Result<NodeHandle> {
        let id = self.id(h);
        self.emit("asPlayer", vec![json!(id)])?;
        Ok(h.clone())
    }
    fn name(&mut self, h: &NodeHandle) -> Result<String> {
        let id = self.id(h);
        let n = self.nodes[&id].name.clone();
        self.emit("name", vec![json!(id)])?;
        Ok(n)
    }
    fn flag_active(&mut self, r: region::Handle) -> Result<()> {
        let id = r.read().id();
        self.emit("active", vec![json!(id)])
    }
    fn check_inactive(&mut self, r: region::Handle) -> Result<()> {
        let id = r.read().id();
        self.emit("inactive", vec![json!(id)])
    }
    fn footprint(&mut self, h: &NodeHandle) -> Result<Option<Location>> {
        let l = self.state.actor(h.node).unwrap().queue.foot_print();
        let id = self.id(h);
        self.emit("footprint", vec![json!(id)])?;
        Ok(l)
    }
    fn zone_update(&mut self, h: &NodeHandle, l: Option<Location>) -> Result<()> {
        let id = self.id(h);
        self.emit("zone", vec![json!(id), coord(&l)])
    }
    fn unknown_entity(&mut self, _: &NodeHandle) -> Result<()> {
        self.emit("unknown", vec![])
    }
    fn now_millis(&mut self) -> Result<i64> {
        let a = self.input["clock"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![json!(1000)]);
        let n = a[self.clock.min(a.len() - 1)].as_i64().unwrap();
        self.clock += 1;
        self.emit("clock", vec![json!(n)])?;
        Ok(n)
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        state: State::default(),
        nodes: BTreeMap::new(),
        chunks: BTreeMap::new(),
        regions: BTreeMap::new(),
        events: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        clock: 0,
    };
    for a in input["nodes"].as_array().unwrap() {
        let id = num(&a["id"]);
        let kind = match a["kind"].as_str().unwrap_or("npc") {
            "player" => Kind::Player,
            "npc" => Kind::Npc,
            _ => Kind::Other,
        };
        let node = m.state.add_npc_at(id, loc(&a["location"]), [1, 2, 3, 4]);
        if let Some(f) = a.get("footprint") {
            m.state.actor(node).unwrap().queue.state.write().foot_print = loc(f);
        }
        let handle = m.state.retain_node(node).unwrap();
        m.nodes.insert(
            id,
            Row {
                handle,
                kind,
                name: a["name"]
                    .as_str()
                    .map_or_else(|| format!("p{id}"), str::to_owned),
            },
        );
    }
    let results:Vec<_>=input["actions"].as_array().unwrap().iter().map(|a|match m.act(a){Ok(v)=>json!({"value":v}),Err(e)=>json!({"error":match e{Error::Null=>"Null".into(),Error::Limit=>"Limit".into(),Error::ConcurrentModification=>"ConcurrentModification".into(),Error::Binding(s)=>s}})}).collect();
    let chunks:BTreeMap<_,_>=m.chunks.iter().map(|(key,c)|{let ids=|list:super_list::EntityList|list.values().iter().map(|n|n.as_ref().map(|h|m.id(h))).collect::<Vec<_>>();(key.clone(),json!({"players":ids(c.read().players.clone()),"npcs":ids(c.read().npcs.clone()),"base":c.read().current_base.coordinates()}))}).collect();
    let regions: BTreeMap<_, _> = m
        .regions
        .iter()
        .map(|(id, r)| (id.to_string(), json!(*r.read().tolerances.read())))
        .collect();
    let nodes:BTreeMap<_,_>=m.nodes.iter().map(|(id,r)|{let q=&m.state.actor(r.handle.node).unwrap().queue;(id.to_string(),json!({"name":r.name,"location":coord(&q.entity().location),"footprint":coord(&q.foot_print())}))}).collect();
    json!({"results":results,"events":m.events,"chunks":chunks,"regions":regions,"nodes":nodes})
}
mod super_list {
    pub type EntityList =
        rs09_server::world::update::List<Option<rs09_server::world::player_walk::NodeHandle>>;
}
