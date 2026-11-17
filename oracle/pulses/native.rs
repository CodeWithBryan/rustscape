//! Scripted node/content/clock boundary for the original pulse classes.
use rs09_server::{
    definitions::item::Shared,
    pulses::{
        self, Checks, Class, Context, Error, Handle, Kind, Level, Manager, Pulse, Result, Runner,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn number(v: &Value, k: &str, d: i32) -> i32 {
    v.get(k).and_then(Value::as_i64).map_or(d, |v| v as i32)
}
fn flag(v: &Value, k: &str, d: bool) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(d)
}
fn kind(v: &Value) -> Option<Kind> {
    v.as_str().map(|s| match s {
        "STANDARD" => Kind::Standard,
        "STRONG" => Kind::Strong,
        "CUSTOM_1" => Kind::Custom1,
        "CUSTOM_2" => Kind::Custom2,
        _ => panic!("unknown kind {s}"),
    })
}
fn kind_name(v: Option<Kind>) -> Option<&'static str> {
    v.map(|k| match k {
        Kind::Standard => "STANDARD",
        Kind::Strong => "STRONG",
        Kind::Custom1 => "CUSTOM_1",
        Kind::Custom2 => "CUSTOM_2",
    })
}
fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::ConcurrentModification => "ConcurrentModification",
        Error::QueueFull => "QueueFull",
        Error::Binding(_) => "Binding",
    }
}
struct Env {
    input: Value,
    pulses: BTreeMap<String, Option<Handle>>,
    names: BTreeMap<usize, String>,
    configs: BTreeMap<usize, Value>,
    arrays: BTreeMap<String, Option<Checks>>,
    array_ids: BTreeMap<usize, usize>,
    nodes: BTreeMap<usize, bool>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    manager: Manager,
    runner: Runner,
    now: i64,
    step: i64,
    clock_index: usize,
    db: bool,
    logging: bool,
    dead: bool,
    bots: i32,
    others: i32,
    lengths: BTreeMap<String, i32>,
    counts: BTreeMap<String, i32>,
    unhandled: usize,
}
impl Env {
    fn name(&self, p: Option<&Handle>) -> Value {
        p.map_or(Value::Null, |p| json!(self.names[&p.identity()]))
    }
    fn desc(&self, p: &Handle) -> Value {
        self.configs
            .get(&p.identity())
            .cloned()
            .unwrap_or(Value::Null)
    }
    fn array(value: &Value) -> Option<Checks> {
        value
            .as_array()
            .map(|a| Shared::new(a.iter().map(|v| v.as_u64().map(|v| v as usize)).collect()))
    }
    fn hook(&mut self, who: &str, event: &str, args: Vec<Value>) -> Result<()> {
        let mut trace = vec![json!(who), json!(event)];
        trace.extend(args);
        self.events.push(json!(trace));
        let count = self.calls.entry(format!("{who}:{event}")).or_default();
        *count += 1;
        let count = *count;
        let rules = self.input["scripts"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for rule in rules {
            if rule["who"] == who && rule["event"] == event && number(&rule, "at", 1) == count {
                for action in rule["actions"].as_array().unwrap() {
                    self.action(action)?;
                }
            }
        }
        Ok(())
    }
    fn pulse_hook(&mut self, p: &Handle, event: &str, args: Vec<Value>) -> Result<()> {
        let name = self.names[&p.identity()].clone();
        self.hook(&name, event, args)
    }
    fn unhandled(p: &Handle) -> bool {
        matches!(p.read().class, Class::Unhandled { .. })
    }
    fn action(&mut self, a: &Value) -> Result<Value> {
        let p = a["pulse"]
            .as_str()
            .and_then(|s| self.pulses.get(s))
            .cloned()
            .flatten();
        let manager = self.manager.clone();
        let runner = self.runner.clone();
        let value = number(a, "value", 0);
        match a["op"].as_str().unwrap() {
            "update" => return Ok(json!(self.update(p.as_ref().ok_or(Error::Null)?, &mut ())?)),
            "body" => return Ok(json!(self.pulse(p.as_ref().ok_or(Error::Null)?, &mut ())?)),
            "runBase" => pulses::run_base(p.as_ref().ok_or(Error::Null)?, &mut (), self)?,
            "start" => self.start(p.as_ref().ok_or(Error::Null)?, &mut ())?,
            "stop" => self.stop(p.as_ref().ok_or(Error::Null)?, &mut ())?,
            "restart" => p.as_ref().ok_or(Error::Null)?.write().restart(),
            "running" => {
                p.as_ref().ok_or(Error::Null)?.write().running = a["value"].as_bool().unwrap()
            }
            "delay" => p.as_ref().ok_or(Error::Null)?.write().delay = value,
            "ticks" => p.as_ref().ok_or(Error::Null)?.write().ticks_passed = value,
            "inactive" => {
                return Ok(json!(
                    self.has_inactive(p.as_ref().ok_or(Error::Null)?, &mut ())?
                ));
            }
            "isRunning" => {
                return Ok(json!(
                    self.is_running(p.as_ref().ok_or(Error::Null)?, &mut ())?
                ));
            }
            "removeFor" => {
                return Ok(json!(self.remove_for(
                    p.as_ref().ok_or(Error::Null)?,
                    a["text"].as_str(),
                    &mut ()
                )?));
            }
            "setNode" => p.as_ref().ok_or(Error::Null)?.write().set_node(
                number(a, "index", 0),
                a["node"].as_u64().map(|v| v as usize),
            )?,
            "getNode" => {
                return Ok(json!(
                    p.as_ref()
                        .ok_or(Error::Null)?
                        .read()
                        .node(number(a, "index", 0))?
                ));
            }
            "replaceChecks" => {
                let array = if let Some(name) = a["array"].as_str() {
                    self.arrays.get(name).cloned().flatten()
                } else {
                    Self::array(&a["value"])
                };
                p.as_ref().ok_or(Error::Null)?.write().checks = array;
            }
            "node" => {
                self.nodes
                    .insert(number(a, "node", 0) as usize, a["value"].as_bool().unwrap());
            }
            "managerRun" => manager.run(
                p.as_ref(),
                a.get("kind").map_or(Some(Kind::Standard), kind),
                &runner,
                &mut (),
                self,
            )?,
            "clear" => manager.clear(&mut (), self)?,
            "clearKind" => {
                return Ok(json!(manager.clear_kind(
                    kind(&a["kind"]),
                    &mut (),
                    self
                )?));
            }
            "current" => return Ok(self.name(manager.current().as_ref())),
            "has" => return Ok(json!(manager.has_running(&mut (), self)?)),
            "moving" => return Ok(json!(manager.is_moving(&mut (), self)?)),
            "dead" => self.dead = a["value"].as_bool().unwrap(),
            "cancelDeath" => manager.cancel_death(&mut (), self)?,
            "unhandled" => {
                let p = manager.unhandled(
                    0,
                    a.get("kind").map_or(Some(Kind::Standard), kind),
                    &runner,
                    &mut (),
                    self,
                )?;
                let name = format!("u{}", self.unhandled);
                self.unhandled += 1;
                self.names.insert(p.identity(), name.clone());
                self.pulses.insert(name, Some(p.clone()));
                return Ok(self.name(Some(&p)));
            }
            "mapPut" => manager.put(kind(&a["kind"]), p),
            "submit" => runner.submit(p)?,
            "world" => runner.update_all(&mut (), self)?,
            "logging" => self.logging = a["value"].as_bool().unwrap(),
            "db" => self.db = a["value"].as_bool().unwrap(),
            "clock" => self.now = a["value"].as_i64().unwrap(),
            "step" => self.step = a["value"].as_i64().unwrap(),
            "fail" => return Err(Error::Binding("script".into())),
            other => panic!("unknown operation {other}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&mut self, value: Value, error: Option<&str>) -> Value {
        let mut states = BTreeMap::<String, Value>::new();
        for (name, p) in self.pulses.clone() {
            let Some(p) = p else {
                states.insert(name, Value::Null);
                continue;
            };
            let p = p.read();
            let (reference, checks) = if let Some(a) = &p.checks {
                let next = self.array_ids.len();
                (
                    Some(*self.array_ids.entry(a.identity()).or_insert(next)),
                    Some(a.read().clone()),
                )
            } else {
                (None, None)
            };
            states.insert(
                name,
                json!([
                    p.class_name,
                    p.running,
                    p.delay,
                    p.ticks_passed,
                    reference,
                    checks
                ]),
            );
        }
        let map = self
            .manager
            .entries()
            .iter()
            .map(|(k, p)| json!([kind_name(*k), self.name(p.as_ref())]))
            .collect::<Vec<_>>();
        let queue = self
            .runner
            .current()
            .iter()
            .map(|p| self.name(Some(p)))
            .collect::<Vec<_>>();
        json!({"result":value,"error":error,"pulses":states,"map":map,"current":self.name(self.manager.current().as_ref()),"queue":queue,"events":self.events,"clock":self.now,"clockIndex":self.clock_index,"totals":[self.bots,self.others],"lengths":self.lengths,"counts":self.counts,"logging":self.logging,"db":self.db})
    }
}
impl Context<()> for Env {
    fn pulse(&mut self, p: &Handle, _: &mut ()) -> Result<bool> {
        let class = p.read().class.clone();
        if let Class::Unhandled { player } = class {
            self.send_message(player, "Nothing interesting happens.", &mut ())?;
            return Ok(true);
        }
        self.pulse_hook(p, "pulse", vec![])?;
        Ok(flag(&self.desc(p), "finish", false))
    }
    fn start(&mut self, p: &Handle, _: &mut ()) -> Result<()> {
        if !Self::unhandled(p) {
            self.pulse_hook(p, "start", vec![])?;
        }
        if flag(&self.desc(p), "startBase", true) {
            p.write().running = true
        }
        Ok(())
    }
    fn stop(&mut self, p: &Handle, _: &mut ()) -> Result<()> {
        if !Self::unhandled(p) {
            self.pulse_hook(p, "stop", vec![])?;
        }
        if flag(&self.desc(p), "stopBase", true) {
            p.write().running = false
        }
        Ok(())
    }
    fn is_running(&mut self, p: &Handle, _: &mut ()) -> Result<bool> {
        if !Self::unhandled(p) {
            self.pulse_hook(p, "isRunning", vec![])?;
        }
        Ok(self.desc(p)["runningResult"]
            .as_bool()
            .unwrap_or_else(|| p.read().running))
    }
    fn has_inactive(&mut self, p: &Handle, s: &mut ()) -> Result<bool> {
        if !Self::unhandled(p) {
            self.pulse_hook(p, "inactive", vec![])?;
        }
        if let Some(value) = self.desc(p)["inactiveResult"].as_bool() {
            return Ok(value);
        }
        pulses::has_inactive_base(p, s, self)
    }
    fn update(&mut self, p: &Handle, s: &mut ()) -> Result<bool> {
        if !Self::unhandled(p) {
            self.pulse_hook(p, "update", vec![])?;
        }
        if let Some(value) = self.desc(p)["updateResult"].as_bool() {
            return Ok(value);
        }
        pulses::update_base(p, s, self)
    }
    fn remove_for(&mut self, p: &Handle, t: Option<&str>, _: &mut ()) -> Result<bool> {
        if !Self::unhandled(p) {
            self.pulse_hook(p, "removeFor", vec![json!(t)])?;
        }
        Ok(flag(&self.desc(p), "removeResult", true))
    }
    fn node_active(&mut self, node: usize, _: &mut ()) -> Result<bool> {
        self.hook(&format!("node:{node}"), "active", vec![])?;
        self.nodes.get(&node).copied().ok_or(Error::Null)
    }
    fn exception(&mut self, e: &Error, _: &mut ()) -> Result<()> {
        self.events.push(json!(["exception", error_name(e)]));
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> Result<()> {
        self.hook("scripts", "weak", vec![])
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> Result<()> {
        self.hook("scripts", "normal", vec![])
    }
    fn is_dead(&mut self, _: &mut ()) -> Result<bool> {
        self.hook("entity", "dead", vec![])?;
        Ok(self.dead)
    }
    fn send_message(&mut self, _: usize, message: &str, _: &mut ()) -> Result<()> {
        self.hook("player", "message", vec![json!(message)])
    }
    fn now_millis(&mut self, _: &mut ()) -> Result<i64> {
        let value = self.input["times"]
            .get(self.clock_index)
            .and_then(Value::as_i64)
            .unwrap_or(self.now);
        self.clock_index += 1;
        self.now = self.now.wrapping_add(self.step);
        self.hook("clock", "sample", vec![json!(value)])?;
        Ok(value)
    }
    fn log(&mut self, l: Level, t: String, _: &mut ()) -> Result<()> {
        self.hook(
            "log",
            match l {
                Level::Warn => "Warn",
                Level::Error => "Error",
            },
            vec![json!(t)],
        )
    }
    fn pulse_length(&mut self, c: &str, m: i32, _: &mut ()) -> Result<()> {
        self.hook("metrics", "length", vec![json!(c), json!(m)])?;
        if self.db {
            let p = self.lengths.entry(c.into()).or_default();
            *p = p.wrapping_add(m)
        }
        Ok(())
    }
    fn count_pulse(&mut self, c: &str, _: &mut ()) -> Result<()> {
        self.hook("metrics", "count", vec![json!(c)])?;
        if self.db {
            let p = self.counts.entry(c.into()).or_default();
            *p = p.wrapping_add(1)
        }
        Ok(())
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        self.logging
    }
    fn set_bot_total(&mut self, n: i32, _: &mut ()) -> Result<()> {
        self.hook("metrics", "bots", vec![json!(n)])?;
        self.bots = n;
        Ok(())
    }
    fn set_other_total(&mut self, n: i32, _: &mut ()) -> Result<()> {
        self.hook("metrics", "others", vec![json!(n)])?;
        self.others = n;
        Ok(())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let hashes = std::array::from_fn(|i| input["hashes"][i].as_i64().unwrap() as i32);
    let mut env = Env {
        input: input.clone(),
        pulses: BTreeMap::new(),
        names: BTreeMap::new(),
        configs: BTreeMap::new(),
        arrays: BTreeMap::new(),
        array_ids: BTreeMap::new(),
        nodes: BTreeMap::from([(0, true), (1, true), (2, false)]),
        events: Vec::new(),
        calls: BTreeMap::new(),
        manager: Manager::new(hashes),
        runner: Runner::default(),
        now: input["clock"].as_i64().unwrap_or(0),
        step: input["step"].as_i64().unwrap_or(0),
        clock_index: 0,
        db: flag(input, "db", true),
        logging: flag(input, "logging", true),
        dead: false,
        bots: number(input, "botTotal", 17),
        others: number(input, "otherTotal", 19),
        lengths: BTreeMap::new(),
        counts: BTreeMap::new(),
        unhandled: 0,
    };
    if let Some(arrays) = input["arrays"].as_object() {
        for (name, value) in arrays {
            let a = Env::array(value);
            if let Some(a) = &a {
                let next = env.array_ids.len();
                env.array_ids.insert(a.identity(), next);
            }
            env.arrays.insert(name.clone(), a);
        }
    }
    for (name, d) in input["pulses"].as_object().unwrap() {
        if d.is_null() {
            env.pulses.insert(name.clone(), None);
            continue;
        }
        let tag = d["class"].as_str().unwrap_or("base");
        let (class, class_name) = match tag {
            "movement" => (Class::Movement, "core.game.interaction.MovementPulse"),
            "combat" => (Class::Combat, "core.game.node.entity.combat.CombatPulse"),
            "script" => (Class::Other, "oracle.ScriptAPIPulse"),
            "bot" => (
                Class::Bot {
                    script: (!flag(d, "scriptNull", false))
                        .then(|| "core.game.bots.GeneralBotCreator$Script".into()),
                },
                "core.game.bots.GeneralBotCreator$BotScriptPulse",
            ),
            _ => (Class::Other, "oracle.TestPulse"),
        };
        let mut checks = if let Some(name) = d["array"].as_str() {
            env.arrays.get(name).cloned().flatten()
        } else {
            Env::array(&d["checks"])
        };
        let delay = if tag == "base" && d["ctor"] == "default" {
            checks = None;
            1
        } else {
            if tag == "base" && d["ctor"].is_string() {
                checks = None
            }
            number(d, "delay", 1)
        };
        let mut p = Pulse::new(delay, checks, class, class_name.into());
        p.running = flag(d, "running", true);
        p.ticks_passed = number(d, "ticks", 0);
        let p = Shared::new(p);
        env.names.insert(p.identity(), name.clone());
        env.configs.insert(p.identity(), d.clone());
        env.pulses.insert(name.clone(), Some(p));
    }
    let mut states = Vec::new();
    for a in input["actions"].as_array().unwrap() {
        env.events.clear();
        let (result, error) = match env.action(a) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(error_name(&e))),
        };
        states.push(env.snapshot(result, error));
    }
    json!(states)
}
