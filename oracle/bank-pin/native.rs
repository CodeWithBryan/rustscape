//! Recording services only. All PIN state/control flow uses production Rust.
use rs09_game_core::{java_json, random::JavaRandom};
use rs09_server::{
    definitions::item::Shared,
    player::bank_pin::{
        self as pin, CloseEvent, Context, Error, InterfaceContext, Manager, Program, Status,
    },
    world::{
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map(|n| n as i32).unwrap_or(d)
}
fn l(v: &Value, k: &str, d: i64) -> i64 {
    v[k].as_i64().unwrap_or(d)
}
fn t(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").into()
}
#[derive(Clone)]
struct Component(Shared<ComponentFields>);
struct ComponentFields {
    id: i32,
    event: Option<CloseEvent>,
}
struct Host {
    row: Value,
    nodes: Vec<NodeHandle>,
    managers: Vec<Manager>,
    player_managers: [Option<Manager>; 2],
    current: [Option<Component>; 2],
    events: Vec<Component>,
    plugins: BTreeMap<i32, Program>,
    program: Program,
    log: Vec<Value>,
    counts: HashMap<String, i32>,
    clock: i64,
    clock_calls: i64,
    random_calls: usize,
    shuffle: JavaRandom,
}
fn set(manager: &Manager, key: &str, value: &Value) {
    let fields = manager.fields();
    let mut f = fields.write();
    match key {
        "pin" => f.pin = value.as_str().map(pin::text),
        "tempPin" => f.temp_pin = value.as_str().map(pin::text),
        "status" => {
            f.status = if value.is_null() {
                None
            } else {
                Some(match value.as_i64().unwrap() {
                    0 => Status::NoPin,
                    1 => Status::Pending,
                    2 => Status::Active,
                    _ => panic!("bad status injection"),
                })
            }
        }
        "unlocked" => f.unlocked = value.as_bool().unwrap(),
        "longRecovery" => f.long_recovery = value.as_bool().unwrap(),
        "deleting" => f.deleting = value.as_bool().unwrap(),
        "pendingDelay" => f.pending_delay = value.as_i64().unwrap(),
        "tryDelay" => f.try_delay = value.as_i64().unwrap(),
        "openId" => f.open_id = value.as_i64().unwrap() as i32,
        "tries" => f.tries = value.as_i64().unwrap() as i32,
        "changeState" => f.change_state = value.as_i64().unwrap() as i32,
        key => panic!("unknown private input field {key}"),
    }
}
impl Host {
    fn mid(&self, m: Option<&Manager>) -> i32 {
        m.map(|m| {
            self.managers
                .iter()
                .position(|other| other.same(m))
                .expect("manager missing from recording roots") as i32
        })
        .unwrap_or(-1)
    }
    fn pid(&self, p: Option<&NodeHandle>) -> pin::Result<usize> {
        p.and_then(|p| self.nodes.iter().position(|n| n == p))
            .ok_or(Error::Null)
    }
    fn nil(&self, key: &str) -> bool {
        self.row["null"] == key
    }
    fn hit(&mut self, label: &str, args: Vec<Value>) -> pin::Result<()> {
        let mut event = vec![json!(label)];
        event.extend(args);
        self.log.push(json!(event));
        let count = self.counts.entry(label.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.row["hooks"].as_array().unwrap().clone();
        for hook in hooks {
            if hook["when"] != label || n(&hook, "at", 1) != count {
                continue;
            }
            let manager = self.managers[n(&hook, "target", 0) as usize].clone();
            match t(&hook, "action").as_str() {
                "fail" => return Err(Error::Binding("controlled".into())),
                "set" => set(&manager, &t(&hook, "field"), &hook["value"]),
                "cancel" => manager.do_cancel(),
                "closeEvent" => {
                    let component = self.events[n(&hook, "event", 0) as usize].clone();
                    component.0.read().event.as_ref().unwrap().close();
                }
                "replaceManager" => {
                    let p = n(&hook, "player", 0) as usize;
                    let manager = Manager::new(Some(self.nodes[p].clone()));
                    self.managers.push(manager.clone());
                    self.player_managers[p] = Some(manager);
                }
                "nullManager" => self.player_managers[n(&hook, "player", 0) as usize] = None,
                "reenter" => manager.open_type(self, n(&hook, "button", 4))?,
                action => panic!("unexpected hook action {action}"),
            }
        }
        Ok(())
    }
    fn state(&self) -> Value {
        let managers=self.managers.iter().map(|m|{let p=m.player().map(|p|self.pid(Some(&p)).unwrap());let fields=m.fields();let f=fields.read();json!({"player":p,"digits":f.digits,"pin":f.pin,"tempPin":f.temp_pin,"status":f.status.map(|s|s as i32),"unlocked":f.unlocked,"longRecovery":f.long_recovery,"pendingDelay":f.pending_delay,"openId":f.open_id,"tries":f.tries,"changeState":f.change_state,"deleting":f.deleting,"tryDelay":f.try_delay})}).collect::<Vec<_>>();
        let events = self
            .events
            .iter()
            .map(|c| {
                let f = c.0.read();
                json!([f.id, self.mid(Some(&f.event.as_ref().unwrap().manager()))])
            })
            .collect::<Vec<_>>();
        let current = self
            .current
            .iter()
            .map(|c| c.as_ref().map(|c| c.0.read().id))
            .collect::<Vec<_>>();
        let plugins = self
            .plugins
            .iter()
            .map(|(id, p)| (id.to_string(), if p.same(&self.program) { 0 } else { 1 }))
            .collect::<BTreeMap<_, _>>();
        json!({"managers":managers,"playerManagers":self.player_managers.iter().map(|m|self.mid(m.as_ref())).collect::<Vec<_>>(),"events":events,"current":current,"plugins":plugins,"clockCalls":self.clock_calls,"randomCalls":self.random_calls})
    }
    fn messages(a: &Value) -> Option<Vec<Option<Vec<u16>>>> {
        a["messages"]
            .as_array()
            .map(|v| v.iter().map(|v| v.as_str().map(pin::text)).collect())
    }
    fn action(&mut self, a: &Value) -> pin::Result<Option<Value>> {
        let manager = self.managers[n(a, "target", 0) as usize].clone();
        let value = n(a, "value", 0);
        match t(a, "op").as_str() {
            "set" => set(&manager, &t(a, "field"), &a["value"]),
            "parse" => {
                let data = java_json::parse(&a["data"].to_string()).unwrap();
                manager.parse(Some(&data))?;
            }
            "settings" => manager.open_settings(self)?,
            "messages" => {
                let messages = Self::messages(a);
                manager.open_settings_messages(self, messages.as_deref())?;
            }
            "type" => manager.open_type(self, value)?,
            "confirm" => manager.confirm(self, value)?,
            "toggle" => manager.toggle_confirm(self, a["value"] == true)?,
            "pin" => manager.open_pin(self)?,
            "stage" => manager.handle_stage(self, value)?,
            "click" => manager.update_temp(self, value)?,
            "digit" => {
                let index = manager
                    .fields()
                    .read()
                    .digits
                    .iter()
                    .position(|d| *d == value)
                    .map(|n| n as i32)
                    .unwrap_or(-1);
                manager.update_temp(self, index)?;
            }
            "cancel" => {
                let messages = Self::messages(a);
                manager.cancel_messages(self, messages.as_deref())?;
            }
            "doCancel" => manager.do_cancel(),
            "switch" => manager.switch_recovery(self)?,
            "login" => manager.draw_login(self)?,
            "days" => return manager.pending_days(self).map(|v| Some(json!(v))),
            "blocked" => return manager.blocked(self).map(|v| Some(json!(v))),
            "getters" => {
                let stage = manager.stage();
                let recovery = manager.recovery_delay();
                let has = manager.has_pin();
                let pending = manager.has_pending_pin(self)?;
                let unlocked = manager.is_unlocked();
                let fields = manager.fields();
                let f = fields.read();
                return Ok(Some(json!([
                    stage,
                    recovery,
                    has,
                    pending,
                    unlocked,
                    f.long_recovery,
                    manager.pin_seconds(),
                    f.tries,
                    f.try_delay,
                    f.pending_delay
                ])));
            }
            "statusMessages" => {
                let status = manager.fields().read().status;
                return manager.messages(self, status).map(|v| Some(json!(v)));
            }
            "statusLogin" => {
                let status = manager.fields().read().status;
                return manager.login_message(self, status).map(|v| Some(json!(v)));
            }
            "statusDraw" => {
                let status = manager.fields().read().status;
                manager.draw_status(self, status)?;
            }
            "closeEvent" => {
                let c = self.events[value as usize].clone();
                return Ok(Some(json!(c.0.read().event.as_ref().unwrap().close())));
            }
            "replaceManager" => {
                let p = n(a, "player", 0) as usize;
                let m = Manager::new(Some(self.nodes[p].clone()));
                self.managers.push(m.clone());
                self.player_managers[p] = Some(m);
            }
            "handle" => {
                let p = n(a, "player", 0);
                let player = (p >= 0).then(|| self.nodes[p as usize].clone());
                let component = if a["nullComponent"] == true {
                    None
                } else {
                    Some(self.component(n(a, "component", 14))?)
                };
                let program = self.program.clone();
                return program
                    .handle(self, player.as_ref(), &component, value)
                    .map(|v| Some(json!(v)));
            }
            "register" => {
                let program = self.program.clone();
                let registered = program.register(self)?;
                return Ok(Some(json!(program.same(&registered))));
            }
            op => panic!("unexpected operation {op}"),
        }
        Ok(None)
    }
}
impl Context for Host {
    type Interfaces = Option<usize>;
    type Dispatch = Option<usize>;
    type Component = Component;
    type Dialogue = Option<usize>;
    type Bank = Option<usize>;
    type Exchange = Option<usize>;
    type Settings = bool;
    fn now(&mut self) -> pin::Result<i64> {
        let times = self.row["times"].as_array().unwrap();
        let value = if times.is_empty() {
            self.clock
        } else {
            times[(self.clock_calls as usize).min(times.len() - 1)]
                .as_i64()
                .unwrap()
        };
        self.clock_calls = self.clock_calls.wrapping_add(1);
        self.clock = self.clock.wrapping_add(l(&self.row, "clockStep", 0));
        self.hit("clock", vec![json!(value)])?;
        Ok(value)
    }
    fn shuffle(&mut self, digits: &mut [i32]) -> pin::Result<()> {
        pin::shuffle(digits, &mut self.shuffle)
    }
    fn random(&mut self, min: i32, max: i32) -> pin::Result<i32> {
        self.hit("random", vec![json!(min), json!(max)])?;
        let values = self.row["randoms"].as_array().unwrap();
        if values.is_empty() {
            return Ok(min);
        }
        let value = values[self.random_calls % values.len()].as_i64().unwrap() as i32;
        self.random_calls += 1;
        Ok(value)
    }
    fn interfaces(&mut self, p: Option<&NodeHandle>) -> pin::Result<Option<usize>> {
        let id = self.pid(p)?;
        self.hit("interfaces", vec![json!(id)])?;
        Ok((!self.nil("interfaces")).then_some(id))
    }
    fn close(&mut self, m: Option<usize>) -> pin::Result<()> {
        let id = m.ok_or(Error::Null)?;
        self.hit("close", vec![json!(id)])?;
        if self.row["invokeClose"] == true {
            let component = self.current[id].clone();
            if let Some(component) = component {
                let event = component.0.read().event.clone();
                if let Some(event) = event {
                    event.close();
                }
            }
        }
        Ok(())
    }
    fn component(&mut self, id: i32) -> pin::Result<Component> {
        self.hit("component", vec![json!(id)])?;
        Ok(Component(Shared::new(ComponentFields { id, event: None })))
    }
    fn close_event(&mut self, c: Component, event: CloseEvent) -> pin::Result<Component> {
        let id = c.0.read().id;
        self.hit(
            "closeEvent",
            vec![json!(id), json!(self.mid(Some(&event.manager())))],
        )?;
        c.0.write().event = Some(event);
        self.events.push(c.clone());
        Ok(c)
    }
    fn open(&mut self, m: Option<usize>, c: Component) -> pin::Result<()> {
        let id = m.ok_or(Error::Null)?;
        let component = c.0.read().id;
        self.hit("open", vec![json!(id), json!(component)])?;
        self.current[id] = Some(c);
        Ok(())
    }
    fn dispatch(&mut self, p: Option<&NodeHandle>) -> pin::Result<Option<usize>> {
        let id = self.pid(p)?;
        self.hit("dispatch", vec![json!(id)])?;
        Ok((!self.nil("dispatch")).then_some(id))
    }
    fn dispatch_string(
        &mut self,
        d: Option<usize>,
        value: &[u16],
        interface: i32,
        child: i32,
    ) -> pin::Result<()> {
        self.hit(
            "string",
            vec![
                json!(d.ok_or(Error::Null)?),
                json!(value),
                json!(interface),
                json!(child),
            ],
        )
    }
    fn dispatch_config(
        &mut self,
        d: Option<usize>,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> pin::Result<()> {
        self.hit(
            "config",
            vec![
                json!(d.ok_or(Error::Null)?),
                json!(interface),
                json!(child),
                json!(hidden),
            ],
        )
    }
    fn dispatch_message(&mut self, d: Option<usize>, value: &[u16]) -> pin::Result<()> {
        self.hit(
            "packetMessage",
            vec![json!(d.ok_or(Error::Null)?), json!(value)],
        )
    }
    fn string_packet(
        &mut self,
        p: Option<&NodeHandle>,
        value: &[u16],
        interface: i32,
        child: i32,
    ) -> pin::Result<()> {
        let id = self.pid(p)?;
        self.hit(
            "packetString",
            vec![json!(id), json!(value), json!(interface), json!(child)],
        )
    }
    fn reposition(
        &mut self,
        p: Option<&NodeHandle>,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> pin::Result<()> {
        let id = self.pid(p)?;
        self.hit(
            "position",
            vec![
                json!(id),
                json!(interface),
                json!(child),
                json!(x),
                json!(y),
            ],
        )
    }
    fn audio(&mut self, p: Option<&NodeHandle>, id: i32) -> pin::Result<()> {
        let p = self.pid(p)?;
        self.hit("audio", vec![json!(p), json!(id)])
    }
    fn message(&mut self, p: Option<&NodeHandle>, value: &[u16]) -> pin::Result<()> {
        let id = self.pid(p)?;
        self.hit("message", vec![json!(id), json!(value)])
    }
    fn dialogue(&mut self, p: Option<&NodeHandle>) -> pin::Result<Option<usize>> {
        let id = self.pid(p)?;
        self.hit("dialogue", vec![json!(id)])?;
        Ok((!self.nil("dialogue")).then_some(id))
    }
    fn send_dialogue(&mut self, d: Option<usize>, value: &[u16]) -> pin::Result<()> {
        self.hit(
            "sendDialogue",
            vec![json!(d.ok_or(Error::Null)?), json!(value)],
        )
    }
    fn bank(&mut self, p: Option<&NodeHandle>) -> pin::Result<Option<usize>> {
        let id = self.pid(p)?;
        self.hit("bank", vec![json!(id)])?;
        Ok((!self.nil("bank")).then_some(id))
    }
    fn open_bank(&mut self, b: Option<usize>) -> pin::Result<()> {
        self.hit("openBank", vec![json!(b.ok_or(Error::Null)?)])
    }
    fn exchange(&mut self, p: Option<&NodeHandle>) -> pin::Result<Option<usize>> {
        let id = self.pid(p)?;
        self.hit("exchange", vec![json!(id)])?;
        Ok((!self.nil("exchange")).then_some(id))
    }
    fn collection(&mut self, e: Option<usize>) -> pin::Result<()> {
        self.hit("collection", vec![json!(e.ok_or(Error::Null)?)])
    }
    fn market(&mut self, p: Option<&NodeHandle>) -> pin::Result<()> {
        let id = self.pid(p)?;
        self.hit("market", vec![json!(id)])
    }
    fn settings(&mut self) -> pin::Result<bool> {
        self.hit("settings", vec![])?;
        Ok(!self.nil("settings"))
    }
    fn world_name(&mut self, s: bool) -> pin::Result<Option<Vec<u16>>> {
        if !s {
            return Err(Error::Null);
        }
        self.hit("name", vec![])?;
        Ok((!self.nil("name")).then(|| pin::text("2009scape")))
    }
    fn dev_mode(&mut self, s: bool) -> pin::Result<bool> {
        if !s {
            return Err(Error::Null);
        }
        self.hit("dev", vec![])?;
        Ok(self.row["dev"] == true)
    }
    fn varp(&mut self, p: Option<&NodeHandle>, id: i32, value: i32) -> pin::Result<()> {
        let p = self.pid(p)?;
        self.hit("varp", vec![json!(p), json!(id), json!(value)])
    }
}
impl InterfaceContext for Host {
    type InputComponent = Option<Component>;
    type Definition = i32;
    fn player_manager(&mut self, p: Option<&NodeHandle>) -> pin::Result<Option<Manager>> {
        let id = self.pid(p)?;
        self.hit("pinManager", vec![json!(id)])?;
        Ok(self.player_managers[id].clone())
    }
    fn component_id(&mut self, c: &Option<Component>) -> pin::Result<i32> {
        let id = c.as_ref().ok_or(Error::Null)?.0.read().id;
        self.hit("componentId", vec![json!(id)])?;
        Ok(id)
    }
    fn definition(&mut self, id: i32) -> pin::Result<i32> {
        self.hit("definition", vec![json!(id)])?;
        Ok(id)
    }
    fn set_plugin(&mut self, d: i32, p: Program) -> pin::Result<()> {
        self.hit(
            "plugin",
            vec![json!(d), json!(if p.same(&self.program) { 0 } else { 1 })],
        )?;
        self.plugins.insert(d, p);
        Ok(())
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut state = State::default();
    let mut nodes = vec![];
    for id in 0..2 {
        let node = state.add(id, Location::new(0, 0, 0), [0; 4]);
        nodes.push(state.retain_node(node).unwrap());
    }
    let managers = vec![
        Manager::new((row["nullOwner"] != true).then(|| nodes[0].clone())),
        Manager::new(Some(nodes[1].clone())),
    ];
    for (k, v) in row["initial"].as_object().unwrap() {
        set(&managers[0], k, v);
    }
    let mut host = Host {
        row: row.clone(),
        nodes,
        player_managers: [Some(managers[0].clone()), Some(managers[1].clone())],
        managers,
        current: [None, None],
        events: vec![],
        plugins: BTreeMap::new(),
        program: Program::default(),
        log: vec![],
        counts: HashMap::new(),
        clock: l(row, "clock", 1000000),
        clock_calls: 0,
        random_calls: 0,
        shuffle: JavaRandom::new(l(row, "seed", 1)),
    };
    let mut out = vec![];
    for action in row["actions"].as_array().unwrap() {
        let (result, error) = match host.action(action) {
            Ok(v) => (v.unwrap_or(json!("__NULL__")), json!("__NULL__")),
            Err(error) => (
                json!("__NULL__"),
                json!(match error {
                    Error::Null => "Null",
                    Error::Type => "Type",
                    Error::Number => "Number",
                    Error::Index => "Index",
                    Error::Binding(_) => "Binding",
                }),
            ),
        };
        out.push(json!({"result":result,"error":error,"state":host.state(),"log":host.log}));
    }
    json!(out)
}
