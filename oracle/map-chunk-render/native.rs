//! Test-only routing/player/activity/packet services for the original renderer.
use rs09_server::{
    definitions::item::Shared,
    world::{
        chunk::{HandleChunk, RegionChunk},
        doors::Location,
        map_chunk_render::{self as viewport, Context, Error, Result, Viewport},
        player_walk::{NodeHandle, State},
        region,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
struct RegionRow {
    handle: region::Handle,
    active: bool,
    viewers: Vec<String>,
}
struct Model {
    input: Value,
    location: Location,
    anchor: Option<Location>,
    name: String,
    viewports: Vec<Viewport>,
    current: usize,
    chunks: BTreeMap<String, HandleChunk>,
    all: Vec<HandleChunk>,
    regions: BTreeMap<i32, RegionRow>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: BTreeSet<usize>,
    player: NodeHandle,
    _state: State,
}
fn num(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn loc(v: &Value) -> Location {
    Location::new(num(&v[0]), num(&v[1]), num(&v[2]))
}
fn key(l: &Location) -> String {
    let [x, y, z] = l.coordinates();
    format!("{},{},{}", x >> 3, y >> 3, z)
}
fn rid(l: &Location) -> i32 {
    let [x, y, _] = l.coordinates();
    ((x >> 6) << 8) | (y >> 6)
}
impl Model {
    fn fresh(&mut self, l: Location) -> HandleChunk {
        let [x, y, z] = l.coordinates();
        let c = Shared::new(RegionChunk::new(
            Location::new((x >> 3) << 3, (y >> 3) << 3, z),
            0,
        ));
        self.all.push(c.clone());
        self.chunks.insert(key(&l), c.clone());
        c
    }
    fn raw_chunk(&mut self, l: Location) -> HandleChunk {
        self.chunks
            .get(&key(&l))
            .cloned()
            .unwrap_or_else(|| self.fresh(l))
    }
    fn chunk_id(&self, c: &HandleChunk) -> usize {
        self.all.iter().position(|p| p.same(c)).unwrap() + 1
    }
    fn region_id(&self, r: &region::Handle) -> i32 {
        *self
            .regions
            .iter()
            .find(|(_, p)| p.handle.same(r))
            .unwrap()
            .0
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
                && h["at"].as_i64().unwrap_or(1) == i64::from(n)
                && self.hooks.insert(i)
            {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.input["failures"][event].as_i64() == Some(i64::from(n)) {
            return Err(Error::Binding("Failure".into()));
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "render" => {
                let p = self.player.clone();
                viewport::render(self, &p)?;
            }
            "set" => {
                if let Some(v) = a.get("location") {
                    self.location = loc(v);
                }
                if let Some(v) = a.get("anchor") {
                    self.anchor = (!v.is_null()).then(|| loc(v));
                }
                if let Some(v) = a.get("name") {
                    self.name = v.as_str().unwrap().into();
                }
            }
            "replaceChunk" => {
                self.fresh(loc(&a["location"]));
            }
            "alias" => {
                let c = self.raw_chunk(loc(&a["other"]));
                self.chunks.insert(key(&loc(&a["location"])), c);
            }
            "rebase" => self.raw_chunk(loc(&a["location"])).write().current_base = loc(&a["base"]),
            "setRow" => {
                let row = a["length"]
                    .as_u64()
                    .map(|n| Shared::new(vec![None; n as usize]));
                *self.viewports[self.current]
                    .write()
                    .get_mut(num(&a["x"]) as usize)
                    .ok_or(Error::Index)? = row;
            }
            "newViewport" => {
                self.viewports.push(viewport::empty_viewport());
                self.current = self.viewports.len() - 1;
            }
            _ => unreachable!(),
        }
        Ok(Value::Null)
    }
    fn views(&self) -> Value {
        json!(
            self.viewports
                .iter()
                .map(|v| v
                    .read()
                    .iter()
                    .map(|r| r.as_ref().map(|r| r
                        .read()
                        .iter()
                        .map(|c| c.as_ref().map(|c| self.chunk_id(c)))
                        .collect::<Vec<_>>()))
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        )
    }
}
impl Context for Model {
    fn viewport(&mut self, _: &NodeHandle) -> Result<Viewport> {
        self.emit("viewport", vec![])?;
        Ok(self.viewports[self.current].clone())
    }
    fn last_scene(&mut self, _: &NodeHandle) -> Result<Option<Location>> {
        self.emit("viewport", vec![])?;
        let l = self.anchor.clone();
        self.emit("scene", vec![])?;
        Ok(l)
    }
    fn location(&mut self, _: &NodeHandle) -> Result<Location> {
        let l = self.location.clone();
        self.emit("location", vec![])?;
        Ok(l)
    }
    fn chunk(&mut self, l: Location) -> Result<HandleChunk> {
        let c = self.raw_chunk(l.clone());
        self.emit("chunk", vec![json!(l.coordinates())])?;
        Ok(c)
    }
    fn region(&mut self, l: Location) -> Result<region::Handle> {
        let [x, y, _] = l.coordinates();
        let r = self
            .regions
            .entry(rid(&l))
            .or_insert_with(|| RegionRow {
                handle: Shared::new(region::Region::new(x >> 6, y >> 6)),
                active: false,
                viewers: vec![],
            })
            .handle
            .clone();
        self.emit("region", vec![json!(l.coordinates())])?;
        Ok(r)
    }
    fn clear_chunk(&mut self, _: &NodeHandle, c: &HandleChunk) -> Result<()> {
        self.emit("clear", vec![json!(self.chunk_id(c))])
    }
    fn update_chunk(&mut self, _: &NodeHandle, c: &HandleChunk) -> Result<()> {
        self.emit("update", vec![json!(self.chunk_id(c))])
    }
    fn synchronize_chunk(&mut self, _: &NodeHandle, c: &HandleChunk) -> Result<()> {
        self.emit("sync", vec![json!(self.chunk_id(c))])
    }
    fn name(&mut self, _: &NodeHandle) -> Result<String> {
        let n = self.name.clone();
        self.emit("name", vec![])?;
        Ok(n)
    }
    fn decrement_view(&mut self, r: &region::Handle, name: String) -> Result<()> {
        let id = self.region_id(r);
        let names = &mut self.regions.get_mut(&id).unwrap().viewers;
        if let Some(i) = names.iter().position(|n| n == &name) {
            names.remove(i);
        }
        self.emit("decrement", vec![json!(id), json!(name)])
    }
    fn increment_view(&mut self, r: &region::Handle, name: String) -> Result<()> {
        let id = self.region_id(r);
        let names = &mut self.regions.get_mut(&id).unwrap().viewers;
        if !names.contains(&name) {
            names.push(name.clone());
        }
        self.emit("increment", vec![json!(id), json!(name)])
    }
    fn is_active(&mut self, r: &region::Handle) -> Result<bool> {
        let id = self.region_id(r);
        let a = self.regions[&id].active;
        self.emit("active", vec![json!(id), json!(a)])?;
        Ok(a)
    }
    fn check_inactive(&mut self, r: &region::Handle) -> Result<()> {
        let id = self.region_id(r);
        self.emit("inactive", vec![json!(id)])
    }
    fn flag_active(&mut self, r: &region::Handle) -> Result<()> {
        let id = self.region_id(r);
        self.emit("flag", vec![json!(id)])?;
        self.regions.get_mut(&id).unwrap().active = true;
        Ok(())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let location = input
        .get("location")
        .map(loc)
        .unwrap_or_else(|| Location::new(3207, 3216, 0));
    let mut state = State::default();
    let node = state.add(1, location.clone(), [1, 2, 3, 4]);
    let player = state.retain_node(node).unwrap();
    let mut m = Model {
        input: input.clone(),
        location,
        anchor: input.get("anchor").filter(|v| !v.is_null()).map(loc),
        name: "viewer".into(),
        viewports: vec![viewport::empty_viewport()],
        current: 0,
        chunks: BTreeMap::new(),
        all: vec![],
        regions: BTreeMap::new(),
        events: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        player,
        _state: state,
    };
    let mut results = vec![];
    for a in input["actions"].as_array().unwrap() {
        let mut result = match m.act(a) {
            Ok(v) => json!({"value":v}),
            Err(e) => {
                json!({"error":match e{Error::Null=>"Null".into(),Error::Index=>"Index".into(),Error::Binding(s)=>s}})
            }
        };
        result["viewports"] = m.views();
        results.push(result);
    }
    let regions: BTreeMap<_, _> = m
        .regions
        .iter()
        .map(|(id, r)| {
            (
                id.to_string(),
                json!({"active":r.active,"viewers":r.viewers}),
            )
        })
        .collect();
    json!({"results":results,"events":m.events,"chunks":m.all.iter().map(|c|c.read().current_base.coordinates()).collect::<Vec<_>>(),"regions":regions,"location":m.location.coordinates(),"anchor":m.anchor.as_ref().map(Location::coordinates),"name":m.name})
}
