//! Recording adapter only: numeric executor identities select fixture callbacks.
//! Production LockElapse values are actual Plugin/Native bodies and capture fields.
use rs09_server::world::action_locks::{
    ActionLocks, Context, Error, Key, Kind, Lock, Result, Text,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Default)]
struct Host;
struct Model {
    input: Value,
    actions: ActionLocks<usize>,
    refs: BTreeMap<String, Option<Lock<usize>>>,
    locks: Vec<Lock<usize>>,
    events: Vec<String>,
    trace: Vec<Value>,
    tick: i32,
    calls: usize,
    hooks: BTreeMap<usize, Vec<Value>>,
}
fn number(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |n| n as i32)
}
fn text(v: &Value) -> Option<Text> {
    v.as_array().map(|a| {
        a.iter()
            .map(|c| c.as_u64().unwrap() as u16)
            .collect::<Vec<_>>()
            .into()
    })
}
fn key(v: &Value) -> Key {
    text(v).map(|s| s.to_vec())
}
fn kind(s: &str) -> Kind {
    match s {
        "m" | "movement" => Kind::Movement,
        "t" | "teleport" => Kind::Teleport,
        "c" | "component" => Kind::Component,
        "i" | "interaction" => Kind::Interaction,
        "equipment" => Kind::Equipment,
        _ => panic!("unknown lock slot {s}"),
    }
}
impl Model {
    fn new(input: &Value) -> Self {
        let mut s = Self {
            input: input.clone(),
            actions: ActionLocks::default(),
            refs: BTreeMap::new(),
            locks: vec![],
            events: vec![],
            trace: vec![],
            tick: number(input, "tick", 0),
            calls: 0,
            hooks: BTreeMap::new(),
        };
        for name in ["m", "t", "c", "i"] {
            let lock = s.actions.get(kind(name)).unwrap();
            s.id(&lock);
            s.refs.insert(name.into(), Some(lock));
        }
        if let Some(h) = input["hooks"].as_object() {
            for (k, v) in h {
                s.hooks
                    .insert(k.parse().unwrap(), v.as_array().unwrap().clone());
            }
        }
        s
    }
    fn id(&mut self, lock: &Lock<usize>) -> usize {
        if let Some(i) = self.locks.iter().position(|l| l.same(lock)) {
            i
        } else {
            let i = self.locks.len();
            self.locks.push(lock.clone());
            i
        }
    }
    fn optional_id(&mut self, lock: Option<Lock<usize>>) -> Value {
        lock.as_ref().map_or(Value::Null, |l| json!(self.id(l)))
    }
    fn get(
        &self,
        action: &Value,
        field: &str,
        passed: Option<&Lock<usize>>,
    ) -> Option<Lock<usize>> {
        let name = action[field].as_str().unwrap_or("");
        if name == "passed" {
            passed.cloned()
        } else {
            self.refs.get(name).cloned().flatten()
        }
    }
    fn event(&mut self, v: &Value) -> Option<usize> {
        v.as_str().map(|name| {
            if let Some(i) = self.events.iter().position(|s| s == name) {
                i
            } else {
                let i = self.events.len();
                self.events.push(name.into());
                i
            }
        })
    }
    fn act(&mut self, host: &mut Host, a: &Value, passed: Option<&Lock<usize>>) -> Result<Value> {
        let main = self.actions.clone();
        let slot = a["kind"].as_str().unwrap_or("all");
        match a["op"].as_str().unwrap() {
            "tick" => self.tick = number(a, "value", 0),
            "fail" => return Err(Error::Binding("callback".into())),
            "new" => {
                let lock = Lock::new(text(&a["message"]));
                self.refs
                    .insert(a["name"].as_str().unwrap().into(), Some(lock.clone()));
                return Ok(json!(self.id(&lock)));
            }
            "get" => {
                let lock = main.get(kind(slot));
                self.refs
                    .insert(a["name"].as_str().unwrap().into(), lock.clone());
                return Ok(self.optional_id(lock));
            }
            "equipment" => main.set_equipment(self.get(a, "name", passed)),
            "same" => {
                let l = self.get(a, "name", passed);
                let r = self.get(a, "other", passed);
                return Ok(json!(match (l, r) {
                    (Some(l), Some(r)) => l.same(&r),
                    (None, None) => true,
                    _ => false,
                }));
            }
            "message" => self
                .get(a, "name", passed)
                .ok_or(Error::Null)?
                .set_message(text(&a["value"])),
            "getMessage" => {
                return Ok(json!(
                    self.get(a, "name", passed)
                        .ok_or(Error::Null)?
                        .message()
                        .map(|s| s.to_vec())
                ));
            }
            "elapse" => {
                let lock = self.get(a, "name", passed).ok_or(Error::Null)?;
                let event = self.event(&a["event"]);
                let result = lock.set_elapse(event);
                return Ok(json!(self.id(&result)));
            }
            "lockref" => {
                let lock = self.get(a, "name", passed).ok_or(Error::Null)?;
                if a["forever"] == true {
                    lock.lock_forever(self, host)?;
                } else {
                    lock.lock(number(a, "ticks", 0), self, host)?;
                }
            }
            "unlockref" => self.get(a, "name", passed).ok_or(Error::Null)?.unlock(),
            "lockedref" => {
                return Ok(json!(
                    self.get(a, "name", passed)
                        .ok_or(Error::Null)?
                        .is_locked(self, host)?
                ));
            }
            "lock" => {
                if slot == "all" {
                    if a["forever"] == true {
                        main.lock_forever(self, host)?;
                    } else {
                        main.lock(number(a, "ticks", 0), self, host)?;
                    }
                } else {
                    let lock = main.get(kind(slot)).ok_or(Error::Null)?;
                    if a["forever"] == true {
                        lock.lock_forever(self, host)?;
                    } else {
                        main.lock_kind(kind(slot), number(a, "ticks", 0), self, host)?;
                    }
                }
            }
            "unlock" => {
                if slot == "all" {
                    main.unlock();
                } else {
                    main.unlock_kind(kind(slot))?;
                }
            }
            "locked" => {
                return if slot == "all" {
                    let movement = main.is_kind_locked(Kind::Movement, self, host)?;
                    let interaction = main.is_kind_locked(Kind::Interaction, self, host)?;
                    Ok(json!([movement, interaction]))
                } else {
                    Ok(json!(main.is_kind_locked(kind(slot), self, host)?))
                };
            }
            "custom" => {
                let event = self.event(&a["event"]);
                let lock =
                    main.lock_custom(key(&a["key"]), number(a, "ticks", 0), event, self, host)?;
                if let Some(name) = a["name"].as_str() {
                    self.refs.insert(name.into(), Some(lock.clone()));
                }
                return Ok(json!(self.id(&lock)));
            }
            "customLocked" => return Ok(json!(main.is_locked(&key(&a["key"]), self, host)?)),
            "remove" => {
                let k = key(&a["key"]);
                if a.get("event").is_some() {
                    let lock = main.custom().read().get(&k).cloned();
                    if let Some(lock) = lock {
                        let event = self.event(&a["event"]);
                        lock.set_elapse(event);
                    }
                }
                let node = if a.get("node").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(number(a, "node", 1))
                };
                main.unlock_custom(&k, a["cache"] != false, node, self, host)?;
            }
            op => panic!("unknown lock action {op}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&mut self) -> Value {
        let mut refs = BTreeMap::new();
        for (name, lock) in self.refs.clone() {
            refs.insert(name, self.optional_id(lock));
        }
        let equipment = self.optional_id(self.actions.get(Kind::Equipment));
        let entries = self
            .actions
            .custom()
            .read()
            .iter()
            .map(|(k, l)| (k.clone(), l.clone()))
            .collect::<Vec<_>>();
        let custom = entries
            .into_iter()
            .map(|(k, l)| json!([k, self.id(&l)]))
            .collect::<Vec<_>>();
        let locks=self.locks.iter().map(|l|json!({"expiration":l.expiration(),"message":l.message().map(|s|s.to_vec()),"event":l.elapse().map(|n|self.events[n].clone())})).collect::<Vec<_>>();
        json!({"refs":refs,"equipment":equipment,"custom":custom,"locks":locks,"tick":self.tick,"calls":self.calls})
    }
}
impl Context<Model, usize, i32> for Host {
    fn ticks(&mut self, m: &mut Model) -> Result<i32> {
        m.calls += 1;
        let call = m.calls;
        let value = m.input["samples"]
            .as_array()
            .and_then(|s| s.get(call - 1))
            .and_then(Value::as_i64)
            .map_or(m.tick, |n| n as i32);
        m.trace.push(json!(["clock", value]));
        if let Some(body) = m.hooks.remove(&call) {
            for action in body {
                m.act(self, &action, None)?;
            }
        }
        if number(&m.input, "failClockAt", -1) as i64 == call as i64 {
            return Err(Error::Binding("clock".into()));
        }
        Ok(value)
    }
    fn elapse(
        &mut self,
        event: Option<usize>,
        node: Option<i32>,
        lock: &Lock<usize>,
        _: &ActionLocks<usize>,
        m: &mut Model,
    ) -> Result<()> {
        let name = m.events[event.ok_or(Error::Null)?].clone();
        let id = m.id(lock);
        m.trace.push(json!(["elapse", name, node, id]));
        if let Some(body) = m.input["callbacks"][&name].as_array().cloned() {
            for action in body {
                m.act(self, &action, Some(lock))?;
            }
        }
        Ok(())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut model = Model::new(input);
    let mut host = Host;
    let mut steps = vec![];
    for action in input["actions"].as_array().unwrap() {
        let (result, error) = match model.act(&mut host, action, None) {
            Ok(v) => (v, Value::Null),
            Err(Error::Null) => (Value::Null, json!("Null")),
            Err(Error::Binding(s)) => (Value::Null, json!(s)),
        };
        let state = model.snapshot();
        steps.push(json!({"result":result,"error":error,"state":state,"trace":model.trace}));
    }
    json!(steps)
}
