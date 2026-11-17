//! Explicit player, clock, constructor, clipping, session and foreign flag adapters.
use rs09_server::{
    definitions::item::{self as definitions, Shared},
    inventory::item,
    world::{
        chunk::{HandleChunk, RegionChunk},
        chunk_sync::{self as sync, Buffer, Context, DirtyTracker, Error, Flag, Items, Result},
        doors::{Handle as Object, Location, Scenery},
        ground_item::{self as ground, Handle as Ground},
        player_walk::{NodeHandle, State},
        scenery::ObjectUpdate,
        scenery_node::Layer,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[path = "../ground-action/native.rs"]
mod ground_action;
#[path = "../ground-spawn-loader/native.rs"]
mod loader;
#[path = "../ground-manager/native.rs"]
mod manager;
#[path = "../pickup/native.rs"]
mod pickup;
#[path = "../ground-spawn/native.rs"]
mod spawn;
struct Player {
    handle: NodeHandle,
    uid: i32,
    name: Option<String>,
    scene: Option<Location>,
    attrs: BTreeMap<String, bool>,
}
struct Model {
    extra: manager::Extra,
    spawns: spawn::Extra,
    loaders: loader::Extra,
    pickup: pickup::Extra,
    ground_action: ground_action::Extra,
    input: Value,
    tick: i32,
    players: Vec<Player>,
    chunks: [HandleChunk; 2],
    grounds: Vec<Ground>,
    objects: Vec<Object>,
    flags: Vec<Shared<Flag>>,
    foreign: BTreeMap<usize, i32>,
    held: Vec<Buffer>,
    sent: Vec<(usize, Buffer)>,
    item_lists: Vec<Items>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: BTreeSet<usize>,
    dirty: DirtyTracker,
    defs: definitions::Registry,
    _state: State,
}
fn num(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v.get(k).map_or(d, num)
}
fn loc(v: &Value) -> Option<Location> {
    v.as_array()
        .map(|a| Location::new(num(&a[0]), num(&a[1]), num(&a[2])))
}
fn err(e: Error) -> String {
    match e {
        Error::Null | Error::Ground(ground::Error::Null) => "Null".into(),
        Error::Binding(s) | Error::Ground(ground::Error::Binding(s)) => s,
    }
}
fn as_ground(e: Error) -> ground::Error {
    match e {
        Error::Ground(e) => e,
        Error::Null => ground::Error::Null,
        Error::Binding(s) => ground::Error::Binding(s),
    }
}
fn buffer(b: &Buffer) -> Value {
    let b = b.read();
    json!({"opcode":b.opcode,"hex":b.payload.iter().map(|v|format!("{v:02x}")).collect::<String>()})
}
impl Model {
    fn p(&self, a: &Value) -> Option<NodeHandle> {
        if a.get("p").is_some_and(Value::is_null) {
            None
        } else {
            Some(self.players[n(a, "p", 0) as usize].handle.clone())
        }
    }
    fn pi(&self, p: &NodeHandle) -> usize {
        self.players.iter().position(|v| v.handle == *p).unwrap()
    }
    fn c(&self, a: &Value) -> HandleChunk {
        self.chunks[n(a, "c", 0) as usize].clone()
    }
    fn g(&self, a: &Value) -> Result<Ground> {
        self.grounds
            .get(num(&a["g"]) as usize)
            .cloned()
            .ok_or_else(|| Error::Binding("Index".into()))
    }
    fn gid(&self, g: &Ground) -> usize {
        self.grounds.iter().position(|v| v.same(g)).unwrap()
    }
    fn flag_id(&self, f: &Shared<Flag>) -> usize {
        self.flags.iter().position(|v| v.same(f)).unwrap()
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
            "newGround" => {
                let input = ground::Context::copy_item(
                    self,
                    n(a, "id", 1),
                    n(a, "amount", 1),
                    n(a, "charge", 1000),
                )?;
                let l = loc(&a["location"]);
                let ctor = a["ctor"].as_str().unwrap_or("owner");
                let p = self.p(a);
                let g = match ctor {
                    "item" => ground::construct(self, &input, None, 200, None)?,
                    "location" => ground::construct(self, &input, l, 200, None)?,
                    "uid" => ground::construct_uid(
                        self,
                        &input,
                        l,
                        n(a, "uid", 7),
                        n(a, "absolute", 200),
                    )?,
                    "decay" => ground::construct(self, &input, l, n(a, "decay", 200), p)?,
                    _ => ground::construct(self, &input, l, 200, p)?,
                };
                return Ok(json!(self.gid(&g)));
            }
            "setGround" => {
                let g = self.g(a)?;
                let mut g = g.write();
                if let Some(v) = a.get("location") {
                    g.location = loc(v);
                }
                if let Some(v) = a.get("removed") {
                    g.removed = v.as_bool().unwrap();
                }
                if let Some(v) = a.get("remainPrivate") {
                    g.remain_private = v.as_bool().unwrap();
                }
                if let Some(v) = a.get("forceVisible") {
                    g.force_visible = v.as_bool().unwrap();
                }
                if let Some(v) = a.get("decay") {
                    g.decay_time = num(v);
                }
            }
            "itemPrivate" => {
                let g = self.g(a)?;
                return Ok(json!(ground::is_private(self, &g)?));
            }
            "itemActive" => {
                let g = self.g(a)?;
                return Ok(json!(ground::is_active(self, &g)?));
            }
            "droppedBy" => {
                let g = self.g(a)?;
                let p = self.p(a).ok_or(Error::Null)?;
                return Ok(json!(ground::dropped_by(self, &g, &p)?));
            }
            "string" => {
                let g = self.g(a)?;
                return Ok(json!(ground::java_string(self, &g)?));
            }
            "setDecay" => {
                let g = self.g(a)?;
                ground::set_decay_time(self, &g, num(&a["delay"]))?;
            }
            "tick" => self.tick = num(&a["value"]),
            "player" => {
                let p = self.p(a).ok_or(Error::Null)?;
                let i = self.pi(&p);
                if let Some(v) = a.get("uid") {
                    self.players[i].uid = num(v);
                }
                if let Some(v) = a.get("scene") {
                    self.players[i].scene = loc(v);
                }
            }
            "attr" => {
                let p = self.p(a).ok_or(Error::Null)?;
                let i = self.pi(&p);
                self.players[i].attrs.insert(
                    a["name"].as_str().unwrap().into(),
                    a["value"].as_bool().unwrap(),
                );
            }
            "base" => self.c(a).write().current_base = loc(&a["location"]).ok_or(Error::Null)?,
            "newObject" => {
                let mut o = Scenery::new(
                    n(a, "id", 1),
                    loc(&a["location"]).ok_or(Error::Null)?,
                    n(a, "shape", 0),
                    n(a, "rot", 0),
                    None,
                    false,
                )
                .unwrap();
                o.renderable = a["renderable"].as_bool().unwrap_or(true);
                self.objects.push(Shared::new(o));
                return Ok(json!(self.objects.len() - 1));
            }
            "setObject" => {
                let o = self.objects[num(&a["o"]) as usize].clone();
                let mut o = o.write();
                if let Some(v) = a.get("renderable") {
                    o.renderable = v.as_bool().unwrap();
                }
                if let Some(v) = a.get("location") {
                    o.location = loc(v).ok_or(Error::Null)?;
                }
            }
            "place" => {
                let c = self.c(a);
                let array = if a["array"] == "stat" {
                    c.read().static_objects.clone()
                } else {
                    c.read().objects.clone()
                };
                array.write()[n(a, "x", 1) as usize][n(a, "y", 2) as usize]
                    [n(a, "i", 0) as usize] =
                    a["o"].as_i64().map(|i| self.objects[i as usize].clone());
            }
            "setItems" => {
                let c = self.c(a);
                let items = if let Some(array) = a["items"].as_array() {
                    let mut out = vec![];
                    for v in array {
                        out.push(if v.is_null() {
                            None
                        } else {
                            Some(
                                self.grounds
                                    .get(num(v) as usize)
                                    .cloned()
                                    .ok_or_else(|| Error::Binding("Index".into()))?,
                            )
                        });
                    }
                    Some(Shared::new(out.into()))
                } else {
                    None
                };
                c.write().items = items;
            }
            "captureItems" => {
                self.item_lists.push(sync::get_items(&self.c(a)));
                return Ok(json!(self.item_lists.len() - 1));
            }
            "draw" => {
                let c = self.c(a);
                let p = self.p(a).ok_or(Error::Null)?;
                let items = c.read().items.clone();
                let v = sync::draw_items(self, &c, items, &p)?;
                return Ok(json!(
                    v.iter()
                        .map(|g| g.as_ref().map(|g| self.gid(g)))
                        .collect::<Vec<_>>()
                ));
            }
            "newFlag" => {
                let f = match a["kind"].as_str().unwrap() {
                    "object" => Flag::Object(ObjectUpdate {
                        object: self.objects[num(&a["o"]) as usize].clone(),
                        remove: a["remove"].as_bool().unwrap_or(false),
                    }),
                    "item" => Flag::Item {
                        item: self.g(a)?,
                        kind: num(&a["type"]),
                        old_amount: n(a, "old", 0),
                    },
                    _ => {
                        let id = self.flags.len();
                        self.foreign.insert(id, n(a, "value", 88));
                        Flag::Foreign(id)
                    }
                };
                self.flags.push(Shared::new(f));
                return Ok(json!(self.flags.len() - 1));
            }
            "mutForeign" => {
                let id = num(&a["f"]) as usize;
                let flag = self
                    .flags
                    .get(id)
                    .ok_or_else(|| Error::Binding("Index".into()))?;
                if !matches!(*flag.read(), Flag::Foreign(_)) {
                    return Err(Error::Binding("ClassCastException".into()));
                }
                self.foreign.insert(id, num(&a["value"]));
            }
            "flag" => {
                let c = self.c(a);
                let f = if a["f"].is_null() {
                    None
                } else {
                    Some(
                        self.flags
                            .get(num(&a["f"]) as usize)
                            .cloned()
                            .ok_or_else(|| Error::Binding("Index".into()))?,
                    )
                };
                sync::flag(&c, f, &mut |c| {
                    self.dirty.on_flagged(c);
                    let id = self.chunks.iter().position(|p| p.same(c)).unwrap();
                    self.emit("dirty", vec![json!(id)])
                })?;
            }
            "sync" => {
                let c = self.c(a);
                let p = self.p(a).ok_or(Error::Null)?;
                sync::synchronize(self, &c, &p)?;
            }
            "update" => {
                let c = self.c(a);
                let p = self.p(a).ok_or(Error::Null)?;
                sync::update(self, &c, &p)?;
            }
            "resetAll" => self.dirty.reset_all(),
            "clearChunk" => {
                sync::clear(self, &self.c(a), &self.p(a).ok_or(Error::Null)?)?;
            }
            _ => return manager::act(self, a),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self) -> Value {
        let gs:Vec<_>=self.grounds.iter().map(|h|{let g=h.read();let i=g.item.read();json!({"id":i.id(),"amount":i.amount(),"charge":i.charge(),"index":i.index,"location":g.location.as_ref().map(Location::coordinates),"dropper":g.dropper.as_ref().map(|p|self.pi(p)),"uid":g.dropper_uid,"ticks":g.ticks,"decay":g.decay_time,"removed":g.removed,"remainPrivate":g.remain_private,"forceVisible":g.force_visible})}).collect();
        let ids = |items: &Items| {
            items
                .read()
                .iter()
                .map(|h| h.as_ref().map(|g| self.gid(g)))
                .collect::<Vec<_>>()
        };
        let cs:Vec<_>=self.chunks.iter().map(|c|{let c=c.read();json!({"flags":c.update_flags.read().iter().map(|f|f.as_ref().map(|f|self.flag_id(f))).collect::<Vec<_>>(),"items":c.items.as_ref().map(ids),"base":c.current_base.coordinates()})}).collect();
        let mut state = json!({"grounds":gs,"chunks":cs,"dirty":self.dirty.len(),"sent":self.sent.iter().map(|(p,b)|json!([p,buffer(b)])).collect::<Vec<_>>(),"held":self.held.iter().map(buffer).collect::<Vec<_>>(),"itemLists":self.item_lists.iter().map(ids).collect::<Vec<_>>()});
        if self.input["manager"] == true {
            manager::snapshot(self, &mut state);
        }
        if self.input["pickup"] == true {
            pickup::snapshot(self, &mut state);
        }
        if self.input["groundAction"] == true {
            ground_action::snapshot(self, &mut state);
        }
        if self.input["spawns"] == true {
            spawn::snapshot(self, &mut state);
        }
        if self.input["loader"] == true {
            loader::snapshot(self, &mut state);
        }
        state
    }
}
impl ground::Context for Model {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        if self.input["manager"] == true {
            manager::prepare_definition(self, id);
        }
        let h = Shared::new(item::Item::new(id, amount, charge, &self.defs));
        self.emit("item", vec![json!(id), json!(amount), json!(charge)])
            .map_err(as_ground)?;
        Ok(h)
    }
    fn default_interaction(&mut self, h: &Ground) -> ground::Result<()> {
        let id = self.grounds.len();
        self.grounds.push(h.clone());
        self.emit("default", vec![json!(id)]).map_err(as_ground)
    }
    fn uid(&mut self, p: &NodeHandle) -> ground::Result<i32> {
        let id = self.pi(p);
        let uid = self.players[id].uid;
        self.emit("uid", vec![json!(id), json!(uid)])
            .map_err(as_ground)?;
        Ok(uid)
    }
    fn username(&mut self, p: &NodeHandle) -> ground::Result<Option<String>> {
        let id = self.pi(p);
        let name = self.players[id].name.clone();
        self.emit("username", vec![json!(id)]).map_err(as_ground)?;
        Ok(name)
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        let n = self.tick;
        self.emit("ticks", vec![json!(n)]).map_err(as_ground)?;
        Ok(n)
    }
    fn virtual_private(&mut self, h: &Ground) -> ground::Result<Option<bool>> {
        manager::virtual_private(self, h).map_err(as_ground)
    }
    fn virtual_active(&mut self, h: &Ground) -> ground::Result<Option<bool>> {
        manager::virtual_active(self, h).map_err(as_ground)
    }
}
impl Context for Model {
    fn last_scene(&mut self, p: &NodeHandle) -> Result<Location> {
        let id = self.pi(p);
        let l = self.players[id].scene.clone();
        self.emit("scene", vec![json!(id)])?;
        l.ok_or(Error::Null)
    }
    fn send(&mut self, p: &NodeHandle, b: &Buffer) -> Result<()> {
        let id = self.pi(p);
        self.sent.push((id, b.clone()));
        self.emit("send", vec![json!(id)])
    }
    fn attribute(&mut self, p: &NodeHandle, name: &str) -> Result<bool> {
        let id = self.pi(p);
        let v = *self.players[id].attrs.get(name).unwrap_or(&false);
        self.emit("attr", vec![json!(id), json!(name)])?;
        Ok(v)
    }
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        let n = if x.wrapping_add(y) & 3 == 0 {
            0
        } else {
            1i32 << ((x ^ y) & 15)
        };
        self.emit("clip", vec![json!(z), json!(x), json!(y)])?;
        Ok(n)
    }
    fn layer(&mut self, o: &Object) -> Result<Option<Layer>> {
        let shape = o.read().shape;
        Ok(if shape <= 3 || shape == 9 {
            Some(Layer::Wall)
        } else if shape <= 8 {
            Some(Layer::WallDecor)
        } else if shape <= 21 {
            Some(Layer::Ground)
        } else if shape == 22 {
            Some(Layer::GroundDecor)
        } else {
            self.emit(
                "invalidLayer",
                vec![json!(format!("Invalid scenery type {shape}"))],
            )?;
            None
        })
    }
    fn foreign_flag(&mut self, id: usize, b: &Buffer, p: &NodeHandle) -> Result<()> {
        self.held.push(b.clone());
        let player = self.pi(p);
        self.emit("foreign", vec![json!(id), json!(player)])?;
        b.write().payload.push(self.foreign[&id] as u8);
        Ok(())
    }
}
fn model(input: &Value) -> Model {
    let mut state = State::default();
    let mut players = vec![];
    for i in 0..2 {
        let l = Location::new(3207, 3216, 0);
        let n = state.add(i, l.clone(), [1, 2, 3, 4]);
        players.push(Player {
            handle: state.retain_node(n).unwrap(),
            uid: 7 + i * 2,
            name: Some(format!("p{i}")),
            scene: Some(l),
            attrs: BTreeMap::new(),
        });
    }
    let mut m = Model {
        extra: manager::Extra::default(),
        spawns: spawn::Extra::default(),
        loaders: loader::Extra::default(),
        pickup: pickup::Extra::default(),
        ground_action: ground_action::Extra::default(),
        input: input.clone(),
        tick: n(input, "tick", 0),
        players,
        chunks: std::array::from_fn(|_| {
            Shared::new(RegionChunk::new(Location::new(3200, 3200, 0), 0))
        }),
        grounds: vec![],
        objects: vec![],
        flags: vec![],
        foreign: BTreeMap::new(),
        held: vec![],
        sent: vec![],
        item_lists: vec![],
        events: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        dirty: DirtyTracker::default(),
        defs: definitions::Registry::default(),
        _state: state,
    };
    manager::initialize(&mut m);
    if input["pickup"] == true {
        pickup::initialize(&mut m);
    }
    m
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = model(input);
    let mut results = vec![];
    for a in input["actions"].as_array().unwrap() {
        let mut result = match m.act(a) {
            Ok(v) => json!({"value":v}),
            Err(e) => json!({"error":err(e)}),
        };
        result["state"] = m.snapshot();
        results.push(result);
    }
    json!({"results":results,"events":m.events})
}
