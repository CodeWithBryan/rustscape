//! Explicit content/node/path/interface adapters for differential script tests.
use rs09_server::{
    definitions::item::Shared,
    scripts::{
        Arguments, Context, Destination, EntityKind, Error, Execution, Form, Handle, Level,
        Location, Node, NodeKind, Path, Processor, Result, Script, Strength,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn number(v: &Value, k: &str, d: i32) -> i32 {
    v.get(k).and_then(Value::as_i64).map_or(d, |v| v as i32)
}
fn strength(s: &str) -> Strength {
    match s {
        "WEAK" => Strength::Weak,
        "NORMAL" => Strength::Normal,
        "STRONG" => Strength::Strong,
        "SOFT" => Strength::Soft,
        _ => panic!("bad strength {s}"),
    }
}
fn error(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Binding(_) => "Binding",
    }
}
struct Env {
    main: Option<Processor>,
    processors: BTreeMap<String, Processor>,
    input: Value,
    scripts: BTreeMap<String, Option<Handle>>,
    names: BTreeMap<usize, String>,
    executors: Vec<String>,
    locations: BTreeMap<String, Location>,
    properties: BTreeMap<(usize, String), Option<Location>>,
    nodes: BTreeMap<usize, Value>,
    own: Option<Location>,
    events: Vec<Value>,
    counts: BTreeMap<String, i32>,
    tick: i32,
    step: i32,
    movement: i32,
    lock: i32,
    created: usize,
    modal: bool,
    opened: bool,
    chatbox: bool,
    stun: bool,
    running: bool,
    sight: bool,
}
impl Env {
    fn loc(&self, v: &Value) -> Option<Location> {
        if let Some(name) = v.as_str() {
            return self.locations.get(name).cloned();
        }
        v.as_array().map(|a| {
            Location::new(
                a[0].as_i64().unwrap() as i32,
                a[1].as_i64().unwrap() as i32,
                a[2].as_i64().unwrap() as i32,
            )
        })
    }
    fn xyz(v: Option<&Location>) -> Value {
        v.map_or(Value::Null, |v| json!(v.coordinates()))
    }
    fn key(&mut self, s: &str) -> usize {
        if let Some(k) = self.executors.iter().position(|key| key == s) {
            k
        } else {
            self.executors.push(s.into());
            self.executors.len() - 1
        }
    }
    fn name(&mut self, s: Option<&Handle>) -> Value {
        let Some(s) = s else { return Value::Null };
        let name = self
            .names
            .entry(s.identity())
            .or_insert_with(|| {
                let n = format!("created{}", self.created);
                self.created += 1;
                n
            })
            .clone();
        self.scripts.insert(name.clone(), Some(s.clone()));
        json!(name)
    }
    fn hook(&mut self, who: &str, event: &str, args: Vec<Value>, p: &Processor) -> Result<()> {
        let mut trace = vec![json!(who), json!(event)];
        trace.extend(args);
        self.events.push(json!(trace));
        let count = self.counts.entry(format!("{who}:{event}")).or_default();
        *count += 1;
        let count = *count;
        let rules = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for rule in rules {
            if rule["who"] == who && rule["event"] == event && number(&rule, "at", 1) == count {
                for a in rule["actions"].as_array().unwrap() {
                    let main = self.main.clone().unwrap_or_else(|| p.clone());
                    self.action(&main, a)?;
                }
            }
        }
        Ok(())
    }
    fn entity_flag(&mut self, name: &str, p: &Processor) -> Result<bool> {
        self.hook("entity", name, vec![], p)?;
        Ok(match name {
            "modal" => self.modal,
            "opened" => self.opened,
            "chatbox" => self.chatbox,
            "stun" => self.stun,
            "running" => self.running,
            _ => panic!("unknown flag"),
        })
    }
    fn action(&mut self, p: &Processor, a: &Value) -> Result<Value> {
        let selected = a["processor"]
            .as_str()
            .and_then(|n| self.processors.get(n))
            .cloned()
            .unwrap_or_else(|| p.clone());
        let p = &selected;
        let api = self.main.clone().unwrap_or_else(|| p.clone());
        let script = a["script"]
            .as_str()
            .and_then(|s| self.scripts.get(s))
            .cloned()
            .flatten();
        let value = number(a, "value", 0);
        let node = a["node"].as_u64().map(|v| v as usize);
        let st = strength(a["strength"].as_str().unwrap_or("WEAK"));
        match a["op"].as_str().unwrap() {
            "aliasProcessor" => {
                self.processors
                    .insert(a["name"].as_str().unwrap().into(), p.clone());
            }
            "newProcessor" => {
                self.processors.insert(
                    a["name"].as_str().unwrap().into(),
                    Processor::with_entity(0),
                );
            }
            "sameProcessor" => {
                return Ok(json!(
                    p.same(&self.processors[a["other"].as_str().unwrap()])
                ));
            }
            "processorEntity" => return Ok(json!(p.entity())),
            "readProcessor" => {
                let approach = self.name(p.approach().as_ref());
                let operable = self.name(p.operable().as_ref());
                let current = self.name(p.current().as_ref());
                let queue = p
                    .queue()
                    .iter()
                    .map(|h| self.name(Some(h)))
                    .collect::<Vec<_>>();
                return Ok(json!([
                    p.delay(),
                    p.interacted(),
                    p.range_called(),
                    p.range(),
                    p.persistent(),
                    approach,
                    operable,
                    current,
                    p.target(),
                    Self::xyz(p.target_destination().as_ref()),
                    queue
                ]));
            }
            "add" => p.add_to_queue(script.as_ref(), st, &mut (), self)?,
            "queue" => return Ok(json!(p.process_queue(&mut (), self)?)),
            "execute" => return Ok(json!(p.execute_script(script.as_ref(), &mut (), self)?)),
            "interact" => p.process_interact(script.as_ref(), &mut (), self)?,
            "setInteraction" => p.set_interaction(node, script.as_ref(), &mut (), self)?,
            "pre" => p.pre_movement(&mut (), self)?,
            "post" => p.post_movement(a["value"].as_bool().unwrap(), &mut (), self)?,
            "reset" => p.reset(&mut (), self)?,
            "weak" => p.remove_weak(),
            "normal" => p.remove_normal(),
            "has" => return Ok(json!(p.has_type(st))),
            "approach" => return Ok(json!(p.in_approach(script.as_ref(), &mut (), self)?)),
            "operable" => return Ok(json!(p.in_operable(&mut (), self)?)),
            "active" => return Ok(self.name(p.active().as_ref())),
            "state" => script.as_ref().ok_or(Error::Null)?.write().state = value,
            "next" => script.as_ref().ok_or(Error::Null)?.write().next_execution = value,
            "tick" => self.tick = value,
            "step" => self.step = value,
            "delay" => p.set_delay(value),
            "destination" => p.set_target_destination(self.loc(&a["value"])),
            "own" => self.own = self.loc(&a["value"]),
            "moveClock" => self.movement = value,
            "modal" => self.modal = a["value"].as_bool().unwrap(),
            "opened" => self.opened = a["value"].as_bool().unwrap(),
            "chatbox" => self.chatbox = a["value"].as_bool().unwrap(),
            "stun" => self.stun = a["value"].as_bool().unwrap(),
            "running" => self.running = a["value"].as_bool().unwrap(),
            "sight" => self.sight = a["value"].as_bool().unwrap(),
            "nodeActive" => {
                self.nodes.get_mut(&node.unwrap()).unwrap()["active"] = a["value"].clone()
            }
            "mutateLocation" => {
                let l = self.locations.get(a["name"].as_str().unwrap()).unwrap();
                let v = a["value"].as_array().unwrap();
                l.set(
                    v[0].as_i64().unwrap() as i32,
                    v[1].as_i64().unwrap() as i32,
                    v[2].as_i64().unwrap() as i32,
                );
            }
            "delayScript" => return Ok(json!(api.delay_script(value, &mut (), self)?)),
            "delayEntity" => api.delay_entity(value, &mut (), self)?,
            "range" => api.set_range(value),
            "clearScripts" => {
                api.reset(&mut (), self)?;
                return Ok(json!(true));
            }
            "restartScript" => return Ok(json!(api.restart_script(&mut (), self)?)),
            "keepRunning" => return Ok(json!(api.keep_running(&mut (), self)?)),
            "stopExecuting" => return Ok(json!(api.stop_executing(&mut (), self)?)),
            "apiQueue" => {
                let key = self.key(a["execution"].as_str().unwrap());
                api.queue_script(
                    Some(key),
                    number(a, "delay", 1),
                    st,
                    a["persist"] == true,
                    &mut (),
                    self,
                )?;
            }
            "fail" => return Err(Error::Binding("hook".into())),
            op => panic!("unknown {op}"),
        }
        Ok(Value::Null)
    }
    fn observe(&mut self, p: &Processor, result: Value, e: Option<&str>) -> Value {
        for s in p.queue() {
            self.name(Some(&s));
        }
        for s in [&p.approach(), &p.operable(), &p.current()] {
            self.name(s.as_ref());
        }
        let scripts = self
            .scripts
            .iter()
            .map(|(name, s)| {
                let value = s.as_ref().map(|s| {
                    let s = s.read();
                    json!([
                        s.class_name,
                        s.execution.map(|key| &self.executors[key]),
                        s.persist,
                        s.state,
                        s.next_execution
                    ])
                });
                (name.clone(), value)
            })
            .collect::<BTreeMap<_, _>>();
        let queue = p
            .queue()
            .iter()
            .map(|s| self.name(Some(s)))
            .collect::<Vec<_>>();
        let approach = self.name(p.approach().as_ref());
        let operable = self.name(p.operable().as_ref());
        let current = self.name(p.current().as_ref());
        let active = self.name(p.active().as_ref());
        let locs = self
            .locations
            .iter()
            .map(|(name, l)| (name.clone(), Self::xyz(Some(l))))
            .collect::<BTreeMap<_, _>>();
        json!({"result":result,"error":e,"queue":queue,"scripts":scripts,"approach":approach,"operable":operable,"current":current,"active":active,"target":p.target(),"delay":p.delay(),"interacted":p.interacted(),"rangeCalled":p.range_called(),"range":p.range(),"persistent":p.persistent(),"destination":Self::xyz(p.target_destination().as_ref()),"own":Self::xyz(self.own.as_ref()),"locations":locs,"tick":self.tick,"movement":self.movement,"lock":self.lock,"events":self.events})
    }
}
impl Context<()> for Env {
    fn entity_kind(&self, _: &()) -> EntityKind {
        match self.input["kind"].as_str() {
            Some("ai") => EntityKind::AiPlayer,
            Some("other") => EntityKind::Other,
            _ => EntityKind::Player,
        }
    }
    fn ticks(&mut self, p: &Processor, _: &mut ()) -> Result<i32> {
        let value = self.tick;
        self.tick = self.tick.wrapping_add(self.step);
        self.hook("world", "ticks", vec![json!(value)], p)?;
        Ok(value)
    }
    fn delayed(&mut self, p: &Processor, s: &mut ()) -> Result<bool> {
        self.hook("entity", "delayed", vec![], p)?;
        let d = self.main.as_ref().unwrap_or(p).delay();
        Ok(d > self.ticks(p, s)?)
    }
    fn stunned(&mut self, p: &Processor, _: &mut ()) -> Result<bool> {
        self.entity_flag("stun", p)
    }
    fn modal_open(&mut self, p: &Processor, _: &mut ()) -> Result<bool> {
        self.entity_flag("modal", p)
    }
    fn interface_open(&mut self, p: &Processor, _: &mut ()) -> Result<bool> {
        self.entity_flag("opened", p)
    }
    fn chatbox_open(&mut self, p: &Processor, _: &mut ()) -> Result<bool> {
        self.entity_flag("chatbox", p)
    }
    fn running(&mut self, p: &Processor, _: &mut ()) -> Result<bool> {
        self.entity_flag("running", p)
    }
    fn close_interfaces(&mut self, p: &Processor, _: &mut ()) -> Result<()> {
        self.hook("entity", "close", vec![], p)
    }
    fn reset_animator(&mut self, p: &Processor, _: &mut ()) -> Result<()> {
        self.hook("entity", "animatorReset", vec![], p)
    }
    fn set_movement_clock(&mut self, v: i32, _: &Processor, _: &mut ()) -> Result<()> {
        self.movement = v;
        Ok(())
    }
    fn finished_moving(&mut self, p: &Processor, s: &mut ()) -> Result<bool> {
        let old = self.movement;
        Ok(old < self.ticks(p, s)?)
    }
    fn entity_location(&mut self, p: &Processor, _: &mut ()) -> Result<Option<Location>> {
        self.hook("entity", "location", vec![], p)?;
        Ok(self.own.clone())
    }
    fn node_kind(&self, n: Node, _: &()) -> NodeKind {
        match self.nodes[&n]["kind"].as_str() {
            Some("npc") => NodeKind::Npc,
            Some("scenery") => NodeKind::Scenery,
            Some("ground") => NodeKind::GroundItem,
            _ => NodeKind::Other,
        }
    }
    fn node_active(&mut self, n: Node, p: &Processor, _: &mut ()) -> Result<bool> {
        self.hook(&format!("node:{n}"), "active", vec![], p)?;
        Ok(self.nodes[&n]["active"] != false)
    }
    fn node_description(&mut self, n: Option<Node>, p: &Processor, _: &mut ()) -> Result<String> {
        if let Some(n) = n {
            self.hook(&format!("node:{n}"), "describe", vec![], p)?;
            Ok(format!("node:{n}"))
        } else {
            Ok("null".into())
        }
    }
    fn node_location(&mut self, n: Node, p: &Processor, s: &mut ()) -> Result<Option<Location>> {
        if n == 0 {
            return self.entity_location(p, s);
        }
        self.hook(&format!("node:{n}"), "location", vec![], p)?;
        Ok(self
            .properties
            .get(&(n, "location".into()))
            .cloned()
            .flatten())
    }
    fn center_location(&mut self, n: Node, p: &Processor, _: &mut ()) -> Result<Option<Location>> {
        self.hook(&format!("node:{n}"), "center", vec![], p)?;
        Ok(self
            .properties
            .get(&(n, "center".into()))
            .cloned()
            .flatten())
    }
    fn face_location(
        &mut self,
        n: Node,
        l: Option<Location>,
        p: &Processor,
        _: &mut (),
    ) -> Result<Option<Location>> {
        self.hook(&format!("node:{n}"), "face", vec![Self::xyz(l.as_ref())], p)?;
        Ok(self.properties.get(&(n, "face".into())).cloned().flatten())
    }
    fn destination(
        &mut self,
        f: Destination,
        n: Node,
        p: &Processor,
        _: &mut (),
    ) -> Result<Option<Location>> {
        self.hook(
            "dest",
            match f {
                Destination::Entity => "Entity",
                Destination::Item => "Item",
            },
            vec![json!(n)],
            p,
        )?;
        Ok(self
            .properties
            .get(&(n, "destination".into()))
            .cloned()
            .flatten())
    }
    fn pathfind(&mut self, n: Option<Node>, p: &Processor, _: &mut ()) -> Result<Path> {
        self.hook("path", "find", vec![json!(n)], p)?;
        let n = n.ok_or(Error::Null)?;
        let d = &self.nodes[&n];
        let points = if let Some(points) = d["points"].as_array() {
            points
                .iter()
                .map(|v| [v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32])
                .collect()
        } else {
            self.properties
                .get(&(n, "location".into()))
                .cloned()
                .flatten()
                .map_or(Vec::new(), |l| {
                    let [x, y, _] = l.coordinates();
                    vec![[x, y]]
                })
        };
        Ok(Path {
            move_near: d["near"] == true,
            points,
        })
    }
    fn line_of_sight(&mut self, l: &Location, p: &Processor, _: &mut ()) -> Result<bool> {
        self.hook("los", "test", vec![Self::xyz(Some(l))], p)?;
        Ok(self.sight)
    }
    fn face(&mut self, l: Location, p: &Processor, _: &mut ()) -> Result<()> {
        self.hook("entity", "face", vec![Self::xyz(Some(&l))], p)
    }
    fn message(&mut self, t: &str, p: &Processor, _: &mut ()) -> Result<()> {
        self.hook("entity", "message", vec![json!(t)], p)
    }
    fn execution_equals(
        &mut self,
        l: Option<Execution>,
        r: Option<Execution>,
        p: &Processor,
        _: &mut (),
    ) -> Result<bool> {
        let Some(l) = l else { return Ok(r.is_none()) };
        let left = self.executors[l].clone();
        let right = r.map(|r| self.executors[r].clone());
        self.hook(&format!("exec:{left}"), "equals", vec![json!(right)], p)?;
        let group = |key: &str| {
            self.input["executors"][key]["group"]
                .as_str()
                .unwrap_or(key)
                .to_owned()
        };
        Ok(right.is_some_and(|r| group(&left) == group(&r)))
    }
    fn execute(
        &mut self,
        e: Option<Execution>,
        a: Arguments,
        p: &Processor,
        _: &mut (),
    ) -> Result<bool> {
        let e = e.ok_or(Error::Null)?;
        let key = self.executors[e].clone();
        let args = match a {
            Arguments::Void { stage } => vec![stage],
            Arguments::Interaction { target, stage } => vec![0, target as i32, stage],
            Arguments::UseWith { used, with, stage } => vec![0, used as i32, with as i32, stage],
        };
        if let Err(e) = self.hook(&format!("exec:{key}"), "invoke", vec![json!(args)], p) {
            return Err(Error::Binding(format!("callback:{}\n", error(&e))));
        }
        let cfg = &self.input["executors"][&key];
        Ok(cfg["byStage"][args.last().unwrap().to_string()]
            .as_bool()
            .or_else(|| cfg["result"].as_bool())
            .unwrap_or(false))
    }
    fn exception_text(&mut self, e: &Error, _: &Processor, _: &mut ()) -> Result<String> {
        Ok(match e {
            Error::Binding(s) if s.starts_with("callback:") => s.clone(),
            _ => format!("callback:{}\n", error(e)),
        })
    }
    fn log(&mut self, owner: &str, l: Level, t: String, p: &Processor, _: &mut ()) -> Result<()> {
        self.hook(
            "log",
            match l {
                Level::Fine => "Fine",
                Level::Error => "Error",
            },
            vec![json!(owner), json!(t)],
            p,
        )
    }
    fn lock_entity(&mut self, t: i32, p: &Processor, _: &mut ()) -> Result<()> {
        self.hook("entity", "lock", vec![json!(t)], p)?;
        self.lock = t;
        Ok(())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut e = Env {
        main: None,
        processors: BTreeMap::new(),
        input: input.clone(),
        scripts: BTreeMap::new(),
        names: BTreeMap::new(),
        executors: Vec::new(),
        locations: BTreeMap::new(),
        properties: BTreeMap::new(),
        nodes: BTreeMap::from([(0, json!({"active":true}))]),
        own: None,
        events: Vec::new(),
        counts: BTreeMap::new(),
        tick: number(input, "tick", 0),
        step: number(input, "step", 0),
        movement: number(input, "movement", 0),
        lock: 0,
        created: 0,
        modal: input["modal"] == true,
        opened: input["opened"] == true,
        chatbox: input["chatbox"] == true,
        stun: input["stun"] == true,
        running: input["running"] == true,
        sight: input["sight"] != false,
    };
    if let Some(locs) = input["locations"].as_object() {
        for (name, v) in locs {
            let l = e.loc(v).unwrap();
            e.locations.insert(name.clone(), l);
        }
    }
    e.own = e.loc(input.get("own").unwrap_or(&json!([0, 0, 0])));
    for n in 1..=4 {
        let d = input["nodes"][n.to_string()]
            .as_object()
            .map_or_else(|| json!({"location":[n,0,0]}), |d| json!(d));
        let loc = e.loc(&d["location"]);
        e.properties.insert((n, "location".into()), loc.clone());
        for k in ["center", "face", "destination"] {
            let l = d.get(k).map_or_else(|| loc.clone(), |v| e.loc(v));
            e.properties.insert((n, k.into()), l);
        }
        e.nodes.insert(n, d);
    }
    for (name, d) in input["scripts"].as_object().unwrap() {
        if d.is_null() {
            e.scripts.insert(name.clone(), None);
            continue;
        }
        let execution = if let Some(key) = d["execution"].as_str() {
            Some(e.key(key))
        } else if d.get("execution").is_none() {
            Some(e.key("f0"))
        } else {
            None
        };
        let st = strength(d["strength"].as_str().unwrap_or("WEAK"));
        let form = match d["form"].as_str() {
            Some("basic") => Form::Basic,
            Some("interaction") => Form::Interaction {
                distance: number(d, "distance", 1),
            },
            Some("use") => Form::UseWith {
                distance: number(d, "distance", 1),
                used: 1,
                with: 2,
            },
            Some("queuedUse") => Form::QueuedUseWith {
                strength: st,
                used: 1,
                with: 2,
            },
            _ => Form::Queued { strength: st },
        };
        let mut s = Script::new(form, execution, d["persist"] == true);
        s.state = number(d, "state", 0);
        s.next_execution = number(d, "next", 0);
        let s = Shared::new(s);
        e.names.insert(s.identity(), name.clone());
        e.scripts.insert(name.clone(), Some(s));
    }
    let p = Processor::with_entity(0);
    e.main = Some(p.clone());
    e.processors.insert("main".into(), p.clone());
    let mut states = Vec::new();
    for a in input["actions"].as_array().unwrap() {
        e.events.clear();
        let (v, error) = match e.action(&p, a) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(error(&e))),
        };
        states.push(e.observe(&p, v, error));
    }
    json!(states)
}
