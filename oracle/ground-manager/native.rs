//! Joined test services for the real manager, canonical chunk lists and packets.
use super::*;
use rs09_server::{
    definitions::property::Property,
    world::ground_manager::{self as gm, Packet},
};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Extra {
    pub manager: gm::Manager,
    lookup: usize,
    players: [PlayerExtra; 2],
    virtuals: HashMap<usize, Virtual>,
    inputs: Vec<item::Handle>,
    arrays: Vec<gm::ItemArray>,
    captures: Vec<Option<Items>>,
    details: BTreeSet<i32>,
}
struct PlayerExtra {
    location: Option<Location>,
    playing: bool,
    artificial: bool,
    items: Option<Items>,
}
impl Default for PlayerExtra {
    fn default() -> Self {
        Self {
            location: None,
            playing: true,
            artificial: false,
            items: None,
        }
    }
}
#[derive(Default)]
struct Virtual {
    private: Option<bool>,
    active: Option<bool>,
    auto: Option<bool>,
}
pub(super) fn initialize(m: &mut Model) {
    for (source, to) in m.players.iter().zip(&mut m.extra.players) {
        to.location = source.scene.clone();
    }
}
pub(super) fn prepare_definition(m: &mut Model, id: i32) {
    if !m.defs.definitions().contains_key(&id) {
        let mut d = definitions::ItemDefinition {
            id,
            ..Default::default()
        };
        d.handlers.insert("tradeable".into(), true.into());
        m.defs.insert(id, Shared::new(d));
    }
}
pub(super) fn error(e: gm::Error) -> Error {
    match e {
        gm::Error::Null => Error::Null,
        gm::Error::Index => Error::Binding("Index".into()),
        gm::Error::Type => Error::Binding("ClassCastException".into()),
        gm::Error::ConcurrentModification => {
            Error::Binding("ConcurrentModificationException".into())
        }
        gm::Error::Ground(e) => Error::Ground(e),
        gm::Error::Binding(s) => Error::Binding(s),
    }
}
fn gm_error(e: Error) -> gm::Error {
    match e {
        Error::Null => gm::Error::Null,
        Error::Ground(e) => gm::Error::Ground(e),
        Error::Binding(s) => gm::Error::Binding(s),
    }
}
fn ground(m: &Model, a: &Value) -> Result<Option<Ground>> {
    if a["g"].is_null() {
        Ok(None)
    } else {
        m.g(a).map(Some)
    }
}
fn query(m: &Model, a: &Value) -> Option<Location> {
    if let Some(id) = a["locationOf"].as_i64() {
        m.grounds[id as usize].read().location.clone()
    } else {
        loc(&a["location"])
    }
}
pub(super) fn raw(m: &mut Model, a: &Value) -> Result<Option<item::Handle>> {
    if let Some(value) = a.get("input") {
        return if value.is_null() {
            Ok(None)
        } else {
            m.extra
                .inputs
                .get(num(value) as usize)
                .cloned()
                .map(Some)
                .ok_or_else(|| Error::Binding("Index".into()))
        };
    }
    Ok(Some(ground::Context::copy_item(
        m,
        n(a, "id", 1),
        n(a, "amount", 1),
        n(a, "charge", 1000),
    )?))
}
fn list(m: &Model, a: &Value) -> Result<Option<Items>> {
    match a["list"].as_str().unwrap() {
        "manager" => Ok(Some(m.extra.manager.get_items())),
        "chunk" => Ok(Some(sync::get_items(&m.c(a)))),
        "ai" => Ok(m.extra.players[n(a, "p", 0) as usize].items.clone()),
        "capture" => m
            .extra
            .captures
            .get(n(a, "index", 0) as usize)
            .cloned()
            .ok_or_else(|| Error::Binding("Index".into())),
        _ => unreachable!(),
    }
}
pub(super) fn act(m: &mut Model, a: &Value) -> Result<Value> {
    let manager = m.extra.manager.clone();
    let result = match a["op"].as_str().unwrap() {
        "managerCreate" => {
            let ctor = a["ctor"].as_str().unwrap_or("ground");
            let g = if ctor == "ground" {
                gm::create(m, &manager, ground(m, a)?)
            } else {
                let input = raw(m, a)?;
                let location = query(m, a);
                let player = m.p(a);
                match ctor {
                    "player" => gm::create_player(m, &manager, input, player),
                    "uid" => gm::create_uid(
                        m,
                        &manager,
                        input,
                        location,
                        n(a, "uid", 7),
                        n(a, "absolute", 200),
                    ),
                    "location" => gm::create_at(m, &manager, input, location, None),
                    _ => gm::create_at(m, &manager, input, location, player),
                }
            }
            .map_err(error)?;
            return Ok(json!(m.gid(&g)));
        }
        "managerDestroy" => gm::destroy(m, &manager, ground(m, a)?).map(|g| g.map(|g| m.gid(&g))),
        "managerGet" => {
            gm::get(m, n(a, "id", 1), query(m, a), m.p(a)).map(|g| g.map(|g| m.gid(&g)))
        }
        "managerIncrease" => {
            let input = raw(m, a)?;
            gm::increase(m, &manager, input, query(m, a), m.p(a)).map(|g| Some(m.gid(&g)))
        }
        "managerPulse" => {
            gm::pulse(m, &manager).map_err(error)?;
            return Ok(Value::Null);
        }
        "chunkAdd" => {
            gm::add_chunk(m, &m.c(a), ground(m, a)?).map_err(error)?;
            return Ok(Value::Null);
        }
        "chunkRemove" => {
            gm::remove_chunk(m, &m.c(a), ground(m, a)?).map_err(error)?;
            return Ok(Value::Null);
        }
        "lookup" => {
            m.extra.lookup = n(a, "value", 0) as usize;
            return Ok(Value::Null);
        }
        "definition" => {
            let id = n(a, "id", 1);
            if a["replace"] == true {
                m.defs.insert(
                    id,
                    Shared::new(definitions::ItemDefinition {
                        id,
                        ..Default::default()
                    }),
                );
            }
            prepare_definition(m, id);
            let d = m.defs.for_id(id);
            let mut d = d.write();
            if let Some(value) = a.get("tradeable") {
                d.handlers.insert(
                    "tradeable".into(),
                    serde_json::from_value(value.clone()).unwrap(),
                );
            }
            if let Some(value) = a.get("plugin") {
                d.handlers.insert(
                    "plugin".into(),
                    value
                        .as_i64()
                        .map_or(Property::Null, |v| Property::Int(v as i32)),
                );
            }
            if let Some(value) = a.get("name") {
                d.name = value.as_str().map(str::to_owned);
                m.extra.details.insert(id);
            }
            if let Some(value) = a.get("options") {
                d.options = value.as_array().map(|values| {
                    Shared::new(
                        values
                            .iter()
                            .map(|v| v.as_str().map(str::to_owned))
                            .collect(),
                    )
                });
                m.extra.details.insert(id);
            }
            return Ok(Value::Null);
        }
        "itemDefinition" => {
            let item = m.g(a)?.read().item.clone();
            item.write().definition = if let Some(id) = a["id"].as_i64() {
                prepare_definition(m, id as i32);
                Some(m.defs.for_id(id as i32))
            } else {
                None
            };
            return Ok(Value::Null);
        }
        "setItem" => {
            let item = m.g(a)?.read().item.clone();
            if let Some(v) = a.get("amount") {
                item.write().set_amount(num(v));
            }
            return Ok(Value::Null);
        }
        "dropper" => {
            m.g(a)?.write().dropper = m.p(a);
            return Ok(Value::Null);
        }
        "inputSet" => {
            m.extra.inputs[n(a, "input", 0) as usize]
                .write()
                .set_amount(n(a, "amount", 1));
            return Ok(Value::Null);
        }
        "moveLocation" => {
            let location = m.g(a)?.read().location.clone().ok_or(Error::Null)?;
            let xyz = a["value"].as_array().unwrap();
            location.set(num(&xyz[0]), num(&xyz[1]), num(&xyz[2]));
            return Ok(Value::Null);
        }
        "virtual" => {
            let id = n(a, "g", 0) as usize;
            let v = m.extra.virtuals.entry(id).or_default();
            if let Some(value) = a.get("private") {
                v.private = value.as_bool();
            }
            if let Some(value) = a.get("active") {
                v.active = value.as_bool();
            }
            if let Some(value) = a.get("auto") {
                v.auto = value.as_bool();
            }
            return Ok(Value::Null);
        }
        "playerState" => {
            let p = &mut m.extra.players[n(a, "p", 0) as usize];
            if let Some(value) = a.get("location") {
                p.location = loc(value);
            }
            if let Some(value) = a.get("playing") {
                p.playing = value.as_bool().unwrap();
            }
            if let Some(value) = a.get("artificial") {
                p.artificial = value.as_bool().unwrap();
            }
            return Ok(Value::Null);
        }
        "setAI" => {
            let values = a["items"].as_array().map(|v| {
                v.iter()
                    .map(|g| g.as_i64().map(|i| m.grounds[i as usize].clone()))
                    .collect::<Vec<_>>()
            });
            m.extra.players[n(a, "p", 0) as usize].items =
                values.map(|values| Shared::new(values.into()));
            return Ok(Value::Null);
        }
        "captureList" => {
            let value = list(m, a)?;
            m.extra.captures.push(value);
            return Ok(json!(m.extra.captures.len() - 1));
        }
        "mutateList" => {
            let values = list(m, a)?.ok_or(Error::Null)?;
            match a["kind"].as_str().unwrap() {
                "clear" => values.write().clear(),
                "add" => values.write().push(ground(m, a)?),
                "remove" => {
                    return Ok(json!(values.write().remove_first(ground(m, a)?.as_ref())));
                }
                "set" => {
                    if !values.write().set(n(a, "slot", 0) as usize, ground(m, a)?) {
                        return Err(Error::Binding("Index".into()));
                    }
                }
                _ => unreachable!(),
            }
            return Ok(Value::Null);
        }
        "input" => {
            let input = raw(m, a)?.ok_or(Error::Null)?;
            m.extra.inputs.push(input);
            return Ok(json!(m.extra.inputs.len() - 1));
        }
        "array" => {
            let mut values = vec![];
            for value in a["items"].as_array().unwrap() {
                values.push(value.as_i64().map(|i| m.extra.inputs[i as usize].clone()));
            }
            m.extra.arrays.push(Shared::new(values.into_boxed_slice()));
            return Ok(json!(m.extra.arrays.len() - 1));
        }
        "arraySet" => {
            m.extra.arrays[n(a, "array", 0) as usize].write()[n(a, "slot", 0) as usize] =
                a["input"]
                    .as_i64()
                    .map(|i| m.extra.inputs[i as usize].clone());
            return Ok(Value::Null);
        }
        "managerMany" => {
            let array = a["array"]
                .as_i64()
                .map(|i| m.extra.arrays[i as usize].clone());
            gm::create_many(m, &manager, array, loc(&a["location"]), m.p(a)).map_err(error)?;
            return Ok(Value::Null);
        }
        _ => return super::spawn::act(m, a),
    }
    .map_err(error)?;
    Ok(json!(result))
}

