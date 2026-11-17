use rs09_protocol::game::{Direction, Frame};
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        player_render::{self as render, Error, Handle, Player, PlayerList, Result},
        rebuild,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
fn loc(v: &Value) -> Option<Location> {
    v.as_array().map(|a| {
        Location::new(
            a[0].as_i64().unwrap() as i32,
            a[1].as_i64().unwrap() as i32,
            a[2].as_i64().unwrap() as i32,
        )
    })
}
fn set(p: &Handle, v: &Value) {
    let mut p = p.write();
    if let Some(i) = v["index"].as_i64() {
        p.index = i as i32;
    }
    if v.get("location").is_some() {
        p.location = loc(&v["location"]);
    }
    for (k, field) in [("active", &mut p.active)] {
        if let Some(b) = v[k].as_bool() {
            *field = b;
        }
    }
    if let Some(b) = v["scene"].as_bool() {
        p.update_scene = b;
    }
    if let Some(b) = v["teleport"].as_bool() {
        p.teleporting = b;
    }
    if let Some(b) = v["invisible"].as_bool() {
        p.invisible = b;
    }
    if let Some(n) = v["walk"].as_i64() {
        p.walk_dir = n as i32;
    }
    if let Some(n) = v["run"].as_i64() {
        p.run_dir = n as i32;
    }
    if let Some(n) = v["direction"].as_i64() {
        p.direction = n as i32;
    }
    if let Some(b) = v["required"].as_bool() {
        p.masks.required = b;
    }
    if let Some(b) = v["synced"].as_bool() {
        p.masks.synced = b;
    }
    if let Some(n) = v["stamp"].as_i64() {
        p.masks.appearance_stamp = n;
    }
}
struct Model {
    input: Value,
    nodes: BTreeMap<String, Handle>,
    views: Vec<Shared<PlayerList>>,
    events: Vec<Value>,
    frames: Vec<String>,
    calls: HashMap<String, i32>,
}
impl Model {
    fn name(&self, p: &Handle) -> String {
        self.nodes
            .iter()
            .find(|(_, n)| n.same(p))
            .unwrap()
            .0
            .clone()
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut e = vec![json!(event)];
        e.extend(args);
        self.events.push(json!(e));
        let n = self.calls.entry(event.into()).or_default();
        *n += 1;
        let n = *n;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for h in hooks {
            if h["event"] == event && h["at"].as_i64().unwrap_or(1) == i64::from(n) {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn send_frame(&mut self, frame: Frame) -> Result<()> {
        self.emit("send", vec![json!(frame.opcode)])?;
        let mut bytes = vec![];
        frame
            .write(Direction::ServerToClient, None, &mut bytes)
            .map_err(|e| Error::Binding(format!("{e:?}")))?;
        self.frames
            .push(bytes.iter().map(|b| format!("{b:02x}")).collect());
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<()> {
        let p = if a.get("node").is_some_and(Value::is_null) {
            None
        } else {
            self.nodes
                .get(a["node"].as_str().unwrap_or("self"))
                .cloned()
        };
        let info = self.nodes["self"].read().info.clone();
        let list = info.read().local_players.clone();
        match a["op"].as_str().unwrap() {
            "render" => render::render(self, &p.ok_or(Error::Null)?)?,
            "rebuild" => rebuild::send(self, &p.ok_or(Error::Null)?)?,
            "set" => set(&p.ok_or(Error::Null)?, &a["fields"]),
            "clear" => list.write().clear(),
            "push" => list.write().push(p),
            "erase" => {
                list.write().remove(a["index"].as_i64().unwrap() as usize)?;
            }
            "replaceList" => {
                let list = Shared::new(PlayerList::default());
                info.write().local_players = list.clone();
                self.views.push(list);
            }
            "maskArray" => {
                info.write().mask_updates =
                    Shared::new(vec![None; a["size"].as_u64().unwrap() as usize]);
            }
            "infoSet" => {
                let mut info = info.write();
                info.mask_update_count = a["count"].as_i64().unwrap() as i32;
                info.prepared_appearance = a["prepared"].as_bool().unwrap();
                info.on_first_cycle = a["first"].as_bool().unwrap();
            }
            "registerMask" => info.write().register_mask_update(p.map(|p| p.identity()))?,
            "information" => {
                let l = self.nodes["self"].read().location.clone();
                info.write().update_information(l);
            }
            "fail" => return Err(Error::Binding("test".into())),
            other => panic!("unknown render action {other}"),
        };
        Ok(())
    }
    fn snapshot(&self) -> Value {
        let mut nodes = BTreeMap::new();
        for (k, p) in &self.nodes {
            let p = p.read();
            let info = p.info.read();
            let stamps: Vec<_> = info
                .appearance_stamps
                .read()
                .iter()
                .enumerate()
                .filter(|(_, s)| **s != 0)
                .map(|(i, s)| json!([i, s]))
                .collect();
            let names: Vec<_> = info
                .local_players
                .read()
                .values()
                .iter()
                .map(|p| p.as_ref().map(|p| self.name(p)))
                .collect();
            nodes.insert(k,json!({"location":p.location.as_ref().map(Location::coordinates),"lastScene":p.last_scene.as_ref().map(Location::coordinates),"active":p.active,"locals":names,"stamps":stamps}));
            if self.input["metadata"] == true {
                let masks: Vec<_> = info
                    .mask_updates
                    .read()
                    .iter()
                    .map(|n| {
                        n.and_then(|id| {
                            self.nodes
                                .iter()
                                .find(|(_, p)| p.identity() == id)
                                .map(|(k, _)| k)
                        })
                    })
                    .collect();
                nodes.get_mut(k).unwrap()["info"] = json!({"first":info.on_first_cycle,"prepared":info.prepared_appearance,"last":info.last_location.as_ref().map(Location::coordinates),"count":info.mask_update_count,"masks":masks});
            }
        }
        let views: Vec<_> = self
            .views
            .iter()
            .map(|v| {
                v.read()
                    .values()
                    .iter()
                    .map(|p| p.as_ref().map(|p| self.name(p)))
                    .collect::<Vec<_>>()
            })
            .collect();
        json!({"nodes":nodes,"views":views,"events":self.events,"frames":self.frames})
    }
}
impl render::Context for Model {
    fn candidates(&mut self, l: Location, d: i32) -> Result<Vec<Option<Handle>>> {
        self.emit("candidates", vec![json!(l.coordinates()), json!(d)])?;
        Ok(self.input["candidates"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|v| v.as_str().and_then(|s| self.nodes.get(s)).cloned())
            .collect())
    }
    fn masks(
        &mut self,
        local: &Handle,
        other: &Handle,
        out: &mut Vec<u8>,
        appearance: bool,
        sync: bool,
    ) -> Result<()> {
        self.emit(
            "masks",
            vec![
                json!(self.name(local)),
                json!(self.name(other)),
                json!(appearance),
                json!(sync),
            ],
        )?;
        out.extend([
            other.read().index as u8,
            u8::from(appearance),
            u8::from(sync),
        ]);
        Ok(())
    }
    fn send(&mut self, _: &Handle, f: Frame) -> Result<()> {
        self.send_frame(f)
    }
}
impl rebuild::Context for Model {
    fn keys(&mut self, id: i32) -> Result<[i32; 4]> {
        self.emit("keys", vec![json!(id)])?;
        Ok([id, id.wrapping_neg(), i32::MIN, i32::MAX])
    }
    fn send(&mut self, _: &Handle, f: Frame) -> Result<()> {
        self.send_frame(f)
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut nodes = BTreeMap::new();
    for (k, v) in input["nodes"].as_object().unwrap() {
        let p = Shared::new(Player::new(
            v["index"].as_i64().unwrap_or(0) as i32,
            loc(&v["location"]).unwrap_or_else(|| Location::new(0, 0, 0)),
            2000,
        ));
        set(&p, v);
        nodes.insert(k.clone(), p);
    }
    let self_node = nodes["self"].clone();
    self_node.write().last_scene = loc(input.get("lastScene").unwrap_or(&json!([3207, 3217, 0])));
    let list = self_node.read().info.read().local_players.clone();
    for n in input["locals"].as_array().cloned().unwrap_or_default() {
        list.write()
            .push(n.as_str().and_then(|s| nodes.get(s)).cloned());
    }
    let mut m = Model {
        input: input.clone(),
        nodes,
        views: vec![list],
        events: vec![],
        frames: vec![],
        calls: HashMap::new(),
    };
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        m.frames.clear();
        let error = m.act(a).err().map(|e| match e {
            Error::Null => "Null",
            Error::Index => "Index",
            Error::ConcurrentModification => "ConcurrentModification",
            Error::Binding(_) => "Binding",
        });
        let mut s = m.snapshot();
        s["error"] = json!(error);
        out.push(s);
    }
    json!(out)
}
