use rs09_protocol::game::{Direction, Frame};
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        npc_render::{self, Context, Error, Handle, List, Npc, Result},
        player_render::{self, Player},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |v| v as i32)
}
fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
struct Model {
    input: Value,
    player: player_render::Handle,
    npcs: BTreeMap<String, Handle>,
    hidden: BTreeMap<String, bool>,
    tolerance: BTreeMap<String, Shared<BTreeMap<i32, i32>>>,
    views: Vec<Shared<List>>,
    events: Vec<Value>,
    frames: Vec<Value>,
    calls: BTreeMap<String, i32>,
}
impl Model {
    fn name(&self, npc: &Handle) -> String {
        self.npcs
            .iter()
            .find_map(|(n, h)| h.same(npc).then_some(n.clone()))
            .unwrap()
    }
    fn list(&self) -> Shared<List> {
        self.player.read().info.read().local_npcs.clone()
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn set(&mut self, name: &str, fields: &Value) -> Result<()> {
        if name == "self" {
            let mut p = self.player.write();
            if fields.get("index").is_some() {
                p.index = n(fields, "index", 1);
            }
            if fields.get("location").is_some() {
                p.location = loc(&fields["location"]);
            }
            return Ok(());
        }
        let npc = self.npcs.get(name).ok_or(Error::Null)?;
        let mut npc = npc.write();
        if fields.get("index").is_some() {
            npc.index = n(fields, "index", 0);
        }
        if fields.get("id").is_some() {
            npc.id = n(fields, "id", 0);
        }
        if fields.get("location").is_some() {
            npc.location = loc(&fields["location"]);
        }
        if fields.get("walk").is_some() {
            npc.walk_dir = n(fields, "walk", -1);
        }
        if fields.get("run").is_some() {
            npc.run_dir = n(fields, "run", -1);
        }
        if fields.get("direction").is_some() {
            npc.direction = n(fields, "direction", 0);
        }
        if let Some(b) = fields["required"].as_bool() {
            npc.masks_required = b;
        }
        if let Some(b) = fields["teleport"].as_bool() {
            npc.teleporting = b;
        }
        if let Some(b) = fields["hidden"].as_bool() {
            self.hidden.insert(name.into(), b);
        }
        if let Some(b) = fields["aggressive"].as_bool() {
            npc.aggressive = b;
            if b {
                self.tolerance
                    .insert(name.into(), Shared::new(BTreeMap::new()));
            } else {
                self.tolerance.remove(name);
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<()> {
        let name = a["node"].as_str().unwrap_or("a");
        let list = self.list();
        match a["op"].as_str().unwrap() {
            "render" => npc_render::render(self, &self.player.clone())?,
            "set" => self.set(name, &a["fields"])?,
            "clear" => list.write().clear(),
            "push" => list.write().push(self.npcs.get(name).cloned()),
            "erase" => {
                list.write().remove(n(a, "index", 0) as usize)?;
            }
            "replaceList" => {
                let fresh = Shared::new(List::default());
                self.player.read().info.write().local_npcs = fresh.clone();
                self.views.push(fresh);
            }
            "fail" => return Err(Error::Binding(String::new())),
            op => panic!("unknown {op}"),
        }
        Ok(())
    }
    fn names(&self, list: &Shared<List>) -> Vec<Value> {
        list.read()
            .values()
            .iter()
            .map(|n| n.as_ref().map_or(Value::Null, |n| json!(self.name(n))))
            .collect()
    }
    fn state(&self, error: Option<String>) -> Value {
        let ns=self.npcs.iter().map(|(name,npc)|{let p=npc.read();(name,json!({"index":p.index,"id":p.id,"location":coord(&p.location),"walk":p.walk_dir,"run":p.run_dir,"direction":p.direction,"required":p.masks_required,"teleport":p.teleporting,"hidden":self.hidden.get(name).copied().unwrap_or(false),"tolerance":self.tolerance.get(name).map(|t|t.read().iter().map(|(k,v)|json!([k,v])).collect::<Vec<_>>())}))}).collect::<BTreeMap<_,_>>();
        let p = self.player.read();
        json!({"npcs":ns,"player":[p.index,coord(&p.location)],"locals":self.names(&self.list()),"views":self.views.iter().map(|l|self.names(l)).collect::<Vec<_>>(),"events":self.events,"frames":self.frames,"error":error})
    }
}
impl Context for Model {
    fn hidden(&mut self, _: &player_render::Handle, npc: &Handle) -> Result<bool> {
        let name = self.name(npc);
        self.emit("hidden", vec![json!(name)])?;
        Ok(self.hidden.get(&name).copied().unwrap_or(false))
    }
    fn candidates(&mut self, location: Location, distance: i32) -> Result<Vec<Option<Handle>>> {
        self.emit(
            "candidates",
            vec![json!(location.coordinates()), json!(distance)],
        )?;
        Ok(self.input["candidates"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|n| n.as_str().and_then(|n| self.npcs.get(n)).cloned())
            .collect())
    }
    fn masks(
        &mut self,
        _: &player_render::Handle,
        npc: &Handle,
        out: &mut Vec<u8>,
        sync: bool,
    ) -> Result<()> {
        let name = self.name(npc);
        self.emit("masks", vec![json!(name), json!(sync)])?;
        out.extend(
            self.input["npcs"][&name]["mask"]
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![json!(0)])
                .into_iter()
                .map(|n| n.as_i64().unwrap() as u8),
        );
        Ok(())
    }
    fn remove_tolerance(&mut self, npc: &Handle, index: i32) -> Result<()> {
        let name = self.name(npc);
        let table = self.tolerance.get(&name).cloned().ok_or(Error::Null)?;
        self.emit("removeTolerance", vec![json!(name), json!(index)])?;
        table.write().remove(&index);
        Ok(())
    }
    fn add_tolerance(&mut self, npc: &Handle, index: i32) -> Result<()> {
        let name = self.name(npc);
        let table = self.tolerance.get(&name).cloned().ok_or(Error::Null)?;
        self.emit("addTolerance", vec![json!(name), json!(index), json!(100)])?;
        table.write().insert(index, 100);
        Ok(())
    }
    fn send(&mut self, _: &player_render::Handle, frame: Frame) -> Result<()> {
        self.emit("send", vec![json!(frame.opcode)])?;
        let mut bytes = vec![];
        frame
            .write(Direction::ServerToClient, None, &mut bytes)
            .map_err(|e| Error::Binding(e.to_string()))?;
        self.frames.push(json!(bytes));
        Ok(())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let player = Shared::new(Player::new(
        n(&input["player"], "index", 1),
        Location::new(3207, 3217, 0),
        2000,
    ));
    let mut m = Model {
        input: input.clone(),
        player,
        npcs: BTreeMap::new(),
        hidden: BTreeMap::new(),
        tolerance: BTreeMap::new(),
        views: vec![],
        events: vec![],
        frames: vec![],
        calls: BTreeMap::new(),
    };
    for (name, fields) in input["npcs"].as_object().unwrap() {
        let mut npc = Npc::new(0, 0, Location::new(0, 0, 0));
        npc.location = None;
        m.npcs.insert(name.clone(), Shared::new(npc));
        m.set(name, fields).unwrap();
    }
    m.set("self", &input["player"]).unwrap();
    let list = m.list();
    for name in input["locals"].as_array().cloned().unwrap_or_default() {
        list.write()
            .push(name.as_str().and_then(|n| m.npcs.get(n)).cloned());
    }
    m.views.push(list);
    let mut states = vec![];
    for action in input["actions"].as_array().unwrap() {
        m.events.clear();
        m.frames.clear();
        let error = m.act(action).err().map(|e| match e {
            Error::Binding(_) => "Binding".into(),
            e => e.to_string(),
        });
        states.push(m.state(error));
    }
    json!(states)
}
