use rs09_server::world::{
    component_objects::{NativeObject, Plugin},
    doors::Location,
    input_prompt::{self as input, Context, InputType, Transfer},
    player_walk::{NodeHandle, State},
};
use serde_json::{Value as J, json};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = input::Result<T>;
#[derive(Clone)]
enum Script {
    Named(String),
    Transfer(Transfer),
}
#[derive(Clone)]
enum V {
    Null,
    Data(J),
    Script(Script),
    After(Plugin),
}
struct After(String);
impl NativeObject for After {}
fn units(v: &J) -> Vec<u16> {
    if let Some(s) = v.as_str() {
        s.encode_utf16().collect()
    } else {
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u16)
            .collect()
    }
}
impl V {
    fn read(v: &J) -> Self {
        if v.is_null() {
            return Self::Null;
        }
        match v["kind"].as_str().unwrap() {
            "script" => Self::Script(Script::Named(v["value"].as_str().unwrap().into())),
            "after" => Self::After(Plugin::new(After(v["value"].as_str().unwrap().into()))),
            "text" => Self::Data(json!({"kind":"text","value":units(&v["value"])})),
            _ => Self::Data(v.clone()),
        }
    }
    fn json(&self) -> J {
        match self {
            Self::Null => J::Null,
            Self::Data(v) => v.clone(),
            Self::Script(Script::Named(s)) => json!({"kind":"script","value":s}),
            Self::Script(Script::Transfer(_)) => json!({"kind":"transfer"}),
            Self::After(p) => json!({"kind":"after","value":p.value::<After>().unwrap().0}),
        }
    }
}
impl input::Value for V {
    fn integer(&self) -> Option<i32> {
        match self {
            Self::Data(v) if v["kind"] == "int" => v["value"].as_i64().map(|n| n as i32),
            _ => None,
        }
    }
    fn from_integer(v: i32) -> Self {
        Self::Data(json!({"kind":"int","value":v}))
    }
    fn from_text(v: Vec<u16>) -> Self {
        Self::Data(json!({"kind":"text","value":v}))
    }
}
#[derive(Clone)]
struct Bank {
    last: i32,
    transfers: Vec<J>,
}
struct Model {
    source: J,
    events: Vec<J>,
    calls: BTreeMap<String, usize>,
    hooks: BTreeSet<usize>,
    players: Vec<NodeHandle>,
    _state: State,
    attributes: Vec<BTreeMap<String, V>>,
    banks: Vec<Bank>,
    bank_ids: Vec<usize>,
    interpreters: Vec<usize>,
    locks: Vec<usize>,
    locked: Vec<bool>,
    aliases: BTreeMap<String, V>,
}
fn n(a: &J, key: &str, default: usize) -> usize {
    a.get(key)
        .and_then(J::as_u64)
        .map_or(default, |n| n as usize)
}
fn kind(n: usize) -> InputType {
    [
        InputType::Amount,
        InputType::Numeric,
        InputType::Short,
        InputType::Long,
        InputType::Message,
    ][n]
}
fn ordinal(t: InputType) -> usize {
    match t {
        InputType::Amount => 0,
        InputType::Numeric => 1,
        InputType::Short => 2,
        InputType::Long => 3,
        InputType::Message => 4,
    }
}
impl Model {
    fn emit(&mut self, event: &str, args: Vec<J>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(J::Array(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self
            .source
            .get("hooks")
            .and_then(J::as_array)
            .cloned()
            .unwrap_or_default();
        for (i, h) in hooks.iter().enumerate() {
            if h["event"] == event && n(h, "at", 1) == count && self.hooks.insert(i) {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.source["failures"][event].as_u64() == Some(count as u64) {
            return Err(input::Error::Binding("state".into()));
        }
        Ok(())
    }
    fn p(&self, p: &NodeHandle) -> usize {
        self.players.iter().position(|q| p == q).unwrap()
    }
    fn get(&mut self, p: usize, key: &str) -> Result<V> {
        self.emit("get", vec![json!(p), json!(key)])?;
        Ok(self.attributes[p].get(key).cloned().unwrap_or(V::Null))
    }
    fn set(&mut self, p: usize, key: &str, value: V) -> Result<()> {
        self.attributes[p].insert(key.into(), value.clone());
        self.emit("set", vec![json!(p), json!(key), value.json()])
    }
    fn cast_script(value: V) -> Result<Option<Script>> {
        match value {
            V::Null => Ok(None),
            V::Script(s) => Ok(Some(s)),
            _ => Err(input::Error::Type),
        }
    }
    fn act(&mut self, a: &J) -> Result<()> {
        let id = n(a, "player", 0);
        let p = self.players[id].clone();
        match a["op"].as_str().unwrap() {
            "removeRaw" => {
                self.attributes[id].remove(a["key"].as_str().unwrap());
            }
            "callbackText" => {
                let V::Script(Script::Transfer(t)) = self.attributes[id]["runscript"].clone()
                else {
                    unreachable!()
                };
                let text = rs09_server::player::attributes::AttributeObject::java_string(&t)
                    .encode_utf16()
                    .collect::<Vec<_>>();
                self.emit("callbackText", vec![json!(text)])?;
            }
            "set" => {
                self.attributes[id].insert(a["key"].as_str().unwrap().into(), V::read(&a["value"]));
            }
            "field" => match a["field"].as_str().unwrap() {
                "bank" => self.bank_ids[id] = n(a, "value", 0),
                "interpreter" => self.interpreters[id] = n(a, "value", 0),
                "locks" => self.locks[id] = n(a, "value", 0),
                "locked" => self.locked[id] = a["value"].as_bool().unwrap(),
                _ => unreachable!(),
            },
            "process" => {
                let value = V::read(&a["value"]);
                if matches!(value, V::Null) {
                    return Err(input::Error::Null);
                }
                input::process(
                    self,
                    &p,
                    value,
                    Script::Named(a["script"].as_str().unwrap_or("ok").into()),
                )?;
            }
            "receive" => {
                let value = V::read(&a["value"]);
                if matches!(value, V::Null) {
                    return Err(input::Error::Null);
                }
                input::receive(self, &p, value)?;
            }
            "send" => input::send(
                self,
                &p,
                kind(n(a, "type", 0)),
                units(a.get("prompt").unwrap_or(&json!("Enter:"))),
                Script::Named(a["script"].as_str().unwrap_or("ok").into()),
            )?,
            "transfer" => input::transfer_x(
                self,
                &p,
                a["slot"].as_i64().unwrap_or(0) as i32,
                a["withdraw"].as_bool().unwrap_or(false),
                a["after"]
                    .as_str()
                    .map(|name| Plugin::new(After(name.into()))),
            )?,
            "capture" => {
                self.aliases.insert(
                    a["name"].as_str().unwrap().into(),
                    self.attributes[id]
                        .get("runscript")
                        .cloned()
                        .unwrap_or(V::Null),
                );
            }
            "restore" => {
                self.attributes[id].insert(
                    "runscript".into(),
                    self.aliases[a["name"].as_str().unwrap()].clone(),
                );
            }
            "call" => {
                let value = self.aliases[a["name"].as_str().unwrap()].clone();
                let s = Self::cast_script(value)?.ok_or(input::Error::Null)?;
                self.invoke(s, V::read(&a["value"]))?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    fn snapshot(&self, error: Option<String>) -> J {
        json!({"error":error,"events":self.events,"players":self.attributes.iter().map(|a|a.iter().map(|(k,v)|(k,v.json())).collect::<BTreeMap<_,_>>()).collect::<Vec<_>>(),"banks":self.banks.iter().map(|b|json!({"last":b.last,"transfers":b.transfers})).collect::<Vec<_>>(),"aliases":self.aliases.iter().map(|(k,v)|(k,v.json())).collect::<BTreeMap<_,_>>()})
    }
}
impl Context for Model {
    type Value = V;
    type Script = Script;
    type Interpreter = usize;
    type Bank = usize;
    fn script(&mut self, p: &NodeHandle) -> Result<Option<Script>> {
        let id = self.p(p);
        let v = self.get(id, "runscript")?;
        Self::cast_script(v)
    }
    fn interaction_locked(&mut self, p: &NodeHandle) -> Result<bool> {
        let id = self.p(p);
        self.emit("locks", vec![json!(id)])?;
        let locks = self.locks[id];
        self.emit("locked", vec![json!(locks)])?;
        Ok(self.locked[locks])
    }
    fn input_type(&mut self, p: &NodeHandle) -> Result<InputType> {
        let id = self.p(p);
        match self.get(id, "input-type")? {
            V::Null => Ok(InputType::Numeric),
            V::Data(v) if v["kind"] == "type" => Ok(kind(v["value"].as_u64().unwrap() as usize)),
            _ => Err(input::Error::Type),
        }
    }
    fn parse_amount(&mut self, p: &NodeHandle) -> Result<bool> {
        let id = self.p(p);
        match self.get(id, "parseamount")? {
            V::Null => Ok(false),
            V::Data(v) if v["kind"] == "bool" => Ok(v["value"].as_bool().unwrap()),
            _ => Err(input::Error::Type),
        }
    }
    fn string(&mut self, value: &V) -> Result<Vec<u16>> {
        match value {
            V::Data(v) => Ok(match v["kind"].as_str().unwrap() {
                "text" => units(&v["value"]),
                "int" | "long" | "bool" => v["value"].to_string().encode_utf16().collect(),
                "type" => [
                    "AMOUNT",
                    "NUMERIC",
                    "STRING_SHORT",
                    "STRING_LONG",
                    "MESSAGE",
                ][v["value"].as_u64().unwrap() as usize]
                    .encode_utf16()
                    .collect(),
                "print" => {
                    self.emit("string", vec![v["label"].clone()])?;
                    match v["error"].as_str().unwrap_or("ok") {
                        "number" => return Err(input::Error::Number),
                        "type" => return Err(input::Error::Type),
                        "state" => return Err(input::Error::Binding("state".into())),
                        _ => {}
                    }
                    units(&v["value"])
                }
                _ => unreachable!(),
            }),
            _ => Err(input::Error::Binding(
                "unsupported recording value string".into(),
            )),
        }
    }
    fn invoke(&mut self, s: Script, value: V) -> Result<()> {
        match s {
            Script::Transfer(t) => input::invoke_transfer(self, &t, &value),
            Script::Named(name) => {
                self.emit("invoke", vec![json!(name), value.json()])?;
                match name.as_str() {
                    "number" => Err(input::Error::Number),
                    "type" => Err(input::Error::Type),
                    "state" => Err(input::Error::Binding("state".into())),
                    _ => Ok(()),
                }
            }
        }
    }
    fn dialogue(&mut self, p: &NodeHandle, text: Vec<u16>) -> Result<()> {
        let id = self.p(p);
        self.emit("dialogue", vec![json!(id), json!(text)])
    }
    fn remove(&mut self, p: &NodeHandle, key: &str) -> Result<()> {
        let id = self.p(p);
        self.attributes[id].remove(key);
        self.emit("remove", vec![json!(id), json!(key)])
    }
    fn set_parse_amount(&mut self, p: &NodeHandle, value: bool) -> Result<()> {
        let id = self.p(p);
        self.set(
            id,
            "parseamount",
            V::Data(json!({"kind":"bool","value":value})),
        )
    }
    fn interpreter(&mut self, p: &NodeHandle) -> Result<usize> {
        let id = self.p(p);
        self.emit("interpreter", vec![json!(id)])?;
        Ok(self.interpreters[id])
    }
    fn send_input(&mut self, d: usize, string: bool, prompt: Vec<u16>) -> Result<()> {
        self.emit("input", vec![json!(d), json!(string), json!(prompt)])
    }
    fn send_long(&mut self, d: usize, prompt: Vec<u16>) -> Result<()> {
        self.emit("long", vec![json!(d), json!(prompt)])
    }
    fn send_message(&mut self, d: usize, prompt: Vec<u16>) -> Result<()> {
        self.emit("message", vec![json!(d), json!(prompt)])
    }
    fn set_script(&mut self, p: &NodeHandle, script: Script) -> Result<()> {
        let id = self.p(p);
        self.set(id, "runscript", V::Script(script))
    }
    fn set_type(&mut self, p: &NodeHandle, t: InputType) -> Result<()> {
        let id = self.p(p);
        self.set(
            id,
            "input-type",
            V::Data(json!({"kind":"type","value":ordinal(t)})),
        )
    }
    fn make_transfer(&mut self, t: Transfer) -> Result<Script> {
        Ok(Script::Transfer(t))
    }
    fn bank(&mut self, p: &NodeHandle) -> Result<usize> {
        let id = self.p(p);
        self.emit("bank", vec![json!(id)])?;
        Ok(self.bank_ids[id])
    }
    fn take(&mut self, b: usize, slot: i32, amount: i32) -> Result<()> {
        self.emit("take", vec![json!(b), json!(slot), json!(amount)])?;
        self.banks[b].transfers.push(json!([true, slot, amount]));
        Ok(())
    }
    fn deposit(&mut self, b: usize, slot: i32, amount: i32) -> Result<()> {
        self.emit("deposit", vec![json!(b), json!(slot), json!(amount)])?;
        self.banks[b].transfers.push(json!([false, slot, amount]));
        Ok(())
    }
    fn last_amount(&mut self, b: usize, amount: i32) -> Result<()> {
        self.banks[b].last = amount;
        self.emit("last", vec![json!(b), json!(amount)])
    }
    fn after(&mut self, p: Plugin) -> Result<()> {
        self.emit("after", vec![json!(p.value::<After>().unwrap().0)])
    }
}
pub fn evaluate(row: &J) -> J {
    if let Some(wire) = row.get("decode") {
        use rs09_protocol::{buffer::Reader, game::actions::ClientAction};
        let data = wire["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect::<Vec<_>>();
        let mut reader = Reader::new(&data);
        reader.seek(n(wire, "start", 0)).unwrap();
        let (error, value) =
            match ClientAction::decode(wire["opcode"].as_u64().unwrap() as u8, &mut reader) {
                Ok(Some(ClientAction::InputIntResponse { response })) => {
                    (J::Null, json!({"kind":"int","value":response}))
                }
                Ok(Some(ClientAction::InputStringResponse { response })) => (
                    J::Null,
                    json!({"kind":"text","value":response.encode_utf16().collect::<Vec<_>>()}),
                ),
                Err(_) => (json!("truncated"), J::Null),
                _ => panic!("unsupported input opcode"),
            };
        return json!({"error":error,"position":reader.position(),"value":value});
    }
    let mut state = State::default();
    let mut players = vec![];
    for i in 0..3 {
        let id = state.add(i, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
        players.push(state.retain_node(id).unwrap());
    }
    let mut m = Model {
        source: row.clone(),
        events: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        players,
        _state: state,
        attributes: vec![BTreeMap::new(); 3],
        banks: vec![
            Bank {
                last: 50,
                transfers: vec![]
            };
            3
        ],
        bank_ids: vec![0, 1, 2],
        interpreters: vec![0, 1, 2],
        locks: vec![0, 1, 2],
        locked: vec![false; 3],
        aliases: BTreeMap::new(),
    };
    let mut out = vec![];
    for a in row["actions"].as_array().unwrap() {
        let e = m.act(a).err().map(|e| match e {
            input::Error::Null => "null".into(),
            input::Error::Type => "type".into(),
            input::Error::Number => "number".into(),
            input::Error::Binding(s) => s,
        });
        out.push(m.snapshot(e));
    }
    json!(out)
}
