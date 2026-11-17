use super::plain;
use rs09_server::{
    definitions::item::Shared,
    world::{
        component_objects::{NativeObject, Plugin},
        doors::Location,
        interface_listeners::Button,
        interface_packets::{self as packets, Action, LoginPulse},
        interfaces::{self, Error, Result},
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value as J, json};
use std::collections::{BTreeMap, BTreeSet};
fn n(v: &J, k: &str, d: usize) -> usize {
    v[k].as_u64().map_or(d, |n| n as usize)
}
fn text(v: &J) -> Option<Vec<u16>> {
    if v.is_null() {
        None
    } else if let Some(s) = v.as_str() {
        Some(s.encode_utf16().collect())
    } else {
        Some(
            v.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect(),
        )
    }
}
struct Function(usize);
impl NativeObject for Function {}
#[derive(Clone)]
struct Player {
    dispatch: Option<usize>,
    manager: Option<usize>,
    interpreter: Option<usize>,
}
#[derive(Clone)]
struct Manager {
    chat: Option<usize>,
    lookup: BTreeMap<i32, usize>,
    veto: bool,
}
#[derive(Clone)]
struct Component {
    id: i32,
    plugin: Option<usize>,
}
struct Model {
    source: J,
    events: Vec<J>,
    calls: BTreeMap<String, usize>,
    hooks: BTreeSet<usize>,
    nodes: Vec<NodeHandle>,
    _state: State,
    players: Vec<Player>,
    managers: Vec<Manager>,
    components: Vec<Component>,
    active: Vec<J>,
    queues: Vec<Vec<Option<usize>>>,
    listener: bool,
    current_lines: Option<plain::Lines>,
    plugins: Vec<Plugin>,
    interop: Vec<interfaces::Handle>,
}
impl Model {
    fn p(&self, p: &NodeHandle) -> usize {
        self.nodes.iter().position(|q| q == p).unwrap()
    }
    fn emit(&mut self, name: &str, args: Vec<J>) -> Result<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(name.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.source["hooks"].as_array().cloned().unwrap_or_default();
        for (i, h) in hooks.iter().enumerate() {
            if h["event"] == name && n(h, "at", 1) == count && self.hooks.insert(i) {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.source["failures"][name].as_u64() == Some(count as u64) {
            return Err(Error::Binding("recording failure".into()));
        }
        Ok(())
    }
    fn get(&mut self, p: usize, field: &str) -> Result<Option<usize>> {
        self.emit(field, vec![json!(p)])?;
        Ok(match field {
            "dispatch" => self.players[p].dispatch,
            "manager" => self.players[p].manager,
            "interpreter" => self.players[p].interpreter,
            _ => unreachable!(),
        })
    }
    fn component(&mut self, id: i32) -> usize {
        let c = self.components.len();
        self.components.push(Component { id, plugin: None });
        self.interop.push(Shared::new(interfaces::Component {
            id,
            definition: None,
            plugin: None,
            close_event: None,
            hidden: false,
            weapon: false,
        }));
        c
    }
    fn act(&mut self, a: &J) -> Result<J> {
        let p = n(a, "player", 0);
        match a["op"].as_str().unwrap() {
            "send" => {
                let t = text(&a["text"]).ok_or(Error::Null)?;
                let node = self.nodes[p].clone();
                plain::send(self, &node, &t)?;
            }
            "lines" => {
                self.current_lines = a["lines"]
                    .as_array()
                    .map(|a| plain::Lines::new(a.iter().map(text).collect()));
                let lines = self.current_lines.clone();
                let c = plain::send_lines(self, Some(n(a, "interpreter", 0)), lines.as_ref())?;
                return Ok(json!(c));
            }
            "option" => {
                let node = self.nodes[p].clone();
                packets::continue_option(
                    self,
                    &node,
                    Action {
                        iface: n(a, "iface", 210) as i32,
                        child: n(a, "child", 1) as i32,
                        slot: a["slot"].as_i64().unwrap_or(-1) as i32,
                        opcode: n(a, "opcode", 132) as i32,
                        opt_index: 0,
                        item: -1,
                    },
                )?;
            }
            "field" => {
                let value = &a["value"];
                let id = value.as_u64().unwrap_or(0) as usize;
                let optional = (!value.is_null()).then_some(id);
                match a["field"].as_str().unwrap() {
                    "dispatch" => self.players[p].dispatch = optional,
                    "manager" => self.players[p].manager = optional,
                    "interpreter" => self.players[p].interpreter = optional,
                    "chatbox" => self.managers[n(a, "manager", 0)].chat = optional,
                    "id" => {
                        self.components[n(a, "component", 1)].id = value.as_i64().unwrap() as i32
                    }
                    "plugin" => self.components[n(a, "component", 1)].plugin = optional,
                    "active" => self.active[n(a, "interpreter", 0)] = value.clone(),
                    "listener" => self.listener = value.as_bool().unwrap(),
                    "veto" => self.managers[n(a, "manager", 0)].veto = value.as_bool().unwrap(),
                    "lookup" => {
                        let m = &mut self.managers[n(a, "manager", 0)];
                        let iface = n(a, "iface", 210) as i32;
                        if let Some(c) = optional {
                            m.lookup.insert(iface, c);
                        } else {
                            m.lookup.remove(&iface);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            "queue" => {
                self.queues[n(a, "interpreter", 0)] = a["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().map(|v| v as usize))
                    .collect()
            }
            "line" => {
                self.current_lines.as_ref().ok_or(Error::Null)?.write()[n(a, "index", 0)] =
                    text(&a["value"]);
            }
            "replaceArray" => {
                self.current_lines = Some(plain::Lines::new(
                    a["values"].as_array().unwrap().iter().map(text).collect(),
                ));
            }
            _ => unreachable!(),
        }
        Ok(J::Null)
    }
    fn snapshot(&self, result: J, error: Option<&str>) -> J {
        json!({"error":error,"result":result,"events":self.events,"players":self.players.iter().map(|p|json!([p.dispatch,p.manager,p.interpreter])).collect::<Vec<_>>(),"chatboxes":self.managers.iter().map(|m|m.chat).collect::<Vec<_>>(),"components":self.components.iter().map(|c|json!([c.id,c.plugin])).collect::<Vec<_>>(),"queues":self.queues})
    }
    fn interp(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        let p = self.p(p);
        self.get(p, "interpreter")
    }
    fn manager_for(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        let p = self.p(p);
        self.get(p, "manager")
    }
    fn dispatch_for(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        let p = self.p(p);
        self.get(p, "dispatch")
    }
    fn chatbox(&mut self, m: Option<usize>) -> Result<Option<usize>> {
        let m = m.ok_or(Error::Null)?;
        self.emit("chatbox", vec![json!(m)])?;
        Ok(self.managers[m].chat)
    }
}
impl plain::Context for Model {
    type Interpreter = Option<usize>;
    type Dispatch = Option<usize>;
    type Manager = Option<usize>;
    type Component = usize;
    fn receiver(&mut self, d: &Option<usize>) -> Result<()> {
        d.ok_or(Error::Null).map(|_| ())
    }
    fn interpreter(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.interp(p)
    }
    fn player(&mut self, d: &Option<usize>) -> Result<NodeHandle> {
        let id = d.ok_or(Error::Null)?;
        self.nodes.get(id).cloned().ok_or(Error::Null)
    }
    fn dispatch(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.dispatch_for(p)
    }
    fn text(
        &mut self,
        d: Option<usize>,
        text: Option<Vec<u16>>,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "text",
            vec![json!(d), json!(text), json!(iface), json!(child)],
        )
    }
    fn manager(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.manager_for(p)
    }
    fn open_chatbox(&mut self, m: Option<usize>, id: i32) -> Result<()> {
        let m = m.ok_or(Error::Null)?;
        self.emit("open", vec![json!(m), json!(id)])?;
        let c = self.component(id);
        self.managers[m].chat = Some(c);
        Ok(())
    }
    fn chatbox(&mut self, m: Option<usize>) -> Result<Option<usize>> {
        Model::chatbox(self, m)
    }
    fn id(&mut self, c: usize) -> Result<i32> {
        self.emit("id", vec![json!(c)])?;
        Ok(self.components[c].id)
    }
    fn hidden(&mut self, d: Option<usize>, id: i32, child: i32, hidden: bool) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "hidden",
            vec![json!(d), json!(id), json!(child), json!(hidden)],
        )
    }
}
fn unused<T>() -> Result<T> {
    Err(Error::Binding("unused recording API".into()))
}
impl packets::Context for Model {
    type Dialogue = Option<usize>;
    type Locks = ();
    type Zone = ();
    type Manager = Option<usize>;
    type Dispatch = Option<usize>;
    fn debug(&mut self, p: &NodeHandle, text: String) -> Result<()> {
        self.emit("debug", vec![json!(self.p(p)), json!(text)])
    }
    fn dialogue(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.interp(p)
    }
    fn has_dialogue(&mut self, d: Option<usize>) -> Result<bool> {
        let d = d.ok_or(Error::Null)?;
        self.emit("dialogue", vec![json!(d)])?;
        Ok(!self.active[d].is_null())
    }
    fn close_dialogue(&mut self, _: Option<usize>) -> Result<()> {
        unused()
    }
    fn locks(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
    fn component_locked(&mut self, _: ()) -> Result<bool> {
        unused()
    }
    fn zone(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
    fn click_zone(&mut self, _: (), _: Action) -> Result<bool> {
        unused()
    }
    fn manager(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.manager_for(p)
    }
    fn component(&mut self, m: Option<usize>, id: i32) -> Result<Option<interfaces::Handle>> {
        let m = m.ok_or(Error::Null)?;
        self.emit("component", vec![json!(m), json!(id)])?;
        let c = self.managers[m]
            .chat
            .filter(|i| self.components[*i].id == id)
            .or_else(|| self.managers[m].lookup.get(&id).copied());
        Ok(c.map(|c| self.interop[c].clone()))
    }
    fn hidden(&mut self, _: &interfaces::Handle) -> Result<bool> {
        unused()
    }
    fn plugin(&mut self, c: &interfaces::Handle) -> Result<Option<Plugin>> {
        let c = self.interop.iter().position(|v| v.same(c)).unwrap();
        self.emit("plugin", vec![json!(c)])?;
        Ok(self.components[c].plugin.map(|p| self.plugins[p].clone()))
    }
    fn id(&mut self, c: &interfaces::Handle) -> Result<i32> {
        let c = self.interop.iter().position(|v| v.same(c)).unwrap();
        self.emit("id", vec![json!(c)])?;
        Ok(self.components[c].id)
    }
    fn button_event(&mut self, _: &NodeHandle, _: i32, _: i32) -> Result<()> {
        unused()
    }
    fn run_listener(&mut self, p: &NodeHandle, c: &interfaces::Handle, b: Button) -> Result<bool> {
        let c = self.interop.iter().position(|v| v.same(c)).unwrap();
        self.emit(
            "listener",
            vec![
                json!(self.p(p)),
                json!(c),
                json!(b.opcode),
                json!(b.button),
                json!(b.slot),
                json!(b.item),
            ],
        )?;
        Ok(self.listener)
    }
    fn handle_plugin(
        &mut self,
        plugin: Plugin,
        p: &NodeHandle,
        c: &interfaces::Handle,
        b: Button,
    ) -> Result<bool> {
        let c = self.interop.iter().position(|v| v.same(c)).unwrap();
        self.emit(
            "pluginHandle",
            vec![
                json!(plugin.value::<Function>().unwrap().0),
                json!(self.p(p)),
                json!(c),
                json!(b.opcode),
                json!(b.button),
                json!(b.slot),
                json!(b.item),
            ],
        )?;
        Ok(false)
    }
    fn close(&mut self, _: Option<usize>) -> Result<bool> {
        unused()
    }
    fn logging_present(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn submit_login(&mut self, _: LoginPulse) -> Result<()> {
        unused()
    }
    fn map_present(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn remove_map(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
    fn dispatch(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.dispatch_for(p)
    }
    fn resizable(&mut self, _: Option<usize>) -> Result<bool> {
        unused()
    }
    fn windows_pane(&mut self, _: Option<usize>, _: i32, _: i32) -> Result<()> {
        unused()
    }
    fn unlock(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
    fn remove_logging(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
    fn configure_world(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
}
impl packets::ContinueContext for Model {
    type DialogueAction = usize;
    fn close_chatbox(&mut self, m: Option<usize>) -> Result<()> {
        let m = m.ok_or(Error::Null)?;
        self.emit("closeChatbox", vec![json!(m)])?;
        if !self.managers[m].veto {
            self.managers[m].chat = Some(0);
        }
        Ok(())
    }
    fn pop_action(&mut self, d: Option<usize>) -> Result<Option<usize>> {
        let d = d.ok_or(Error::Null)?;
        self.emit("actions", vec![json!(d)])?;
        Ok(if self.queues[d].is_empty() {
            None
        } else {
            self.queues[d].remove(0)
        })
    }
    fn handle_action(&mut self, id: usize, p: &NodeHandle, child: i32) -> Result<()> {
        self.emit("action", vec![json!(id), json!(self.p(p)), json!(child)])
    }
    fn continue_dialogue(&mut self, d: Option<usize>, iface: i32, child: i32) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit("activeHandle", vec![json!(d), json!(iface), json!(child)])
    }
}
fn error(e: Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Binding(_) => "Binding",
        _ => "other",
    }
}
pub fn evaluate(row: &J) -> J {
    if let Some(split) = row.get("split") {
        let result = text(&split["text"])
            .ok_or(Error::Null)
            .and_then(|t| plain::split_lines(&t, n(split, "limit", 54) as i32));
        return match result {
            Ok(lines) => json!({"error":null,"lines":lines}),
            Err(e) => json!({"error":error(e),"lines":null}),
        };
    }
    let mut state = State::default();
    let mut nodes = vec![];
    for i in 0..3 {
        let id = state.add(i, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
        nodes.push(state.retain_node(id).unwrap());
    }
    let mut model = Model {
        source: row.clone(),
        events: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        nodes,
        _state: state,
        players: (0..3)
            .map(|i| Player {
                dispatch: Some(i),
                manager: Some(i),
                interpreter: Some(i),
            })
            .collect(),
        managers: vec![
            Manager {
                chat: Some(0),
                lookup: BTreeMap::new(),
                veto: false
            };
            3
        ],
        components: vec![],
        active: vec![J::Null; 4],
        queues: vec![vec![]; 4],
        listener: false,
        current_lines: None,
        plugins: vec![Plugin::new(Function(0)), Plugin::new(Function(1))],
        interop: vec![],
    };
    for id in [137, 210, 211, 212, 213, 762, 763] {
        model.component(id);
    }
    let mut out = vec![];
    for a in row["actions"].as_array().unwrap() {
        let (result, error) = match model.act(a) {
            Ok(r) => (r, None),
            Err(e) => (J::Null, Some(error(e))),
        };
        out.push(model.snapshot(result, error));
    }
    json!(out)
}
