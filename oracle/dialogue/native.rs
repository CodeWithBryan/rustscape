//! Native replay of the dialogue corpus. `Model` is a recording `dialogue::Host`
//! whose services mirror the JVM stand-ins in Standins.java event for event, and
//! the scripted plugin/file bodies mirror Harness.java. They are real typed
//! native objects: labels are diagnostic output only and never select behaviour.
use rs09_server::world::{
    dialogue::{
        self as d, Arg, Attribute, Entity, Error, Event, FacialExpression, Lines, LogLevel,
        NodeHandle, Result, ScriptValue, Selection, Target, Text, Topic, TopicData,
    },
    doors::Location,
    player_walk::State,
};
use serde_json::{Value as J, json};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
};

type Plugin = d::Plugin<Model>;
type File = d::File<Model>;

fn n(v: &J, k: &str, default: i64) -> i64 {
    v[k].as_i64().unwrap_or(default)
}
fn int(v: &J) -> i32 {
    v.as_i64().unwrap() as i32
}
fn text(v: &J) -> Option<Text> {
    if v.is_null() {
        None
    } else if let Some(s) = v.as_str() {
        Some(s.encode_utf16().collect())
    } else {
        Some(
            v.as_array()
                .unwrap()
                .iter()
                .map(|u| u.as_u64().unwrap() as u16)
                .collect(),
        )
    }
}
fn units(t: &[u16]) -> J {
    if t.iter().any(|c| (0xD800..=0xDFFF).contains(c)) {
        json!(t)
    } else {
        json!(String::from_utf16(t).unwrap())
    }
}
fn opt_units(t: &Option<Text>) -> J {
    t.as_deref().map_or(J::Null, units)
}
fn expr(v: &J) -> Option<FacialExpression> {
    v.as_str()
        .map(|name| FacialExpression::value_of(name).unwrap())
}
fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Cast => "Cast",
        Error::Number => "Number",
        Error::IllegalArgument(_) => "IllegalArgument",
        Error::Binding(_) => "Binding",
    }
}
/// Discards a result the original call site also ignores.
fn done<T>(_: T) {}
fn failure() -> Error {
    Error::Binding("recording failure".into())
}

struct PlayerState {
    dispatch: Option<usize>,
    manager: Option<usize>,
    interpreter: Option<usize>,
    username: Option<Text>,
    male: bool,
    debug: bool,
    node_id: i32,
}
struct Manager {
    chat: Option<usize>,
    base: usize,
    veto: bool,
}
struct Component {
    id: i32,
    close_event: bool,
}
struct Npc {
    id: i32,
    shown: Option<usize>,
    no_queue: bool,
    no_pulses: bool,
}
struct Item {
    id: i32,
    amount: i32,
}
enum Destroy {
    Unset,
    Text(Option<Text>),
    Int,
}
struct Definition {
    zoom: i32,
    rotation_x: i32,
    rotation_y: i32,
    name: Option<Text>,
    destroy: Destroy,
}
impl Definition {
    fn new(id: i32) -> Self {
        Self {
            zoom: 2000i32.wrapping_add(id),
            rotation_x: id.wrapping_mul(3),
            rotation_y: id.wrapping_mul(7),
            name: Some(format!("Def {id}").encode_utf16().collect()),
            destroy: Destroy::Unset,
        }
    }
}

pub struct Model {
    source: J,
    events: Vec<J>,
    calls: BTreeMap<String, usize>,
    fired: BTreeSet<usize>,
    mark: usize,
    nodes: Vec<NodeHandle>,
    _state: State,
    players: Vec<PlayerState>,
    interps: Vec<d::Interpreter<Model>>,
    registry: d::Registry<Model>,
    managers: Vec<Manager>,
    comps: Vec<Component>,
    npcs: Vec<Npc>,
    items: Vec<Item>,
    defs: BTreeMap<i32, Definition>,
    npc_names: BTreeMap<i32, Option<Text>>,
    npc_missing: BTreeSet<i32>,
    server: Option<Text>,
    no_settings: bool,
    attrs: Vec<BTreeMap<String, J>>,
    protos: Vec<Plugin>,
    instances: Vec<Plugin>,
    seen: Vec<(Plugin, String)>,
    counts: BTreeMap<String, usize>,
    files: Vec<File>,
    topics: Vec<Topic<Model>>,
    acts: Vec<d::Action<Model>>,
    cur: Option<Lines>,
}

