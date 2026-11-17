//! Explicit component content/player/plugin/packet services for the JVM corpus.
use rs09_protocol::game::{Frame, updates};
use rs09_server::world::component_objects::{Close, NativeObject, Plugin};
use rs09_server::{
    definitions::item::Shared,
    world::interfaces::{
        self as ui, Context, Def, Error, Handle, LoginContext, Manager, Open, Registry, Result,
        Type,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
struct ObservedPlugin(usize);
struct ObservedClose(usize);
struct ObjectPlugin {
    label: usize,
    count: Shared<i32>,
}
struct ObjectClose {
    label: usize,
    fields: Shared<(i32, bool)>,
}
impl NativeObject for ObjectPlugin {}
impl NativeObject for ObjectClose {}
#[derive(Clone)]
enum Alias {
    Plugin(Plugin),
    Close(Close),
}
impl Alias {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plugin(a), Self::Plugin(b)) => a.same(b),
            (Self::Close(a), Self::Close(b)) => a.same(b),
            _ => false,
        }
    }
}
impl NativeObject for ObservedPlugin {}
impl NativeObject for ObservedClose {}
fn plugin_label(p: &Plugin) -> usize {
    p.value::<ObservedPlugin>()
        .map_or_else(|| p.value::<ObjectPlugin>().unwrap().label, |v| v.0)
}
struct Model {
    input: Value,
    registry: Registry,
    manager: Manager,
    components: Vec<Handle>,
    weapon: Option<Handle>,
    mode: Option<i32>,
    events: Vec<Value>,
    frames: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: BTreeSet<usize>,
    recording: bool,
    plugins: BTreeMap<usize, Plugin>,
    aliases: BTreeMap<String, Option<Alias>>,
    objects: Vec<Alias>,
    close_attribute: Value,
    runscript: Value,
}
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
impl Model {
    fn alias(&self, a: &Value) -> Option<Alias> {
        a["alias"]
            .as_str()
            .and_then(|name| self.aliases.get(name))
            .cloned()
            .flatten()
    }
    fn plugin_alias(&self, a: &Value) -> Result<Option<Plugin>> {
        match self.alias(a) {
            None => Ok(None),
            Some(Alias::Plugin(p)) => Ok(Some(p)),
            _ => Err(Error::Type),
        }
    }
    fn close_alias(&self, a: &Value) -> Result<Option<Close>> {
        match self.alias(a) {
            None => Ok(None),
            Some(Alias::Close(p)) => Ok(Some(p)),
            _ => Err(Error::Type),
        }
    }
    fn object_ref(&self, a: Option<&Alias>) -> Option<usize> {
        a.map(|a| {
            self.objects
                .iter()
                .position(|b| a.same(b))
                .expect("unrecorded object")
        })
    }
    fn record(&mut self, a: Alias) {
        if !self.objects.iter().any(|b| a.same(b)) {
            self.objects.push(a);
        }
    }
    fn objects_snapshot(&self) -> Value {
        let aliases = self
            .aliases
            .iter()
            .map(|(k, v)| (k.clone(), self.object_ref(v.as_ref())))
            .collect::<BTreeMap<_, _>>();
        let callbacks = self
            .objects
            .iter()
            .enumerate()
            .map(|(i, a)| match a {
                Alias::Plugin(p) => {
                    let p = p.value::<ObjectPlugin>().unwrap();
                    json!({"ref":i,"kind":"plugin","label":p.label,"count":*p.count.read()})
                }
                Alias::Close(e) => {
                    if let Some(e) = e.value::<ObjectClose>() {
                        let f = e.fields.read();
                        json!({"ref":i,"kind":"close","label":e.label,"count":f.0,"result":f.1})
                    } else {
                        json!({"ref":i,"kind":"guard"})
                    }
                }
            })
            .collect::<Vec<_>>();
        let components = self
            .components
            .iter()
            .map(|c| {
                let c = c.read();
                json!([
                    self.object_ref(c.plugin.clone().map(Alias::Plugin).as_ref()),
                    self.object_ref(c.close_event.clone().map(Alias::Close).as_ref()),
                    self.object_ref(
                        c.definition
                            .as_ref()
                            .and_then(|d| d.read().plugin.clone())
                            .map(Alias::Plugin)
                            .as_ref()
                    )
                ])
            })
            .collect::<Vec<_>>();
        let registry = self
            .registry
            .definitions
            .iter()
            .map(|(id, d)| {
                (
                    id.to_string(),
                    self.object_ref(d.read().plugin.clone().map(Alias::Plugin).as_ref()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        json!({"callbacks":callbacks,"aliases":aliases,"components":components,"registry":registry,"attribute":self.close_attribute})
    }
    fn plugin_object(&mut self, id: usize) -> Plugin {
        self.plugins
            .entry(id)
            .or_insert_with(|| Plugin::new(ObservedPlugin(id)))
            .clone()
    }
    fn slot(&self, c: &Handle) -> usize {
        self.components.iter().position(|v| v.same(c)).unwrap()
    }
    fn handle(&self, v: &Value) -> Option<Handle> {
        v.as_u64().map(|id| self.components[id as usize].clone())
    }
    fn component(&mut self, id: i32) -> Handle {
        let c = self.registry.component(id);
        self.components.push(c.clone());
        c
    }
    fn boolean(&self, key: &str, fallback: bool) -> Result<bool> {
        match self.input.get(key) {
            None | Some(Value::Null) => Ok(fallback),
            Some(Value::Bool(v)) => Ok(*v),
            _ => Err(Error::Type),
        }
    }
    fn emit(&mut self, phase: &str, args: Vec<Value>) -> Result<()> {
        if !self.recording {
            return Ok(());
        }
        let mut row = vec![json!(phase)];
        row.extend(args);
        self.events.push(json!(row));
        let at = self.calls.entry(phase.into()).or_default();
        *at += 1;
        let at = *at;
        for (i, h) in self.input["hooks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            if h["event"] == phase
                && h["at"].as_i64().unwrap_or(1) == at as i64
                && self.hooks.insert(i)
            {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.input["failures"][phase].as_i64() == Some(at as i64) {
            return Err(Error::Binding("fixture".into()));
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let m = self.manager.clone();
        match a["op"].as_str().unwrap() {
            "objectPlugin" => {
                let alias = Alias::Plugin(Plugin::new(ObjectPlugin {
                    label: a["label"].as_u64().unwrap_or(1) as usize,
                    count: Shared::new(0),
                }));
                self.record(alias.clone());
                self.aliases
                    .insert(a["alias"].as_str().unwrap().into(), Some(alias));
            }
            "objectEvent" => {
                let alias = Alias::Close(Close::new(ObjectClose {
                    label: a["label"].as_u64().unwrap_or(1) as usize,
                    fields: Shared::new((0, a["result"] != false)),
                }));
                self.record(alias.clone());
                self.aliases
                    .insert(a["alias"].as_str().unwrap().into(), Some(alias));
            }
            "objectAlias" => {
                let alias = self
                    .aliases
                    .get(a["from"].as_str().unwrap())
                    .cloned()
                    .flatten();
                self.aliases
                    .insert(a["alias"].as_str().unwrap().into(), alias);
            }
            "removeAlias" => {
                self.aliases.remove(a["alias"].as_str().unwrap());
            }
            "callbackSame" => {
                let left = self
                    .aliases
                    .get(a["a"].as_str().unwrap())
                    .cloned()
                    .flatten();
                let right = self
                    .aliases
                    .get(a["b"].as_str().unwrap())
                    .cloned()
                    .flatten();
                return Ok(json!(match (left, right) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.same(&b),
                    _ => false,
                }));
            }
            "assignPlugin" => {
                let c = self.handle(&a["component"]);
                let plugin = self.plugin_alias(a)?;
                c.ok_or(Error::Null)?.write().plugin = plugin;
            }
            "assignDefinitionPlugin" => {
                let definition = self.registry.for_id(n(&a["id"]));
                let plugin = self.plugin_alias(a)?;
                definition.write().plugin = plugin;
            }
            "assignClose" => {
                let c = self.handle(&a["component"]);
                let event = self.close_alias(a)?;
                c.ok_or(Error::Null)?.write().close_event = event;
            }
            "aliasPlugin" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                let p = self.registry.plugin(&c).map(Alias::Plugin);
                if let Some(p) = &p {
                    self.record(p.clone());
                }
                self.aliases.insert(a["alias"].as_str().unwrap().into(), p);
            }
            "aliasClose" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                let p = c.read().close_event.clone().map(Alias::Close);
                if let Some(p) = &p {
                    self.record(p.clone());
                }
                self.aliases.insert(a["alias"].as_str().unwrap().into(), p);
            }
            "eventResult" => {
                let event = self
                    .close_alias(a)?
                    .ok_or(Error::Null)?
                    .value::<ObjectClose>()
                    .ok_or(Error::Type)?;
                event.fields.write().1 = a["value"].as_bool().ok_or(Error::Type)?;
            }
            "guard" => {
                let c = self.handle(&a["component"]);
                ui::set_unclosable(self, c.as_ref())?;
                self.record(Alias::Close(c.unwrap().read().close_event.clone().unwrap()));
            }
            "attribute" => self.close_attribute = a["value"].clone(),
            "removeDefinition" => {
                self.registry.definitions.remove(&n(&a["id"]));
            }
            "fireDefault" => {
                self.plugin_alias(a)?.ok_or(Error::Null)?;
                return Ok(json!(
                    rs09_server::world::component_objects::default_fire_event()
                ));
            }
            "newInstance" => {
                let p = Alias::Plugin(self.plugin_alias(a)?.ok_or(Error::Null)?);
                return Ok(json!(self.object_ref(Some(&p))));
            }
            "getters" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                let id = c.read().id;
                let hidden = c.read().hidden;
                let plugin = self.registry.plugin(&c).map(Alias::Plugin);
                let event = c.read().close_event.clone().map(Alias::Close);
                let current = self.registry.for_id(c.read().id);
                let same = c
                    .read()
                    .definition
                    .as_ref()
                    .is_some_and(|d| d.same(&current));
                return Ok(json!([
                    id,
                    hidden,
                    self.object_ref(plugin.as_ref()),
                    self.object_ref(event.as_ref()),
                    same
                ]));
            }
            "create" => {
                let c = if a["weapon"] == true {
                    self.create_weapon()?
                } else {
                    self.component(a["id"].as_i64().map_or(548, |v| v as i32))
                };
                return Ok(json!(self.slot(&c)));
            }
            "load" => {
                let count = self.registry.load(a["text"].as_str().unwrap())?;
                self.log(format!("Parsed {count} interface configs."))?;
            }
            "definition" => {
                let d = self.registry.for_id(n(&a["id"]));
                let mut d = d.write();
                if let Some(v) = a.get("type") {
                    d.kind = v.as_i64().map(|i| Type::ALL[i as usize]);
                }
                if let Some(v) = a["walkable"].as_bool() {
                    d.walkable = v;
                }
                if let Some(v) = a["tab"].as_i64() {
                    d.tab_index = v as i32;
                }
                if let Some(v) = a.get("plugin") {
                    d.plugin = v.as_u64().map(|v| self.plugin_object(v as usize));
                }
            }
            "component" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                if let Some(v) = a["id"].as_i64() {
                    c.write().id = v as i32;
                }
                if let Some(v) = a.get("definition") {
                    c.write().definition = v.as_i64().map(|id| self.registry.for_id(id as i32));
                }
                if let Some(v) = a.get("plugin") {
                    c.write().plugin = v.as_u64().map(|v| self.plugin_object(v as usize));
                }
                if let Some(v) = a["hidden"].as_bool() {
                    c.write().hidden = v;
                }
                if let Some(v) = a["closeEvent"].as_u64() {
                    c.write().close_event = Some(Close::new(ObservedClose(v as usize)));
                }
            }
            "windows" => {
                let c = self.handle(&a["component"]);
                let result = c.as_ref().map(|c| self.slot(c));
                let open = if let Some(v) = a.get("typed") {
                    Open::Typed(n(v))
                } else {
                    Open::Overlap(a["overlap"] == true)
                };
                ui::open_windows(self, &m, c, open)?;
                return Ok(if a.get("typed").is_some() {
                    Value::Null
                } else {
                    json!(result)
                });
            }
            "open" => {
                ui::component_open(self, &m, &self.handle(&a["component"]).ok_or(Error::Null)?)?
            }
            "close" => {
                return Ok(json!(ui::component_close(
                    self,
                    &self.handle(&a["component"]).ok_or(Error::Null)?
                )?));
            }
            "plugin" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                return Ok(json!(self.registry.plugin(&c).as_ref().map(plugin_label)));
            }
            "tab" => ui::open_tab(
                self,
                &m,
                n(&a["slot"]),
                &self.handle(&a["component"]).ok_or(Error::Null)?,
            )?,
            "tabAuto" => {
                ui::open_tab_auto(self, &m, &self.handle(&a["component"]).ok_or(Error::Null)?)?
            }
            "defaults" => ui::open_default_tabs(self, &m)?,
            "info" => ui::open_info_bars(self, &m)?,
            "chat" => {
                ui::open_chatbox(self, &m, &self.handle(&a["component"]).ok_or(Error::Null)?)?
            }
            "switch" => ui::switch_window_mode(self, &m, n(&a["mode"]))?,
            "game" => ui::send_game_configuration(self, &m)?,
            "managerOpen" => {
                let c = self.handle(&a["component"]);
                let opened = ui::open(self, &m, c)?;
                return Ok(json!(opened.as_ref().map(|c| self.slot(c))));
            }
            "managerOpenId" => {
                let opened = ui::open_id(self, &m, n(&a["id"]))?;
                return Ok(json!(opened.as_ref().map(|c| self.slot(c))));
            }
            "managerClose" => return Ok(json!(ui::close(self, &m)?)),
            "managerCloseComponent" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                return Ok(json!(ui::close_component(self, &m, &c)?));
            }
            "managerCloseChat" => ui::close_chatbox(self, &m)?,
            "managerSingle" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                let opened = ui::open_single_tab(self, &m, &c)?;
                return Ok(json!(self.slot(&opened)));
            }
            "managerCloseSingle" => return Ok(json!(ui::close_single_tab(self, &m)?)),
            "managerOverlay" => {
                let c = self.handle(&a["component"]);
                ui::open_overlay(self, &m, c)?;
            }
            "managerCloseOverlay" => ui::close_overlay(self, &m)?,
            "managerRemoveTabs" => ui::remove_tabs(
                self,
                &m,
                &a["indices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(n)
                    .collect::<Vec<_>>(),
            )?,
            "managerRestoreTabs" => ui::restore_tabs(self, &m)?,
            "managerCloseTabs" => ui::close_default_tabs(self, &m)?,
            "managerViewedTab" => ui::set_viewed_tab(self, &m, n(&a["index"]))?,
            "managerGet" => {
                let c = ui::get_component(self, &m, n(&a["id"]))?;
                return Ok(json!(c.as_ref().map(|c| self.slot(c))));
            }
            "managerWalkable" => return Ok(json!(ui::is_walkable(&m)?)),
            "managerHasMain" => return Ok(json!(ui::has_main_component(&m, n(&a["id"])))),
            "managerIsOpened" => return Ok(json!(ui::is_opened(&m))),
            "managerHasChat" => return Ok(json!(ui::has_chatbox(&m))),
            "managerSameClose" => {
                let c = self.handle(&a["component"]).ok_or(Error::Null)?;
                let other = self.handle(&a["other"]).ok_or(Error::Null)?;
                return Ok(json!(c.read().close_event == other.read().close_event));
            }
            "managerState" => {
                for (field, key) in [(0, "singleTab"), (1, "overlay"), (2, "wildyOverlay")] {
                    if let Some(v) = a.get(key) {
                        let c = self.handle(v);
                        let mut f = m.fields.write();
                        match field {
                            0 => f.single_tab = c,
                            1 => f.overlay = c,
                            _ => f.wildy_overlay = c,
                        }
                    }
                }
                if let Some(v) = a["index"].as_i64() {
                    m.fields.write().current_tab_index = v as i32;
                }
                if let Some(v) = a.get("runscript") {
                    self.runscript = v.clone();
                }
                if a.get("tabsNull").is_some() {
                    m.fields.write().tabs = None;
                }
            }
            "state" => {
                if let Some(v) = a["counter"].as_i64() {
                    m.fields.write().packet_count = v as i32;
                }
                if let Some(v) = a["mode"].as_i64() {
                    *self.mode.as_mut().ok_or(Error::Null)? = v as i32;
                }
                if let Some(v) = a["clientNull"].as_bool() {
                    self.mode = if v {
                        None
                    } else {
                        Some(a["mode"].as_i64().map_or(0, |v| v as i32))
                    };
                }
                for (field, key) in [(0, "opened"), (1, "window"), (2, "chatbox")] {
                    if let Some(v) = a.get(key) {
                        let c = self.handle(v);
                        let mut f = m.fields.write();
                        match field {
                            0 => f.opened = c,
                            1 => f.windows_pane = c,
                            _ => f.chatbox = c,
                        }
                    }
                }
                if let Some(v) = a.get("weapon") {
                    self.weapon = self.handle(v);
                }
                if let Some(v) = a["tabs"].as_array() {
                    m.fields.write().tabs =
                        Some(Shared::new(v.iter().map(|v| self.handle(v)).collect()));
                }
            }
            _ => return Err(Error::Binding("unknown action".into())),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, result: Value, error: Option<&str>) -> Value {
        let mut refs = BTreeMap::<usize, usize>::new();
        let mut definition = |d: &Def| {
            let next = refs.len();
            let id = *refs.entry(d.identity()).or_insert(next);
            let d = d.read();
            json!({"ref":id,"type":d.kind.map(|t|Type::ALL.iter().position(|k|*k==t).unwrap()),"walkable":d.walkable,"tab":d.tab_index,"plugin":d.plugin.as_ref().map(plugin_label)})
        };
        let sorted: BTreeMap<_, _> = self.registry.definitions.iter().collect();
        let definitions: BTreeMap<_, _> = sorted
            .into_iter()
            .map(|(id, d)| (id.to_string(), definition(d)))
            .collect();
        let components:Vec<_>=self.components.iter().map(|c|{let c=c.read();json!({"id":c.id,"definition":c.definition.as_ref().map(&mut definition),"plugin":c.plugin.as_ref().map(plugin_label),"hidden":c.hidden,"weapon":c.weapon})}).collect();
        let f = self.manager.fields.read();
        let slot = |c: &Option<Handle>| c.as_ref().map(|c| self.slot(c));
        let tabs = f
            .tabs
            .as_ref()
            .map(|tabs| tabs.read().iter().map(slot).collect::<Vec<_>>());
        let mut value = json!({"result":result,"error":error,"events":self.events,"frames":self.frames,"definitions":definitions,"components":components,"window":slot(&f.windows_pane),"opened":slot(&f.opened),"chatbox":slot(&f.chatbox),"counter":f.packet_count,"mode":self.mode,"tabs":tabs,"weapon":slot(&self.weapon)});
        if self.input["identity"] == true {
            value["objects"] = self.objects_snapshot();
        }
        if self.input["lifecycle"] == true {
            value["lifecycle"] = json!({"single":slot(&f.single_tab),"overlay":slot(&f.overlay),"index":f.current_tab_index,"runscript":self.runscript,"closeEvents":self.components.iter().map(|c| c.read().close_event.is_some()).collect::<Vec<_>>()});
        }
        value
    }
}
impl Context for Model {
    fn has_runscript(&mut self) -> Result<bool> {
        let present = !self.runscript.is_null();
        self.emit("runscriptGet", vec![])?;
        Ok(present)
    }
    fn remove_runscript(&mut self) -> Result<()> {
        self.runscript = Value::Null;
        self.emit("runscriptRemove", vec![])
    }
    fn dispatch_close(&mut self, component: Option<Handle>) -> Result<()> {
        self.emit(
            "dispatchClose",
            vec![json!(component.as_ref().map(|c| self.slot(c)))],
        )
    }
    fn run_script(&mut self, m: &Manager, id: i32, value: Option<i32>) -> Result<()> {
        let types = if value.is_some() { "i" } else { "" };
        self.emit(
            "script",
            vec![
                json!(id),
                json!(types),
                json!(value.into_iter().collect::<Vec<_>>()),
            ],
        )?;
        let args = value
            .map(updates::ScriptArg::Int)
            .into_iter()
            .collect::<Vec<_>>();
        let frame = updates::run_client_script(id, types.as_bytes(), &args, m.packet_count(1))
            .map_err(|e| Error::Binding(e.to_string()))?;
        self.send(frame)
    }
    fn interface_config(&mut self, m: &Manager, pane: i32, child: i32, hidden: bool) -> Result<()> {
        self.emit(
            "interfaceConfig",
            vec![json!(pane), json!(child), json!(hidden)],
        )?;
        self.send(updates::interface_hidden(
            hidden,
            m.packet_count(1),
            pane.wrapping_shl(16) | child,
        ))
    }
    fn component(&mut self, id: i32) -> Result<Handle> {
        Ok(Model::component(self, id))
    }
    fn resizable(&mut self) -> Result<bool> {
        Ok(self.mode.is_some_and(|m| m > 1))
    }
    fn send(&mut self, f: Frame) -> Result<()> {
        let hex: String = f.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.frames.push(json!([f.opcode, hex]));
        self.emit("send", vec![json!(f.opcode), json!(hex)])
    }
    fn log(&mut self, s: String) -> Result<()> {
        self.emit("log", vec![json!(s)])
    }
    fn listener_open(&mut self, c: &Handle) -> Result<()> {
        self.emit("listenerOpen", vec![json!(self.slot(c))])
    }
    fn listener_close(&mut self, c: &Handle) -> Result<bool> {
        self.emit("listenerClose", vec![json!(self.slot(c))])?;
        self.boolean("closeAllowed", true)
    }
    fn plugin_open(&mut self, plugin: Plugin, c: &Handle) -> Result<()> {
        if let Some(receiver) = plugin.value::<ObjectPlugin>() {
            let reference = self.object_ref(Some(&Alias::Plugin(plugin.clone())));
            self.emit(
                "objectPlugin",
                vec![json!(reference), json!(receiver.label), json!(self.slot(c))],
            )?;
            let count = *receiver.count.read();
            *receiver.count.write() = count.wrapping_add(1);
            return Ok(());
        }
        let receiver = plugin.value::<ObservedPlugin>().unwrap();
        self.emit("plugin", vec![json!(receiver.0), json!(self.slot(c))])
    }
    fn close_event(&mut self, event: Close, c: &Handle) -> Result<bool> {
        if let Some(receiver) = event.value::<ObjectClose>() {
            let reference = self.object_ref(Some(&Alias::Close(event.clone())));
            self.emit(
                "objectClose",
                vec![json!(reference), json!(receiver.label), json!(self.slot(c))],
            )?;
            let count = receiver.fields.read().0;
            receiver.fields.write().0 = count.wrapping_add(1);
            return Ok(receiver.fields.read().1);
        }
        if event
            .value::<rs09_server::world::component_objects::Unclosable>()
            .is_some()
        {
            return self.unclosable_attribute().map(|value| !value);
        }
        let receiver = event.value::<ObservedClose>().unwrap();
        self.emit("closeEvent", vec![json!(receiver.0), json!(self.slot(c))])?;
        self.boolean("eventClose", true)
    }
    fn open_component(&mut self, m: &Manager, c: &Handle) -> Result<()> {
        ui::component_open(self, m, c)
    }
    fn dispatch_open(&mut self, c: Handle) -> Result<()> {
        self.emit("dispatch", vec![json!(self.slot(&c))])
    }
    fn set_varp(&mut self, id: i32, value: i32) -> Result<()> {
        self.emit("varp", vec![json!(id), json!(value)])?;
        self.send(updates::varp(id, value))
    }
    fn weapon_extension(&mut self) -> Result<Option<Handle>> {
        let c = self.weapon.clone();
        self.emit("extension", vec![json!(c.as_ref().map(|c| self.slot(c)))])?;
        Ok(c)
    }
    fn create_weapon(&mut self) -> Result<Handle> {
        let c = self.component(92);
        c.write().weapon = true;
        self.emit("weaponCreate", vec![json!(self.slot(&c))])?;
        Ok(c)
    }
    fn add_weapon(&mut self, c: Handle) -> Result<()> {
        self.weapon = Some(c.clone());
        self.emit("addWeapon", vec![json!(self.slot(&c))])
    }
    fn spellbook(&mut self) -> Result<i32> {
        let n = self.input["spellbook"].as_i64().map_or(192, |v| v as i32);
        self.emit("spellbook", vec![])?;
        Ok(n)
    }
    fn has_familiar(&mut self) -> Result<bool> {
        let v = self.boolean("familiar", false)?;
        self.emit("familiar", vec![])?;
        Ok(v)
    }
    fn friends_component(&mut self) -> Result<Handle> {
        let c = self.component(550);
        self.emit("friendsCreate", vec![json!(self.slot(&c))])?;
        Ok(c)
    }
    fn autocast(&mut self) -> Result<Option<usize>> {
        let spell = self.input["autocast"].as_i64().map(|v| v as usize);
        self.emit("autocast", vec![])?;
        Ok(spell)
    }
    fn auto_spell_id(&mut self, c: &Handle, spell: usize) -> Result<i32> {
        self.emit("autoId", vec![json!(self.slot(c)), json!(spell as i32)])?;
        Ok((spell as i32).wrapping_add(10))
    }
    fn select_auto_spell(&mut self, c: &Handle, id: i32) -> Result<()> {
        self.emit("autoSelect", vec![json!(self.slot(c)), json!(id)])
    }
    fn window_mode(&mut self) -> Result<i32> {
        self.mode.ok_or(Error::Null)
    }
    fn set_window_mode(&mut self, mode: i32) -> Result<()> {
        *self.mode.as_mut().ok_or(Error::Null)? = mode;
        Ok(())
    }
    fn tutorial_complete(&mut self) -> Result<bool> {
        let v = self.boolean("tutorial", true)?;
        self.emit("tutorial", vec![])?;
        Ok(v)
    }
    fn tutorial_tabs(&mut self) -> Result<()> {
        self.emit("tutorialTabs", vec![])
    }
    fn set_unclosable_attribute(&mut self, value: bool) -> Result<()> {
        self.emit("setAttr", vec![json!(value)])?;
        self.close_attribute = json!(value);
        Ok(())
    }
    fn unclosable_attribute(&mut self) -> Result<bool> {
        self.emit("getAttr", vec![])?;
        match self.close_attribute {
            Value::Null => Ok(false),
            Value::Bool(value) => Ok(value),
            _ => Err(Error::Type),
        }
    }
}
impl LoginContext for Model {
    fn welcome(&mut self) -> Result<()> {
        self.emit("welcome", vec![])
    }
    fn configure(&mut self, m: &Manager) -> Result<()> {
        self.emit("configure", vec![])?;
        if self.input["configureUI"] == true {
            ui::open_default_tabs(self, m)?;
            ui::open_info_bars(self, m)?;
        }
        Ok(())
    }
    fn login_plugins(&mut self) -> Result<()> {
        Ok(())
    }
    fn appearance_sync(&mut self) -> Result<()> {
        self.emit("appearance", vec![])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        registry: Registry::default(),
        manager: Manager::default(),
        components: vec![],
        weapon: None,
        mode: Some(input["mode"].as_i64().map_or(2, |v| v as i32)),
        events: vec![],
        frames: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        recording: false,
        plugins: BTreeMap::new(),
        aliases: BTreeMap::new(),
        objects: vec![],
        close_attribute: Value::Null,
        runscript: Value::Null,
    };
    if let Some(text) = input["config"].as_str() {
        m.registry.load(text).unwrap();
    }
    m.recording = true;
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(e) => (
                Value::Null,
                Some(match e {
                    Error::Null => "Null",
                    Error::Index => "Index",
                    Error::Number => "Number",
                    Error::Type => "Type",
                    Error::InvalidAttack => "InvalidAttack",
                    Error::Limit => "Limit",
                    Error::Json(_) => "ParseException",
                    Error::Binding(_) => "Binding",
                }),
            ),
        };
        out.push(m.snapshot(result, error));
    }
    json!(out)
}