pub(super) fn snapshot(m: &Model, state: &mut Value) {
    let ids = |items: &Items| {
        items
            .read()
            .iter()
            .map(|g| g.as_ref().map(|g| m.gid(g)))
            .collect::<Vec<_>>()
    };
    state["manager"] = json!(ids(&m.extra.manager.get_items()));
    state["players"]=json!(m.extra.players.iter().map(|p|json!({"location":p.location.as_ref().map(Location::coordinates),"playing":p.playing,"artificial":p.artificial,"ai":p.items.as_ref().map(ids)})).collect::<Vec<_>>());
    state["captures"] = json!(
        m.extra
            .captures
            .iter()
            .map(|i| i.as_ref().map(ids))
            .collect::<Vec<_>>()
    );
    state["definitions"] = json!({});
    let mut definitions: Vec<_> = m.defs.definitions().iter().collect();
    definitions.sort_by_key(|(id, _)| **id);
    state["definitions"]=json!(definitions.into_iter().map(|(id,d)|{
        let d=d.read();
        let mut row=json!({"id":id,"tradeable":d.handlers.get("tradeable").map_or(json!(false),|v|json!(v)),"plugin":d.handlers.get("plugin").and_then(Property::as_i64)});
        if m.extra.details.contains(id){row["name"]=json!(d.name);row["options"]=json!(d.options.as_ref().map(|o|o.read().clone()));}
        row
    }).collect::<Vec<_>>());
    state["inputs"] = json!(
        m.extra
            .inputs
            .iter()
            .map(|i| {
                let i = i.read();
                json!([i.id(), i.amount(), i.charge()])
            })
            .collect::<Vec<_>>()
    );
    state["arrays"] = json!(
        m.extra
            .arrays
            .iter()
            .map(|array| array
                .read()
                .iter()
                .map(|value| value.as_ref().map(|v| m
                    .extra
                    .inputs
                    .iter()
                    .position(|i| i.same(v))
                    .unwrap()))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
}

impl gm::Context for Model {
    type Plugin = i32;
    fn tradeable(&mut self, d: &definitions::Handle) -> gm::Result<bool> {
        let id = d.read().id;
        let value = item::definition_tradeable(d).map_err(|e| match e {
            rs09_server::inventory::Error::Null => gm::Error::Null,
            _ => gm::Error::Type,
        })?;
        self.emit("trade", vec![json!(id), json!(value)])
            .map_err(gm_error)?;
        Ok(value)
    }
    fn plugin(&mut self, d: &definitions::Handle) -> gm::Result<Option<i32>> {
        let (id, value) = {
            let d = d.read();
            (
                d.id,
                d.handlers
                    .get("plugin")
                    .and_then(Property::as_i64)
                    .map(|v| v as i32),
            )
        };
        self.emit("plugin", vec![json!(id), json!(value)])
            .map_err(gm_error)?;
        Ok(value)
    }
    fn plugin_remove(
        &mut self,
        plugin: i32,
        p: Option<NodeHandle>,
        g: &Ground,
        option: i32,
    ) -> gm::Result<()> {
        self.emit(
            "pluginRemove",
            vec![
                json!(plugin),
                json!(p.map(|p| self.pi(&p))),
                json!(self.gid(g)),
                json!(option),
            ],
        )
        .map_err(gm_error)
    }
    fn chunk(&mut self, location: Location) -> gm::Result<HandleChunk> {
        let c = self.chunks[self.extra.lookup].clone();
        self.emit("lookup", vec![json!(location.coordinates())])
            .map_err(gm_error)?;
        Ok(c)
    }
    fn flagged(&mut self, c: &HandleChunk) -> gm::Result<()> {
        for f in c.read().update_flags.read().iter().flatten() {
            if !self.flags.iter().any(|old| old.same(f)) {
                self.flags.push(f.clone());
            }
        }
        self.dirty.on_flagged(c);
        self.emit(
            "dirty",
            vec![json!(
                self.chunks.iter().position(|old| old.same(c)).unwrap()
            )],
        )
        .map_err(gm_error)
    }
    fn player_location(&mut self, p: &NodeHandle) -> gm::Result<Option<Location>> {
        let i = self.pi(p);
        let l = self.extra.players[i].location.clone();
        self.emit("location", vec![json!(i)]).map_err(gm_error)?;
        Ok(l)
    }
    fn playing(&mut self, p: &NodeHandle) -> gm::Result<bool> {
        let i = self.pi(p);
        let value = self.extra.players[i].playing;
        self.emit("playing", vec![json!(i)]).map_err(gm_error)?;
        Ok(value)
    }
    fn artificial(&mut self, p: &NodeHandle) -> gm::Result<bool> {
        let i = self.pi(p);
        let value = self.extra.players[i].artificial;
        self.emit("artificial", vec![json!(i)]).map_err(gm_error)?;
        Ok(value)
    }
    fn ai_items(&mut self, p: &NodeHandle) -> gm::Result<Option<Items>> {
        let i = self.pi(p);
        let value = self.extra.players[i].items.clone();
        self.emit("ai", vec![json!(i)]).map_err(gm_error)?;
        Ok(value)
    }
    fn packet(
        &mut self,
        kind: Packet,
        p: Option<NodeHandle>,
        g: &Ground,
        old: i32,
    ) -> gm::Result<()> {
        let name = match kind {
            Packet::Construct => "construct",
            Packet::Clear => "clear",
            Packet::Amount => "amount",
        };
        self.emit(
            "packet",
            vec![
                json!(name),
                json!(p.as_ref().map(|p| self.pi(p))),
                json!(self.gid(g)),
                json!(old),
            ],
        )
        .map_err(gm_error)?;
        let frames = gm::direct_frames(self, kind, p.clone(), g, old).map_err(gm_error)?;
        let mut wire = Vec::new();
        for frame in &frames {
            frame
                .write(
                    rs09_protocol::game::Direction::ServerToClient,
                    None,
                    &mut wire,
                )
                .map_err(|e| gm::Error::Binding(format!("invalid native packet boundary: {e}")))?;
        }
        let mut buffer = frames[0].clone();
        for frame in &frames[1..] {
            buffer.payload.push(frame.opcode);
            buffer.payload.extend_from_slice(&frame.payload);
        }
        Context::send(self, &p.ok_or(gm::Error::Null)?, &Shared::new(buffer)).map_err(gm_error)
    }
    fn auto_spawn(&mut self, g: &Ground) -> gm::Result<bool> {
        if self.spawns.registry.get(g).is_some() {
            return Ok(true);
        }
        let id = self.gid(g);
        let value = self.extra.virtuals.get(&id).and_then(|v| v.auto);
        if let Some(value) = value {
            self.emit("auto", vec![json!(id)]).map_err(gm_error)?;
            Ok(value)
        } else {
            Ok(false)
        }
    }
    fn respawn(&mut self, g: &Ground) -> gm::Result<()> {
        if let Some(spawn) = self.spawns.registry.get(g) {
            return super::spawn::schedule(self, &spawn);
        }
        self.emit("respawn", vec![json!(self.gid(g))])
            .map_err(gm_error)
    }
}
pub(super) fn virtual_private(m: &mut Model, g: &Ground) -> Result<Option<bool>> {
    if m.spawns.registry.get(g).is_some() {
        return Ok(Some(false));
    }
    let id = m.gid(g);
    let value = m.extra.virtuals.get(&id).and_then(|v| v.private);
    if value.is_some() {
        m.emit("private", vec![json!(id)])?;
    }
    Ok(value)
}
pub(super) fn virtual_active(m: &mut Model, g: &Ground) -> Result<Option<bool>> {
    if m.spawns.registry.get(g).is_some() {
        return Ok(Some(true));
    }
    let id = m.gid(g);
    let value = m.extra.virtuals.get(&id).and_then(|v| v.active);
    if value.is_some() {
        m.emit("active", vec![json!(id)])?;
    }
    Ok(value)
}