struct Act(usize);
impl d::DialogueAction<Model> for Act {
    fn handle(&self, h: &mut Model, player: &NodeHandle, button_id: i32) -> Result<()> {
        let p = h.p(player);
        h.emit("action", vec![json!(self.0), json!(p), json!(button_id)])
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Scripted content written against the native DialoguePlugin contract.
struct ScriptPlugin {
    spec: J,
    label: OnceLock<String>,
    class: &'static str,
}
impl ScriptPlugin {
    fn prototype(spec: &J, index: usize) -> Plugin {
        let mode = spec["mode"].as_str();
        let class = match (mode, spec["cls"].as_str()) {
            (Some("noctor"), _) => "NoCtorPlugin",
            (Some("reflect"), _) => "ReflectPlugin",
            (_, Some("B")) => "ScriptPluginB",
            _ => "ScriptPluginA",
        };
        Plugin::prototype(Self {
            spec: spec.clone(),
            label: OnceLock::from(format!("p{index}")),
            class,
        })
    }
    fn label(&self) -> String {
        self.label.get().cloned().unwrap_or_default()
    }
    /// The (Player) constructor followed by the harness binding its label.
    fn instance(
        h: &mut Model,
        spec: &J,
        class: &'static str,
        player: Option<NodeHandle>,
    ) -> Result<Plugin> {
        let body = Self {
            spec: spec.clone(),
            label: OnceLock::new(),
            class,
        };
        let made = Plugin::new(h, player, body)?;
        let label = format!("P{}", h.instances.len());
        made.body::<Self>().unwrap().label.set(label).unwrap();
        h.instances.push(made.clone());
        Ok(made)
    }
    fn run(&self, h: &mut Model, this: &Plugin, ops: &J, button: i32) -> Result<bool> {
        let mut ret = true;
        for o in ops.as_array().unwrap() {
            match o[0].as_str().unwrap() {
                "npc" => {
                    let l = h.lines(&o[1]);
                    done(this.npc_lines(h, &l)?)
                }
                "npcId" => {
                    let l = h.lines(&o[2]);
                    done(this.npc_id(h, int(&o[1]), &l)?)
                }
                "npcE" => {
                    let l = h.lines(&o[2]);
                    done(this.npc_expr(h, expr(&o[1]), &l)?)
                }
                "player" => {
                    let l = h.lines(&o[1]);
                    done(this.player_lines(h, &l)?)
                }
                "playerE" => {
                    let l = h.lines(&o[2]);
                    done(this.player_expr(h, expr(&o[1]), &l)?)
                }
                "npcl" => done(this.npcl(h, expr(&o[1]), text(&o[2]).as_deref())?),
                "playerl" => done(this.playerl(h, expr(&o[1]), text(&o[2]).as_deref())?),
                "options" => {
                    let l = h.lines(&o[1]);
                    done(this.options(h, &l)?)
                }
                "line" => {
                    let l = h.lines_or_null(&o[1]);
                    done(this.send_dialogue(h, l.as_ref())?)
                }
                "normal" => {
                    let entity = h.entity(&o[1]);
                    let l = h.lines(&o[3]);
                    this.send_normal_dialogue(h, entity.as_ref(), expr(&o[2]), &l)?
                }
                "stage" => this.set_stage(int(&o[1])),
                "next" => this.next(),
                "inc" => this.increment(),
                "getInc" => h.emit("value", vec![json!(this.get_and_increment())])?,
                "finish" => this.finish(),
                "end" => this.end(h)?,
                "close" => {
                    let closed = this.close(h)?;
                    h.emit("value", vec![json!(closed)])?
                }
                "loadFile" => this.load_file(h.file(&o[1]).as_ref())?,
                "addOption" => this.add_option(text(&o[1]).as_deref(), h.file(&o[2])),
                "choices" => {
                    let sent = this.send_choices(h)?;
                    h.emit("value", vec![json!(sent)])?
                }
                "topics" => {
                    let topics = h.topic_list(&o[1]);
                    let shown = this.show_topics(h, &topics)?;
                    h.emit("value", vec![json!(shown)])?
                }
                "pirate" => {
                    let word = this.pirate_gender(h)?;
                    h.emit("value", vec![units(&word)])?
                }
                "finished" => h.emit("value", vec![json!(this.is_finished())])?,
                "setFile" => this.set_file(h.file(&o[1])),
                "setNpc" => this.set_npc(o[1].as_u64().map(|i| Some(i as usize))),
                "i" => done(h.act(&o[1])?),
                "btn" => {
                    if button == int(&o[1]) {
                        ret &= self.run(h, this, &o[2], button)?;
                    }
                }
                "fail" => return Err(failure()),
                "ret" => ret = o[1].as_bool().unwrap(),
                other => panic!("unknown plugin op {other}"),
            }
        }
        Ok(ret)
    }
}
impl d::PluginBody<Model> for ScriptPlugin {
    fn class_name(&self) -> &str {
        self.class
    }
    fn ids(&self, h: &mut Model, _: &Plugin) -> Result<Vec<i32>> {
        h.emit("getIds", vec![json!(self.label())])?;
        Ok(self.spec["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(int)
            .collect())
    }
    fn new_instance(
        &self,
        h: &mut Model,
        _: &Plugin,
        player: Option<NodeHandle>,
    ) -> Result<Option<Plugin>> {
        let who = player.as_ref().map(|p| h.p(p));
        h.emit("newInstance", vec![json!(self.label()), json!(who)])?;
        match self.spec["mode"].as_str().unwrap_or("override") {
            "null" => Ok(None),
            "throw" => Err(failure()),
            // The reflective base newInstance prints the failure and returns null.
            "noctor" => {
                h.emit("trace", vec![json!("NoSuchMethodException")])?;
                Ok(None)
            }
            "reflect" => match Self::instance(h, &self.spec, self.class, player) {
                Ok(made) => Ok(Some(made)),
                Err(_) => {
                    h.emit("trace", vec![json!("InvocationTargetException")])?;
                    Ok(None)
                }
            },
            _ => Self::instance(h, &self.spec, self.class, player).map(Some),
        }
    }
    fn open(&self, h: &mut Model, this: &Plugin, args: &[Arg<Model>]) -> Result<bool> {
        if self.spec["open"].is_null() {
            return this.base_open(h, args);
        }
        h.emit("pluginOpen", vec![json!(self.label()), json!(args.len())])?;
        if let Some(Arg::Npc(npc)) = args.first() {
            this.set_npc(Some(*npc));
        }
        self.run(h, this, &self.spec["open"], 0)?;
        Ok(self.spec["openRet"] != json!(false))
    }
    fn close(&self, h: &mut Model, this: &Plugin) -> Result<bool> {
        h.emit("pluginClose", vec![json!(self.label())])?;
        if self.spec["close"] == json!("veto") {
            return Ok(false);
        }
        this.base_close(h)
    }
    fn handle(
        &self,
        h: &mut Model,
        this: &Plugin,
        interface_id: i32,
        button_id: i32,
    ) -> Result<bool> {
        let stage = this.stage();
        h.emit(
            "pluginHandle",
            vec![
                json!(self.label()),
                json!(interface_id),
                json!(button_id),
                json!(stage),
            ],
        )?;
        let script = &self.spec["script"];
        let ops = match &script[stage.to_string()] {
            J::Null => &script["*"],
            ops => ops,
        };
        if ops.is_null() {
            return Ok(true);
        }
        self.run(h, this, ops, button_id)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Scripted content written against the native DialogueFile contract.
struct ScriptFile {
    spec: J,
    label: String,
    shadow: bool,
    shadow_stage: Mutex<i32>,
    current: Option<i32>,
}
impl ScriptFile {
    fn run(&self, h: &mut Model, this: &File, ops: &J, button: i32) -> Result<()> {
        for o in ops.as_array().unwrap() {
            match o[0].as_str().unwrap() {
                "npc" => {
                    let l = h.lines(&o[1]);
                    done(this.npc_lines(h, &l)?)
                }
                "npcId" => {
                    let l = h.lines(&o[2]);
                    done(this.npc_id(h, int(&o[1]), &l)?)
                }
                "npcE" => {
                    let l = h.lines(&o[2]);
                    done(this.npc_expr(h, expr(&o[1]), &l)?)
                }
                "npcIdE" => {
                    let l = h.lines(&o[3]);
                    done(this.npc_id_expr(h, int(&o[1]), expr(&o[2]), &l)?)
                }
                "npcT" => {
                    let title = text(&o[2]).unwrap();
                    let l = h.lines(&o[4]);
                    done(this.npc_titled(h, int(&o[1]), &title, expr(&o[3]), &l)?)
                }
                "player" => {
                    let l = h.lines(&o[1]);
                    done(this.player_lines(h, &l)?)
                }
                "playerE" => {
                    let l = h.lines(&o[2]);
                    done(this.player_expr(h, expr(&o[1]), &l)?)
                }
                "npcl" => done(this.npcl(h, expr(&o[1]), text(&o[2]).as_deref())?),
                "npclId" => {
                    done(this.npcl_id(h, int(&o[1]), expr(&o[2]), text(&o[3]).as_deref())?)
                }
                "npclT" => {
                    let title = text(&o[2]).unwrap();
                    done(this.npcl_titled(
                        h,
                        int(&o[1]),
                        &title,
                        expr(&o[3]),
                        text(&o[4]).as_deref(),
                    )?)
                }
                "npcl1" => done(this.npcl_plain(h, text(&o[1]).as_deref())?),
                "playerl" => done(this.playerl(h, expr(&o[1]), text(&o[2]).as_deref())?),
                "playerl1" => done(this.playerl_plain(h, text(&o[1]).as_deref())?),
                "options" => {
                    let l = h.lines(&o[1]);
                    this.options(h, &l, None)?
                }
                "optionsT" => {
                    let l = h.lines(&o[1]);
                    this.options(h, &l, text(&o[2]).as_deref())?
                }
                "line" => {
                    let l = h.lines(&o[1]);
                    this.dialogue(h, &l)?
                }
                "normal" => {
                    let entity = h.entity(&o[1]);
                    let l = h.lines(&o[3]);
                    this.send_normal_dialogue(h, entity.as_ref(), expr(&o[2]), &l)?
                }
                "stage" => this.set_stage(h, int(&o[1]))?,
                "cur" => {
                    let stage = this.current_stage(h)?;
                    h.emit("value", vec![json!(stage)])?
                }
                "end" => this.end(h)?,
                "endFile" => this.end_file()?,
                "returnAt" => this.return_at_stage(int(&o[1]))?,
                "abandon" => this.abandon_file(h)?,
                "topics" | "topicsT" => {
                    let topics = h.topic_list(&o[1]);
                    let title = text(&o[2]);
                    let shown = this.show_topics(h, &topics, title.as_deref())?;
                    h.emit("value", vec![json!(shown)])?
                }
                "load" => {
                    let player = o[1].as_u64().map(|i| h.nodes[i as usize].clone());
                    let npc = o[2].as_u64().map(|i| Some(i as usize));
                    let interpreter = o[3].as_u64().map(|i| h.interps[i as usize].clone());
                    done(this.load(player, npc, interpreter)?)
                }
                "i" => done(h.act(&o[1])?),
                "btn" => {
                    if button == int(&o[1]) {
                        self.run(h, this, &o[2], button)?;
                    }
                }
                "fail" => return Err(failure()),
                other => panic!("unknown file op {other}"),
            }
        }
        Ok(())
    }
}
impl d::FileBody<Model> for ScriptFile {
    fn class_name(&self) -> &str {
        "ScriptFile"
    }
    fn stage(&self, _: &mut Model, this: &File) -> Result<i32> {
        Ok(if self.shadow {
            *self.shadow_stage.lock().unwrap()
        } else {
            this.base_stage()
        })
    }
    fn set_stage(&self, _: &mut Model, this: &File, value: i32) -> Result<()> {
        if self.shadow {
            *self.shadow_stage.lock().unwrap() = value;
        } else {
            this.set_base_stage(value);
        }
        Ok(())
    }
    fn current_stage(&self, h: &mut Model, this: &File) -> Result<i32> {
        match self.current {
            Some(value) => Ok(value),
            None => self.stage(h, this),
        }
    }
    fn handle(&self, h: &mut Model, this: &File, component_id: i32, button_id: i32) -> Result<()> {
        let stage = self.stage(h, this)?;
        h.emit(
            "fileHandle",
            vec![
                json!(self.label),
                json!(component_id),
                json!(button_id),
                json!(stage),
            ],
        )?;
        let script = &self.spec["script"];
        let ops = match &script[self.stage(h, this)?.to_string()] {
            J::Null => &script["*"],
            ops => ops,
        };
        if ops.is_null() {
            return Ok(());
        }
        self.run(h, this, ops, button_id)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Model {
    fn p(&self, player: &NodeHandle) -> usize {
        self.nodes.iter().position(|q| q == player).unwrap()
    }
    fn emit(&mut self, name: &str, args: Vec<J>) -> Result<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(name.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.source["hooks"].as_array().cloned().unwrap_or_default();
        for (i, hook) in hooks.iter().enumerate() {
            if hook["event"] == name && n(hook, "at", 1) as usize == count && self.fired.insert(i) {
                for action in hook["actions"].as_array().unwrap() {
                    self.act(action)?;
                }
            }
        }
        if self.source["failures"][name].as_u64() == Some(count as u64) {
            return Err(failure());
        }
        Ok(())
    }
    fn lines_or_null(&mut self, raw: &J) -> Option<Lines> {
        self.cur = raw
            .as_array()
            .map(|l| Lines::new(l.iter().map(text).collect()));
        self.cur.clone()
    }
    fn lines(&mut self, raw: &J) -> Lines {
        self.lines_or_null(raw)
            .expect("a null String[] is outside the native API")
    }
    fn file(&self, raw: &J) -> Option<File> {
        raw.as_u64().map(|i| self.files[i as usize].clone())
    }
    fn topic_list(&self, raw: &J) -> Vec<Topic<Model>> {
        raw.as_array()
            .unwrap()
            .iter()
            .map(|i| self.topics[i.as_u64().unwrap() as usize].clone())
            .collect()
    }
    fn entity(&self, raw: &J) -> Option<Entity<Model>> {
        let name = raw.as_str()?;
        let index: usize = name[1..].parse().unwrap();
        Some(if name.starts_with('p') {
            Entity::Player(self.nodes[index].clone())
        } else {
            Entity::Npc(Some(index))
        })
    }
    fn args(&self, a: &J) -> Vec<Arg<Model>> {
        let Some(list) = a["args"].as_array() else {
            return Vec::new();
        };
        list.iter()
            .map(|x| {
                if let Some(npc) = x.get("npc") {
                    Arg::Npc(Some(npc.as_u64().unwrap() as usize))
                } else if let Some(file) = x.get("file") {
                    Arg::File(self.files[file.as_u64().unwrap() as usize].clone())
                } else if let Some(s) = x.get("s") {
                    Arg::Text(text(s).unwrap())
                } else if let Some(s) = x.as_str() {
                    Arg::Text(s.encode_utf16().collect())
                } else if let Some(b) = x.as_bool() {
                    Arg::Bool(b)
                } else if x.is_null() {
                    Arg::Null
                } else {
                    Arg::Int(int(x))
                }
            })
            .collect()
    }
    fn script_values(&self, a: &J) -> Vec<ScriptValue> {
        self.args(a)
            .into_iter()
            .map(|arg| match arg {
                Arg::Null => ScriptValue::Null,
                Arg::Int(v) => ScriptValue::Int(v),
                Arg::Bool(v) => ScriptValue::Bool(v),
                Arg::Text(v) => ScriptValue::Text(v),
                _ => panic!("run-script arguments in this corpus are scalars"),
            })
            .collect()
    }
    /// A missing instance index is the harness list's IndexOutOfBoundsException.
    fn plugin_ref(&self, a: &J) -> Result<Option<Plugin>> {
        if let Some(i) = a["proto"].as_u64() {
            Ok(Some(self.protos[i as usize].clone()))
        } else if let Some(i) = a["inst"].as_u64() {
            self.instances
                .get(i as usize)
                .cloned()
                .map(Some)
                .ok_or(Error::Index)
        } else {
            Ok(None)
        }
    }
    fn label(&mut self, plugin: Option<&Plugin>) -> J {
        let Some(plugin) = plugin else {
            return J::Null;
        };
        if let Some(script) = plugin.body::<ScriptPlugin>() {
            return json!(script.label());
        }
        if let Some((_, label)) = self.seen.iter().find(|(p, _)| p.same(plugin)) {
            return json!(label);
        }
        let class = plugin.class_name().to_owned();
        let count = self.counts.entry(class.clone()).or_default();
        *count += 1;
        let label = format!("{class}#{count}");
        self.seen.push((plugin.clone(), label.clone()));
        json!(label)
    }
    fn file_label(file: Option<&File>) -> J {
        file.map_or(J::Null, |f| json!(f.body::<ScriptFile>().unwrap().label))
    }
    fn comp(c: Option<usize>) -> J {
        json!(c)
    }
    fn item(&self, raw: &J) -> Option<usize> {
        raw.as_u64().map(|i| i as usize)
    }
    fn definition(&mut self, id: i32) -> &mut Definition {
        self.defs.entry(id).or_insert_with(|| Definition::new(id))
    }

    pub fn act(&mut self, a: &J) -> Result<J> {
        let p = n(a, "p", 0) as usize;
        let interp = self.interps[n(a, "d", p as i64) as usize].clone();
        Ok(match a["op"].as_str().unwrap() {
            "open" => {
                let args = self.args(a);
                json!(interp.open(self, int(&a["key"]), &args)?)
            }
            "openName" => {
                let args = self.args(a);
                json!(interp.open_named(self, text(&a["name"]).as_deref(), &args)?)
            }
            "openFile" => {
                let (file, args) = (self.file(&a["file"]), self.args(a));
                interp.open_file(self, file.as_ref(), &args)?;
                J::Null
            }
            "handle" => {
                interp.handle(self, n(a, "c", 0) as i32, n(a, "b", 0) as i32)?;
                J::Null
            }
            "close" => json!(interp.close(self)?),
            "dialogue" => {
                let lines = self.lines_or_null(&a["lines"]);
                Self::comp(interp.send_dialogue(self, lines.as_ref())?)
            }
            "plain" => {
                let lines = self.lines(&a["lines"]);
                Self::comp(interp.send_plain_message(self, a["hide"] == json!(true), &lines)?)
            }
            "destroy" => Self::comp(interp.send_destroy_item(self, n(a, "id", 0) as i32)?),
            "blue" => {
                let lines = self.lines(&a["lines"]);
                Self::comp(interp.send_plane_message_with_blue_title(
                    self,
                    text(&a["title"]),
                    &lines,
                )?)
            }
            "scroll" => {
                let lines = self.lines(&a["lines"]);
                Self::comp(interp.send_scroll_message_with_blue_title(
                    self,
                    text(&a["title"]),
                    &lines,
                )?)
            }
            "item" => {
                let lines = self.lines(&a["lines"]);
                if let Some(id) = a.get("id") {
                    Self::comp(interp.send_item_message_id(self, int(id), &lines)?)
                } else {
                    let item = self.item(&a["item"]);
                    Self::comp(interp.send_item_message(self, &item, &lines)?)
                }
            }
            "double" => {
                let lines = self.lines(&a["lines"]);
                if let Some(ids) = a.get("ids") {
                    Self::comp(interp.send_double_item_message_ids(
                        self,
                        int(&ids[0]),
                        int(&ids[1]),
                        &lines,
                    )?)
                } else {
                    let (first, second) = (self.item(&a["items"][0]), self.item(&a["items"][1]));
                    Self::comp(interp.send_double_item_message(self, &first, &second, &lines)?)
                }
            }
            "say" => {
                let entity = self.entity(&a["e"]);
                let npc = n(a, "npc", 0) as i32;
                let face = FacialExpression::or_unset(expr(&a["expr"]));
                let anim = n(a, "anim", -1) as i32;
                let hide = a["hide"] == json!(true);
                let lines = self.lines(&a["lines"]);
                Self::comp(match a["form"].as_str().unwrap() {
                    "eF" => interp.send_dialogues_for(self, entity.as_ref(), face, &lines)?,
                    "eI" => interp.send_dialogues_for(self, entity.as_ref(), anim, &lines)?,
                    "nFh" => interp.send_dialogues(self, npc, face, hide, &lines)?,
                    "eFh" => interp.send_dialogues_for_hidden(
                        self,
                        entity.as_ref(),
                        face,
                        hide,
                        &lines,
                    )?,
                    "eIh" => interp.send_dialogues_for_hidden(
                        self,
                        entity.as_ref(),
                        anim,
                        hide,
                        &lines,
                    )?,
                    "nF" => interp.send_dialogues(self, npc, face, false, &lines)?,
                    "nI" => interp.send_dialogues(self, npc, anim, false, &lines)?,
                    "nIh" => interp.send_dialogues(self, npc, anim, hide, &lines)?,
                    other => panic!("unknown form {other}"),
                })
            }
            "options" => {
                let lines = self.lines(&a["lines"]);
                Self::comp(interp.send_options(self, text(&a["title"]), &lines)?)
            }
            "input" => {
                let values = self.script_values(a);
                interp.send_input(self, a["string"] == json!(true), values)?;
                J::Null
            }
            "long" => {
                let values = self.script_values(a);
                interp.send_long_input(self, values)?;
                J::Null
            }
            "message" => {
                interp.send_message_input(self, text(&a["to"]))?;
                J::Null
            }
            "add" => {
                let registry = self.registry.clone();
                registry.add(int(&a["id"]), self.plugin_ref(a)?.as_ref())?;
                J::Null
            }
            "init" => {
                let (proto, registry) = (
                    self.protos[int(&a["proto"]) as usize].clone(),
                    self.registry.clone(),
                );
                proto.init(self, &registry)?;
                J::Null
            }
            "simple" => {
                let registry = self.registry.clone();
                d::SimpleDialoguePlugin::prototype::<Model>().init(self, &registry)?;
                J::Null
            }
            "contains" => json!(self.registry.contains(int(&a["id"]))),
            "keyOf" => json!(d::Registry::<Model>::dialogue_key(
                &text(&a["name"]).ok_or(Error::Null)?
            )),
            "subst" => {
                let player = a.get("nullPlayer").is_none().then(|| self.nodes[p].clone());
                units(&d::substitute(
                    self,
                    player.as_ref(),
                    text(&a["msg"]).as_deref(),
                )?)
            }
            "addAction" => {
                interp.add_action(a["action"].as_u64().map(|i| self.acts[i as usize].clone()));
                J::Null
            }
            "runAction" => {
                let action = interp
                    .actions()
                    .read()
                    .get(n(a, "index", 0) as usize)
                    .cloned()
                    .ok_or(Error::Index)?
                    .ok_or(Error::Null)?;
                let node = self.nodes[p].clone();
                action.handle(self, &node, n(a, "b", 0) as i32)?;
                J::Null
            }
            "setDialogue" => {
                interp.set_dialogue(self.plugin_ref(a)?);
                J::Null
            }
            "new" => {
                let proto = self.protos[int(&a["proto"]) as usize].clone();
                let script = proto.body::<ScriptPlugin>().unwrap();
                let class = if script.spec["cls"] == json!("B") {
                    "ScriptPluginB"
                } else {
                    "ScriptPluginA"
                };
                let player = a.get("nullPlayer").is_none().then(|| self.nodes[p].clone());
                let made = ScriptPlugin::instance(self, &script.spec, class, player)?;
                json!(made.body::<ScriptPlugin>().unwrap().label())
            }
            "exprs" => json!(
                FacialExpression::ALL
                    .iter()
                    .map(|e| json!([e.name(), e.animation_id()]))
                    .collect::<Vec<_>>()
            ),
            "pm" => {
                let target = self.plugin_ref(a)?.unwrap();
                let script = target.body::<ScriptPlugin>().unwrap();
                json!(script.run(self, &target, &a["ops"], n(a, "b", 0) as i32)?)
            }
            "fm" => {
                let target = self.files[int(&a["file"]) as usize].clone();
                target.body::<ScriptFile>().unwrap().run(
                    self,
                    &target,
                    &a["ops"],
                    n(a, "b", 0) as i32,
                )?;
                J::Null
            }
            "line" => {
                *self
                    .cur
                    .as_ref()
                    .ok_or(Error::Null)?
                    .write()
                    .get_mut(n(a, "i", 0) as usize)
                    .ok_or(Error::Index)? = text(&a["v"]);
                J::Null
            }
            "field" => {
                self.field(a, p);
                J::Null
            }
            other => panic!("unknown action {other}"),
        })
    }
    fn field(&mut self, a: &J, p: usize) {
        let value = &a["v"];
        let m = n(a, "m", p as i64) as usize;
        let index = value.as_u64().map(|v| v as usize);
        let flag = value.as_bool().unwrap_or(false);
        match a["f"].as_str().unwrap() {
            "dispatch" => self.players[p].dispatch = index,
            "manager" => self.players[p].manager = index,
            "interpreter" => self.players[p].interpreter = index,
            "chatbox" => self.managers[m].chat = index,
            "closeEvent" => {
                let c = a["c"]
                    .as_u64()
                    .map_or_else(|| self.managers[m].chat.unwrap(), |c| c as usize);
                self.comps[c].close_event = flag;
            }
            "componentId" => self.comps[int(&a["c"]) as usize].id = int(value),
            "veto" => self.managers[m].veto = flag,
            "username" => self.players[p].username = text(value),
            "male" => self.players[p].male = flag,
            "debug" => self.players[p].debug = flag,
            "nodeId" => self.players[p].node_id = int(value),
            "server" => self.server = text(value),
            "noSettings" => self.no_settings = flag,
            "npcName" => done(self.npc_names.insert(int(&a["id"]), text(value))),
            "npcMissing" => done(self.npc_missing.insert(int(&a["id"]))),
            "npcId" => self.npcs[int(&a["n"]) as usize].id = int(value),
            "shown" => self.npcs[int(&a["n"]) as usize].shown = index,
            "noQueue" => self.npcs[int(&a["n"]) as usize].no_queue = flag,
            "noPulses" => self.npcs[int(&a["n"]) as usize].no_pulses = flag,
            "itemId" => self.items[int(&a["item"]) as usize].id = int(value),
            "itemAmount" => self.items[int(&a["item"]) as usize].amount = int(value),
            "def" => {
                let def = self.definition(int(&a["id"]));
                (def.zoom, def.rotation_x, def.rotation_y) =
                    (int(&value[0]), int(&value[1]), int(&value[2]));
            }
            "defName" => self.definition(int(&a["id"])).name = text(value),
            "destroyMessage" => {
                self.definition(int(&a["id"])).destroy = if value.is_number() {
                    Destroy::Int
                } else {
                    Destroy::Text(text(value))
                }
            }
            "attr" => done(self.attrs[p].insert(a["key"].as_str().unwrap().into(), value.clone())),
            other => panic!("unknown field {other}"),
        }
    }
    fn interp_index(&self, interpreter: Option<&d::Interpreter<Model>>) -> J {
        interpreter.map_or(J::Null, |i| {
            json!(
                self.interps
                    .iter()
                    .position(|x| x.same(i))
                    .map_or(-1, |i| i as i64)
            )
        })
    }
    fn plugin_state(&mut self, plugin: &Plugin) -> J {
        let player = plugin.player().map(|p| self.p(&p));
        json!([
            self.label(Some(plugin)),
            plugin.stage(),
            plugin.is_finished(),
            Self::file_label(plugin.file().as_ref()),
            plugin.npc().flatten(),
            player,
            self.interp_index(plugin.interpreter().as_ref()),
            plugin
                .option_names()
                .iter()
                .map(|t| units(t))
                .collect::<Vec<_>>(),
            plugin
                .option_files()
                .iter()
                .map(|f| Self::file_label(f.as_ref()))
                .collect::<Vec<_>>(),
        ])
    }
    fn snapshot(&mut self, result: J, error: Option<&Error>) -> J {
        let message = match error {
            Some(Error::IllegalArgument(m)) if m.starts_with("Dialogue ") => json!(m),
            _ => J::Null,
        };
        let mut interp = Vec::new();
        for i in self.interps.clone() {
            let actions: Vec<J> = i
                .actions()
                .read()
                .iter()
                .map(|a| {
                    a.as_ref().map_or(J::Null, |a| {
                        json!(a.as_any().downcast_ref::<Act>().unwrap().0)
                    })
                })
                .collect();
            let topics: Vec<i64> = i
                .active_topics()
                .read()
                .iter()
                .map(|t| {
                    self.topics
                        .iter()
                        .position(|x| x.same(t))
                        .map_or(-1, |i| i as i64)
                })
                .collect();
            let label = self.label(i.dialogue().as_ref());
            interp.push(json!([label, i.key(), actions, topics]));
        }
        let mut plugins = Vec::new();
        for plugin in self
            .protos
            .clone()
            .iter()
            .chain(self.instances.clone().iter())
        {
            plugins.push(self.plugin_state(plugin));
        }
        let mut index = 0;
        while index < self.seen.len() {
            let plugin = self.seen[index].0.clone();
            plugins.push(self.plugin_state(&plugin));
            index += 1;
        }
        let mut files = Vec::new();
        for file in self.files.clone() {
            let body = file.body::<ScriptFile>().unwrap();
            let player = file.player().map(|p| self.p(&p));
            let owner = self.label(file.dialogue_plugin().as_ref());
            files.push(json!([
                file.base_stage(),
                *body.shadow_stage.lock().unwrap(),
                player,
                file.npc().flatten(),
                self.interp_index(file.interpreter().as_ref()),
                owner,
            ]));
        }
        let events = self.events[self.mark..].to_vec();
        self.mark = self.events.len();
        json!({
            "error": error.map(error_name),
            "message": message,
            "result": result,
            "interp": interp,
            "plugins": plugins,
            "files": files,
            "attrs": self.attrs,
            "chat": self.managers.iter().map(|m| m.chat).collect::<Vec<_>>(),
            "comps": self.comps.iter().map(|c| json!([c.id, c.close_event])).collect::<Vec<_>>(),
            "events": events,
        })
    }
    fn new(row: &J) -> Self {
        let mut state = State::default();
        let mut nodes = Vec::new();
        for i in 0..2 {
            let id = state.add(i, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
            nodes.push(state.retain_node(id).unwrap());
        }
        let registry = d::Registry::new();
        let interps = vec![
            d::Interpreter::new(Some(nodes[0].clone()), registry.clone()),
            d::Interpreter::new(Some(nodes[1].clone()), registry.clone()),
            d::Interpreter::new(None, registry.clone()),
        ];
        let npc_ids = row["npcs"]
            .as_array()
            .map_or_else(|| vec![1, 9000, 2], |l| l.iter().map(int).collect());
        let mut model = Self {
            source: json!({}),
            events: Vec::new(),
            calls: BTreeMap::new(),
            fired: BTreeSet::new(),
            mark: 0,
            _state: state,
            players: (0..2)
                .map(|i| PlayerState {
                    dispatch: Some(i),
                    manager: Some(i),
                    interpreter: Some(i),
                    username: Some(format!("Player{i}").encode_utf16().collect()),
                    male: true,
                    debug: false,
                    node_id: 0,
                })
                .collect(),
            interps,
            registry,
            managers: (0..3)
                .map(|i| Manager {
                    chat: Some(i),
                    base: i,
                    veto: false,
                })
                .collect(),
            comps: (0..3)
                .map(|_| Component {
                    id: 137,
                    close_event: false,
                })
                .collect(),
            npcs: npc_ids
                .iter()
                .enumerate()
                .map(|(i, id)| Npc {
                    id: *id,
                    shown: Some(i),
                    no_queue: false,
                    no_pulses: false,
                })
                .collect(),
            items: row["items"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .map(|it| Item {
                            id: int(&it[0]),
                            amount: int(&it[1]),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            defs: BTreeMap::new(),
            npc_names: BTreeMap::new(),
            npc_missing: BTreeSet::new(),
            server: Some("2009scape".encode_utf16().collect()),
            no_settings: false,
            attrs: vec![BTreeMap::new(), BTreeMap::new()],
            protos: Vec::new(),
            instances: Vec::new(),
            seen: Vec::new(),
            counts: BTreeMap::new(),
            files: Vec::new(),
            topics: Vec::new(),
            acts: (0..3)
                .map(|i| Arc::new(Act(i)) as d::Action<Model>)
                .collect(),
            cur: None,
            nodes,
        };
        for (i, spec) in row["files"].as_array().into_iter().flatten().enumerate() {
            model.files.push(File::new(ScriptFile {
                spec: spec.clone(),
                label: format!("F{i}"),
                shadow: spec["shadow"] == json!(true),
                shadow_stage: Mutex::new(-1),
                current: spec["current"].as_i64().map(|v| v as i32),
            }));
        }
        for t in row["topics"].as_array().into_iter().flatten() {
            let to_stage = if let Some(file) = t["file"].as_u64() {
                Target::File(model.files[file as usize].clone())
            } else if let Some(stage) = t["stage"].as_i64() {
                Target::Stage(stage as i32)
            } else {
                Target::Other
            };
            model.topics.push(Topic::new(TopicData {
                expr: expr(&t["expr"]).unwrap_or(FacialExpression::Asking),
                text: text(&t["text"]).unwrap(),
                to_stage,
                skip_player: t["skip"] == json!(true),
                show_condition: t["if"].as_bool(),
            }));
        }
        for (i, spec) in row["protos"].as_array().into_iter().flatten().enumerate() {
            model.protos.push(ScriptPlugin::prototype(spec, i));
        }
        model.source = row.clone();
        model
    }
}

impl d::Host for Model {
    type Npc = Option<usize>;
    type Item = Option<usize>;
    type ItemDefinition = i32;
    type Dispatch = Option<usize>;
    type Manager = Option<usize>;
    type Component = usize;
    type WalkingQueue = Option<usize>;
    type PulseManager = Option<usize>;

    fn interpreter(&mut self, player: &NodeHandle) -> Result<Option<d::Interpreter<Self>>> {
        let p = self.p(player);
        self.emit("interpreter", vec![json!(p)])?;
        Ok(self.players[p].interpreter.map(|i| self.interps[i].clone()))
    }
    fn dispatch(&mut self, player: &NodeHandle) -> Result<Option<usize>> {
        let p = self.p(player);
        self.emit("dispatch", vec![json!(p)])?;
        Ok(self.players[p].dispatch)
    }
    fn manager(&mut self, player: &NodeHandle) -> Result<Option<usize>> {
        let p = self.p(player);
        self.emit("manager", vec![json!(p)])?;
        Ok(self.players[p].manager)
    }
    fn username(&mut self, player: &NodeHandle) -> Result<Option<Text>> {
        let p = self.p(player);
        self.emit("username", vec![json!(p)])?;
        Ok(self.players[p].username.clone())
    }
    fn is_male(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.p(player);
        self.emit("male", vec![json!(p)])?;
        Ok(self.players[p].male)
    }
    fn is_debug(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.p(player);
        self.emit("isDebug", vec![json!(p)])?;
        Ok(self.players[p].debug)
    }
    fn player_id(&mut self, player: &NodeHandle) -> Result<i32> {
        let p = self.p(player);
        self.emit("playerId", vec![json!(p)])?;
        Ok(self.players[p].node_id)
    }
    fn send_message(&mut self, player: &NodeHandle, message: Text) -> Result<()> {
        let p = self.p(player);
        self.emit("message", vec![json!(p), units(&message)])
    }
    fn set_attribute(&mut self, player: &NodeHandle, key: &str, value: Attribute) -> Result<()> {
        let p = self.p(player);
        let value = match value {
            Attribute::Int(v) => json!(v),
            Attribute::Bool(v) => json!(v),
        };
        self.emit("setAttribute", vec![json!(p), json!(key), value.clone()])?;
        self.attrs[p].insert(key.into(), value);
        Ok(())
    }
    fn dispatch_event(&mut self, player: &NodeHandle, event: Event<Self>) -> Result<()> {
        let p = self.p(player);
        let described = match event {
            Event::Open(plugin) => json!(["open", self.label(Some(&plugin))]),
            Event::Close(plugin) => json!(["close", self.label(Some(&plugin))]),
            Event::OptionSelection {
                dialogue,
                current_stage,
                option_id,
            } => {
                let label = match dialogue {
                    Selection::Plugin(plugin) => self.label(Some(&plugin)),
                    Selection::File(file) => Self::file_label(Some(&file)),
                };
                json!(["select", label, current_stage, option_id])
            }
        };
        self.emit("event", vec![json!(p), described])
    }
    fn send_string(
        &mut self,
        d: Option<usize>,
        value: Option<Text>,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "text",
            vec![json!(d), opt_units(&value), json!(iface), json!(child)],
        )
    }
    fn send_interface_config(
        &mut self,
        d: Option<usize>,
        iface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "hidden",
            vec![json!(d), json!(iface), json!(child), json!(hidden)],
        )
    }
    fn send_animation_interface(
        &mut self,
        d: Option<usize>,
        animation: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "anim",
            vec![json!(d), json!(animation), json!(iface), json!(child)],
        )
    }
    fn send_item_on_interface(
        &mut self,
        d: Option<usize>,
        item: i32,
        amount: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "itemOn",
            vec![
                json!(d),
                json!(item),
                json!(amount),
                json!(iface),
                json!(child),
            ],
        )
    }
    fn send_npc_on_interface(
        &mut self,
        d: Option<usize>,
        npc: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "npcOn",
            vec![json!(d), json!(npc), json!(iface), json!(child)],
        )
    }
    fn send_player_on_interface(&mut self, d: Option<usize>, iface: i32, child: i32) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit("playerOn", vec![json!(d), json!(iface), json!(child)])
    }
    fn send_reposition_on_interface(
        &mut self,
        d: Option<usize>,
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "reposition",
            vec![json!(d), json!(iface), json!(child), json!(x), json!(y)],
        )
    }
    fn send_angle_on_interface(
        &mut self,
        d: Option<usize>,
        iface: i32,
        child: i32,
        zoom: i32,
        pitch: i32,
        yaw: i32,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        self.emit(
            "angle",
            vec![
                json!(d),
                json!(iface),
                json!(child),
                json!(zoom),
                json!(pitch),
                json!(yaw),
            ],
        )
    }
    fn send_run_script(
        &mut self,
        d: Option<usize>,
        id: i32,
        types: &str,
        arguments: Vec<ScriptValue>,
    ) -> Result<()> {
        let d = d.ok_or(Error::Null)?;
        let described: Vec<J> = arguments
            .iter()
            .map(|v| match v {
                ScriptValue::Null => J::Null,
                ScriptValue::Int(v) => json!(v),
                ScriptValue::Bool(v) => json!(v),
                ScriptValue::Text(v) => json!(["s", units(v)]),
            })
            .collect();
        self.emit(
            "runScript",
            vec![json!(d), json!(id), json!(types), json!(described)],
        )
    }
    fn open_chatbox(&mut self, m: Option<usize>, iface: i32) -> Result<()> {
        let m = m.ok_or(Error::Null)?;
        self.comps.push(Component {
            id: iface,
            close_event: false,
        });
        let created = self.comps.len() - 1;
        self.managers[m].chat = Some(created);
        self.emit("open", vec![json!(m), json!(iface), json!(created)])
    }
    fn close_chatbox(&mut self, m: Option<usize>) -> Result<()> {
        let m = m.ok_or(Error::Null)?;
        self.emit("closeChatbox", vec![json!(m)])?;
        if !self.managers[m].veto {
            self.managers[m].chat = Some(self.managers[m].base);
        }
        Ok(())
    }
    fn chatbox(&mut self, m: Option<usize>) -> Result<Option<usize>> {
        let m = m.ok_or(Error::Null)?;
        self.emit("chatbox", vec![json!(m)])?;
        Ok(self.managers[m].chat)
    }
    fn component_id(&mut self, c: &usize) -> Result<i32> {
        self.emit("id", vec![json!(c)])?;
        Ok(self.comps[*c].id)
    }
    fn has_close_event(&mut self, c: &usize) -> Result<bool> {
        self.emit("closeEvent", vec![json!(c)])?;
        Ok(self.comps[*c].close_event)
    }
    fn npc_id(&mut self, npc: &Option<usize>) -> Result<i32> {
        let npc = npc.ok_or(Error::Null)?;
        self.emit("npcId", vec![json!(npc)])?;
        Ok(self.npcs[npc].id)
    }
    fn shown_npc(
        &mut self,
        npc: &Option<usize>,
        player: Option<&NodeHandle>,
    ) -> Result<Option<usize>> {
        let npc = npc.ok_or(Error::Null)?;
        let who = player.map(|p| self.p(p));
        self.emit("shownNpc", vec![json!(npc), json!(who)])?;
        Ok(self.npcs[npc].shown)
    }
    fn set_dialogue_player(
        &mut self,
        npc: &Option<usize>,
        player: Option<&NodeHandle>,
    ) -> Result<()> {
        let npc = npc.ok_or(Error::Null)?;
        let who = player.map(|p| self.p(p));
        self.emit("dialoguePlayer", vec![json!(npc), json!(who)])
    }
    fn walking_queue(&mut self, npc: &Option<usize>) -> Result<Option<usize>> {
        let npc = npc.ok_or(Error::Null)?;
        self.emit("walkingQueue", vec![json!(npc)])?;
        Ok((!self.npcs[npc].no_queue).then_some(npc))
    }
    fn reset_walking_queue(&mut self, queue: Option<usize>) -> Result<()> {
        let npc = queue.ok_or(Error::Null)?;
        self.emit("walkReset", vec![json!(npc)])
    }
    fn pulse_manager(&mut self, npc: &Option<usize>) -> Result<Option<usize>> {
        let npc = npc.ok_or(Error::Null)?;
        self.emit("pulseManager", vec![json!(npc)])?;
        Ok((!self.npcs[npc].no_pulses).then_some(npc))
    }
    fn clear_pulses(&mut self, manager: Option<usize>) -> Result<()> {
        let npc = manager.ok_or(Error::Null)?;
        self.emit("pulseClear", vec![json!(npc)])
    }
    fn npc_name(&mut self, id: i32) -> Result<Option<Text>> {
        self.emit("npcDef", vec![json!(id)])?;
        if self.npc_missing.contains(&id) {
            return Err(Error::Null);
        }
        Ok(match self.npc_names.get(&id) {
            Some(name) => name.clone(),
            None => Some(format!("Npc {id}").encode_utf16().collect()),
        })
    }
    fn item_definition(&mut self, id: i32) -> Result<i32> {
        self.emit("itemDef", vec![json!(id)])?;
        self.definition(id);
        Ok(id)
    }
    fn model_zoom(&mut self, definition: &i32) -> Result<i32> {
        Ok(self.definition(*definition).zoom)
    }
    fn model_rotation_x(&mut self, definition: &i32) -> Result<i32> {
        Ok(self.definition(*definition).rotation_x)
    }
    fn model_rotation_y(&mut self, definition: &i32) -> Result<i32> {
        Ok(self.definition(*definition).rotation_y)
    }
    fn definition_name(&mut self, definition: &i32) -> Result<Option<Text>> {
        Ok(self.definition(*definition).name.clone())
    }
    fn destroy_message(&mut self, definition: &i32) -> Result<Option<Text>> {
        match &self.definition(*definition).destroy {
            Destroy::Unset | Destroy::Text(None) => Ok(None),
            Destroy::Text(Some(message)) => Ok(Some(message.clone())),
            Destroy::Int => Err(Error::Cast),
        }
    }
    fn new_item(&mut self, id: i32) -> Result<Option<usize>> {
        self.items.push(Item { id, amount: 1 });
        let created = self.items.len() - 1;
        self.emit("newItem", vec![json!(created), json!(id), json!(1)])?;
        Ok(Some(created))
    }
    fn item_id(&mut self, item: &Option<usize>) -> Result<i32> {
        Ok(self.items[item.ok_or(Error::Null)?].id)
    }
    fn item_amount(&mut self, item: &Option<usize>) -> Result<i32> {
        Ok(self.items[item.ok_or(Error::Null)?].amount)
    }
    fn server_name(&mut self) -> Result<Option<Text>> {
        self.emit("settings", vec![])?;
        if self.no_settings {
            return Err(Error::Null);
        }
        Ok(self.server.clone())
    }
    fn log(&mut self, origin: &str, level: LogLevel, message: Text) -> Result<()> {
        let level = match level {
            LogLevel::Fine => "FINE",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Err => "ERR",
            LogLevel::Debug => "DEBUG",
        };
        self.emit("log", vec![json!(origin), json!(level), units(&message)])
    }
    fn stderr(&mut self, message: Text) -> Result<()> {
        self.emit("stderr", vec![units(&message)])
    }
    fn stack_trace(&mut self, error: &Error) -> Result<()> {
        self.emit("trace", vec![json!(error_name(error))])
    }
}

pub fn evaluate(row: &J) -> J {
    let mut model = Model::new(row);
    let mut out = Vec::new();
    for action in row["actions"].as_array().unwrap() {
        let (result, error) = match model.act(action) {
            Ok(result) => (result, None),
            Err(error) => (J::Null, Some(error)),
        };
        out.push(model.snapshot(result, error.as_ref()));
    }
    json!(out)
}
