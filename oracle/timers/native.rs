//! Scripted world/content boundary for the original timer differential corpus.
use rs09_game_core::java_json::{self, Json};
use rs09_server::timers::registry::{Factories, Level, Registry};
use rs09_server::{
    definitions::item::Shared,
    timers::{self, Context, Error, Flag, Handle, Manager, Result, Timer, TimerList},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn number(value: &Value, key: &str, default: i32) -> i32 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map_or(default, |v| v as i32)
}
fn text<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn error_name(error: &Error) -> &'static str {
    match error {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Number => "Number",
        Error::Index => "Index",
        Error::ConcurrentModification => "ConcurrentModification",
        Error::Binding(_) => "Binding",
    }
}
pub struct Harness {
    request: Value,
    tick: i32,
    step: i32,
    player: bool,
    modal: bool,
    delay: i32,
    handles: BTreeMap<String, Option<Handle>>,
    names: BTreeMap<usize, String>,
    configs: BTreeMap<usize, Value>,
    counts: BTreeMap<String, i32>,
    created: usize,
    events: Vec<Value>,
    pub root: Json,
    registry: Registry,
}
pub trait ScriptState {
    fn action(&mut self, action: &Value) -> Result<()>;
}
impl ScriptState for () {
    fn action(&mut self, action: &Value) -> Result<()> {
        panic!("unknown action {action}")
    }
}
impl ScriptState for rs09_server::player::core_data::CoreData {
    fn action(&mut self, action: &Value) -> Result<()> {
        match action["op"].as_str().unwrap() {
            "coreVarp" => {
                let index = number(action, "index", 0);
                self.varps.insert(index, number(action, "value", 0));
                if let Some(save) = action.get("save") {
                    self.save_varps.insert(index, save.as_bool());
                }
            }
            "coreSecondary" => self.use_secondary = action["value"].as_bool().unwrap(),
            "coreLocation" => {
                self.location = action["value"].as_array().map(|v| {
                    rs09_server::player::attributes::AttributeLocation::new(
                        v[0].as_i64().unwrap() as i32,
                        v[1].as_i64().unwrap() as i32,
                        v[2].as_i64().unwrap() as i32,
                    )
                })
            }
            _ => panic!("unknown core action {action}"),
        }
        Ok(())
    }
}
fn class_match(timer: &Timer, class: &str) -> bool {
    match class {
        "PlainTimer" => matches!(
            timer.class_name.as_str(),
            "PlainTimer" | "ChildTimer" | "FactoryTimer" | "FactoryChild"
        ),
        "PersistentTimer" => timer.persistent,
        "ChildTimer" | "ChildPersistent" | "FactoryChild" => timer.class_name == class,
        _ => true,
    }
}
impl Harness {
    fn registry_action<S: ScriptState>(
        &mut self,
        manager: &mut Manager,
        state: &mut S,
        action: &Value,
    ) -> Result<Value> {
        let registry = self.registry.clone();
        let timer = action["timer"]
            .as_str()
            .and_then(|label| self.handles.get(label))
            .cloned()
            .flatten();
        let args = action["args"].as_array().cloned().unwrap_or_default();
        let id = text(action, "id", "missing");
        let class = text(action, "class", "RSTimer");
        let result = match action["op"].as_str().unwrap() {
            "registryRegister" => {
                registry.register(timer.as_ref(), manager, state, self)?;
                None
            }
            "registryPut" => {
                registry.put(id.into(), timer);
                None
            }
            "autoAppend" => {
                registry.automatic.write().push(timer);
                None
            }
            "autoClear" => {
                registry.automatic.write().clear();
                None
            }
            "auto" => {
                registry.add_automatic(manager, state, self)?;
                None
            }
            "registryGet" => registry.instance(id, &args, manager, state, self)?,
            "registryType" => {
                registry.instance_where(|t| class_match(t, class), &args, manager, state, self)?
            }
            "spawnType" => Some(registry.spawn_where(
                |t| class_match(t, class),
                &args,
                manager,
                state,
                self,
            )?),
            "getOrStart" => registry.get_or_start(id, &args, manager, state, self)?,
            "getOrStartType" => Some(registry.get_or_start_where(
                |t| class_match(t, class),
                &args,
                manager,
                state,
                self,
            )?),
            "has" => return Ok(json!(manager.get_identifier(id)?.is_some())),
            _ => unreachable!(),
        };
        Ok(self.name(result.as_ref()))
    }
    fn record_log(&mut self, level: Level, message: String) -> Result<()> {
        self.events
            .push(json!(["registryLog", format!("{level:?}"), message]));
        let count = self.counts.entry("registryLog".into()).or_default();
        *count += 1;
        if number(&self.request, "logFailAt", -1) == *count {
            return Err(Error::Binding("log".into()));
        }
        Ok(())
    }
    fn construct(&mut self, desc: &Value, name: String, fresh: bool) -> Handle {
        let class = text(desc, "class", "PlainTimer");
        let mut timer = Timer::new(
            number(desc, "interval", 0),
            text(desc, "id", "timer").into(),
            class.into(),
            matches!(
                class,
                "PersistentTimer" | "ChildPersistent" | "FactoryPersistent"
            ),
        );
        timer.is_soft = desc["soft"] == true;
        timer.is_auto = desc["auto"] == true;
        if desc["death"] == true {
            timer.flags.push(Flag::ClearOnDeath);
        }
        if !fresh {
            timer.next_execution = number(desc, "next", 0);
            timer.last_execution = number(desc, "last", 0);
        }
        let timer = Shared::new(timer);
        self.names.insert(timer.identity(), name.clone());
        self.configs.insert(timer.identity(), desc.clone());
        self.handles.insert(name, Some(timer.clone()));
        timer
    }
    fn name(&self, timer: Option<&Handle>) -> Value {
        timer.map_or(Value::Null, |timer| json!(self.names[&timer.identity()]))
    }
    fn list(manager: &Manager, name: &str) -> TimerList {
        match name {
            "active" => manager.active.clone(),
            "pending" => manager.pending.clone(),
            _ => manager.removing.clone(),
        }
    }
    fn hook<S: ScriptState>(
        &mut self,
        timer: &Handle,
        event: &str,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<()> {
        let name = self.names[&timer.identity()].clone();
        self.events.push(json!([event, name]));
        let count = self.counts.entry(format!("{name}:{event}")).or_default();
        *count += 1;
        let count = *count;
        let rules = self.request["scripts"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for rule in rules {
            if rule["timer"] == name && rule["event"] == event && number(&rule, "at", 1) == count {
                for action in rule["actions"].as_array().unwrap() {
                    self.action(manager, state, action)?;
                }
            }
        }
        Ok(())
    }
    fn object(raw: &str) -> Result<Json> {
        let value = java_json::parse(raw).map_err(|_| Error::Binding("JSON parse".into()))?;
        if matches!(value, Json::Object(_)) {
            Ok(value)
        } else {
            Err(Error::Type)
        }
    }
    pub fn action<S: ScriptState>(
        &mut self,
        manager: &mut Manager,
        state: &mut S,
        action: &Value,
    ) -> Result<Value> {
        if matches!(
            action["op"].as_str().unwrap(),
            "registryRegister"
                | "registryPut"
                | "autoAppend"
                | "autoClear"
                | "auto"
                | "registryGet"
                | "registryType"
                | "spawnType"
                | "getOrStart"
                | "getOrStartType"
                | "has"
        ) {
            return self.registry_action(manager, state, action);
        }
        let timer = action["timer"]
            .as_str()
            .and_then(|name| self.handles.get(name))
            .cloned()
            .flatten();
        let value = number(action, "value", 0);
        match action["op"].as_str().unwrap() {
            "register" => manager.register(timer.as_ref(), state, self)?,
            "optionalRegister" => manager.register_optional(timer.as_ref(), state, self)?,
            "remove" => manager.remove(timer.as_ref(), state, self)?,
            "removeId" => manager.remove_identifier(action["id"].as_str().unwrap(), state, self)?,
            "getId" => {
                return Ok(self.name(
                    manager
                        .get_identifier(action["id"].as_str().unwrap())?
                        .as_ref(),
                ));
            }
            "getClass" => {
                return Ok(self.name(
                    manager
                        .get_where(|t| class_match(t, action["class"].as_str().unwrap()))?
                        .as_ref(),
                ));
            }
            "removeClass" => manager.remove_where(
                |t| class_match(t, action["class"].as_str().unwrap()),
                state,
                self,
            )?,
            "process" => manager.process(state, self)?,
            "death" => manager.death(state, self)?,
            "clear" => manager.clear(),
            "tick" => self.tick = value,
            "step" => self.step = value,
            "modal" => self.modal = action["value"].as_bool().unwrap(),
            "delay" => self.delay = value,
            "setTimer" => {
                let timer = timer.ok_or(Error::Null)?;
                let mut timer = timer.write();
                match action["field"].as_str().unwrap() {
                    "interval" => timer.run_interval = value,
                    "next" => timer.next_execution = value,
                    "last" => timer.last_execution = value,
                    _ => unreachable!(),
                }
            }
            "rawAdd" => Self::list(manager, action["list"].as_str().unwrap())
                .write()
                .push(timer),
            "rawSet" => {
                Self::list(manager, action["list"].as_str().unwrap())
                    .write()
                    .set(value as usize, timer)?;
            }
            "rawRemove" => {
                Self::list(manager, action["list"].as_str().unwrap())
                    .write()
                    .remove(value as usize)?;
            }
            "rawClear" => Self::list(manager, action["list"].as_str().unwrap())
                .write()
                .clear(),
            "root" => self.root = Self::object(action["text"].as_str().unwrap())?,
            "save" | "baseSave" => {
                let mut root = std::mem::replace(&mut self.root, Json::object());
                let result = if action["op"] == "save" {
                    manager.save(&mut root, state, self)
                } else {
                    let timer = timer.ok_or(Error::Null)?;
                    if timer.read().persistent {
                        self.save(&timer, &mut root, manager, state)
                    } else {
                        Err(Error::Type)
                    }
                };
                self.root = root;
                result?;
            }
            "parse" => manager.parse(
                &Self::object(action["text"].as_str().unwrap())?,
                state,
                self,
            )?,
            "baseParse" => {
                let root = Self::object(action["text"].as_str().unwrap())?;
                let timer = timer.ok_or(Error::Null)?;
                if !timer.read().persistent {
                    return Err(Error::Type);
                }
                self.parse(&timer, &root, manager, state)?;
            }
            "fail" => return Err(Error::Binding("script".into())),
            _ => state.action(action)?,
        }
        Ok(Value::Null)
    }
    pub fn snapshot(&self, manager: &Manager, result: Value, error: Option<&str>) -> Value {
        let observe = |list: &TimerList| {
            list.read()
                .values()
                .iter()
                .map(|t| self.name(t.as_ref()))
                .collect::<Vec<_>>()
        };
        let timers = self
            .handles
            .iter()
            .map(|(name, t)| {
                let value = t.as_ref().map(|t| {
                    let t = t.read();
                    json!([
                        t.class_name,
                        t.identifier,
                        t.run_interval,
                        t.next_execution,
                        t.last_execution,
                        t.is_soft,
                        t.is_auto,
                        t.flags,
                        t.persistent
                    ])
                });
                (name.clone(), value)
            })
            .collect::<BTreeMap<_, _>>();
        let mut value = json!({"result":result,"error":error,"tick":self.tick,"active":observe(&manager.active),"pending":observe(&manager.pending),"removing":observe(&manager.removing),"timers":timers,"root":self.root.text(),"events":self.events});
        if self.request["registryNative"] == true {
            value["extra"] = json!({"map":self.registry.entries().iter().map(|(key,timer)|json!([key,self.name(timer.as_ref())])).collect::<Vec<_>>(),"auto":observe(&self.registry.automatic)});
        }
        value
    }
}
impl<S: ScriptState> Factories<S> for Harness {
    type Argument = Value;
    fn instantiate(
        &mut self,
        prototype: &Handle,
        args: Option<&[Value]>,
        _: &Registry,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<Option<Handle>> {
        let desc = self.configs[&prototype.identity()].clone();
        if matches!(
            prototype.read().class_name.as_str(),
            "FactoryTimer" | "FactoryChild" | "FactoryPersistent"
        ) {
            self.hook(
                prototype,
                if args.is_some() {
                    "factory"
                } else {
                    "retrieve"
                },
                manager,
                state,
            )?;
            self.events.push(json!(["arguments", args]));
            if let Some(target) = desc["factoryTarget"].as_str() {
                return Ok(Some(
                    self.handles
                        .get(target)
                        .cloned()
                        .flatten()
                        .ok_or(Error::Null)?,
                ));
            }
        }
        let label = format!("created{}", self.created);
        self.created += 1;
        let timer = self.construct(&desc, label, true);
        if desc["useArgs"] == true
            && let Some(args) = args
        {
            timer.write().run_interval = args[0]
                .as_i64()
                .map(|v| v as i32)
                .or_else(|| args[0].as_f64().map(|v| v as i32))
                .ok_or(Error::Type)?;
        }
        Ok(Some(timer))
    }
    fn registry_log(
        &mut self,
        level: Level,
        message: String,
        _: &Registry,
        _: &mut Manager,
        _: &mut S,
    ) -> Result<()> {
        self.record_log(level, message)
    }
}
impl<S: ScriptState> Context<S> for Harness {
    fn is_player(&self, _: &S) -> bool {
        self.player
    }
    fn modal_open(&mut self, _: &mut S) -> Result<bool> {
        self.events.push(json!(["modal", self.modal]));
        Ok(self.modal)
    }
    fn script_delay(&mut self, _: &mut S) -> Result<i32> {
        self.events.push(json!(["delay", self.delay]));
        Ok(self.delay)
    }
    fn ticks(&mut self, _: &mut S) -> Result<i32> {
        let tick = self.tick;
        self.events.push(json!(["tick", tick]));
        self.tick = self.tick.wrapping_add(self.step);
        Ok(tick)
    }
    fn entity_name(&mut self, _: &mut S) -> Result<Option<String>> {
        Ok(Some("entity".into()))
    }
    fn before_register(&mut self, t: &Handle, m: &mut Manager, s: &mut S) -> Result<()> {
        self.hook(t, "before", m, s)
    }
    fn on_register(&mut self, t: &Handle, m: &mut Manager, s: &mut S) -> Result<()> {
        self.hook(t, "register", m, s)
    }
    fn on_removal(&mut self, t: &Handle, m: &mut Manager, s: &mut S) -> Result<()> {
        self.hook(t, "remove", m, s)
    }
    fn run(&mut self, t: &Handle, m: &mut Manager, s: &mut S) -> Result<bool> {
        self.hook(t, "run", m, s)?;
        Ok(self.configs[&t.identity()]["result"] != false)
    }
    fn initial_delay(&mut self, t: &Handle, m: &mut Manager, s: &mut S) -> Result<i32> {
        self.hook(t, "initial", m, s)?;
        Ok(number(
            &self.configs[&t.identity()],
            "initial",
            t.read().run_interval,
        ))
    }
    fn save(&mut self, t: &Handle, root: &mut Json, m: &mut Manager, s: &mut S) -> Result<()> {
        self.hook(t, "save", m, s)?;
        timers::save_base(t, root, s, self)
    }
    fn parse(&mut self, t: &Handle, root: &Json, m: &mut Manager, s: &mut S) -> Result<()> {
        self.hook(t, "parse", m, s)?;
        timers::parse_base(t, root)
    }
    fn resolve(
        &mut self,
        id: &str,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<Option<Handle>> {
        if self.request["registryNative"] == true {
            return self
                .registry
                .clone()
                .instance(id, &[], manager, state, self);
        }
        self.events.push(json!(["resolve", id]));
        let Some(name) = self.request["registry"][id].as_str() else {
            return Ok(None);
        };
        let Some(timer) = self.handles.get(name).cloned().flatten() else {
            return Ok(None);
        };
        let desc = self.configs[&timer.identity()].clone();
        let label = format!("created{}", self.created);
        self.created += 1;
        Ok(Some(self.construct(&desc, label, true)))
    }
    fn run_failure(&mut self, class: &str, name: Option<&str>, _: &mut S) -> Result<()> {
        if self.request["registryNative"] == true {
            return self.record_log(Level::Error,format!("Prematurely removing timer {class} from {} because it threw an exception when ran. Exception follows:",name.unwrap_or("null")));
        }
        self.events.push(json!(["log",format!("Prematurely removing timer {class} from {} because it threw an exception when ran. Exception follows:",name.unwrap_or("null"))]));
        Ok(())
    }
    fn unknown_timer(&mut self, id: &str, _: &mut S) -> Result<()> {
        if self.request["registryNative"] == true {
            return self.record_log(Level::Error,format!("Tried to load data for persistent timer identified by {id}, but no such timer seems to exist."));
        }
        self.events.push(json!(["log",format!("Tried to load data for persistent timer identified by {id}, but no such timer seems to exist.")]));
        Ok(())
    }
    fn exception(&mut self, error: &Error, _: &mut S) -> Result<()> {
        self.events.push(json!(["exception", error_name(error)]));
        Ok(())
    }
}
impl Harness {
    pub fn new(request: &Value) -> (Self, Manager) {
        let mut harness = Harness {
            request: request.clone(),
            tick: number(request, "tick", 0),
            step: number(request, "step", 0),
            player: request["player"] != false,
            modal: request["modal"] == true,
            delay: number(request, "delay", 0),
            handles: BTreeMap::new(),
            names: BTreeMap::new(),
            configs: BTreeMap::new(),
            counts: BTreeMap::new(),
            created: 0,
            events: Vec::new(),
            root: Json::object(),
            registry: Registry::default(),
        };
        for (name, desc) in request["timers"].as_object().unwrap() {
            if desc.is_null() {
                harness.handles.insert(name.clone(), None);
            } else {
                harness.construct(desc, name.clone(), false);
            }
        }
        let manager = Manager::default();
        for name in ["active", "pending", "removing"] {
            for label in request[name].as_array().into_iter().flatten() {
                let timer = label
                    .as_str()
                    .and_then(|label| harness.handles.get(label))
                    .cloned()
                    .flatten();
                Harness::list(&manager, name).write().push(timer);
            }
        }
        (harness, manager)
    }
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}
pub fn evaluate(request: &Value) -> Value {
    let (mut harness, mut manager) = Harness::new(request);
    let mut states = Vec::new();
    for action in request["actions"].as_array().unwrap() {
        harness.clear_events();
        let (value, error) = match harness.action(&mut manager, &mut (), action) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(error_name(&e))),
        };
        states.push(harness.snapshot(&manager, value, error));
    }
    json!(states)
}
