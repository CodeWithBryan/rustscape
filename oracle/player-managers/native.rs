//! Native replay of the player managers corpus. The case's inputs become the
//! required services of `skull::Host`, `hint_icons::Host`, `warnings::Host`,
//! `assist::Host` and `diary::Host`; every service records its call the way
//! the oracle's adapters do. The managers, the HintIcon frames, the session
//! object, the deep-wilderness listener object and the persisted diary state
//! are the real native ones. The timer manager here is a stand-in for the
//! timers port: TimerManager.registerTimer/removeTimer/processTimers with the
//! Skulled timer's three bodies, written only with the public SkullManager
//! functions (see docs/player-managers.md).
use rs09_game_core::{java_double, java_json::Json};
use rs09_protocol::game::Frame;
use rs09_server::{
    definitions::item::Shared,
    inventory::container_listener::{Handle as Listener, NativeListener},
    player::persisted::diaries,
    world::{
        player_managers::{
            Error, Kind, PlayerRef, Result, assist, diary,
            diary_data::TYPES,
            hint_icons::{self, HintIconManager, Target},
            skull::{self, DeepWildListener, ListenerAttribute, SkullIcon, SkullManager, answers},
            warnings::{self, WarningMessages},
        },
        trade::Location,
    },
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq)]
pub struct Slot(pub i32);
impl PlayerRef for Slot {}

/// A ContainerListener the case stores in the attribute: not the manager's.
struct Foreign;
impl NativeListener for Foreign {}

fn num(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn long(value: &Value) -> i64 {
    value.as_i64().unwrap()
}
fn dbl(value: f64) -> Value {
    json!(java_double::format(value))
}
fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Inventory => "inventory",
        Kind::Equipment => "equipment",
        Kind::Bank => "bank",
    }
}
fn kind_of(value: &Value) -> Kind {
    match value.as_str().unwrap() {
        "inventory" => Kind::Inventory,
        "equipment" => Kind::Equipment,
        _ => Kind::Bank,
    }
}
fn name(error: &Error) -> String {
    match error {
        Error::Null => "Null".into(),
        Error::Index => "Index".into(),
        Error::Cast => "ClassCast".into(),
        Error::Binding(text) => text.clone(),
    }
}
fn to_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Number(n) => n
            .as_i64()
            .map_or_else(|| Json::Double(n.as_f64().unwrap()), Json::Int),
        Value::String(s) => Json::Text(s.clone()),
        Value::Array(a) => Json::Array(a.iter().map(to_json).collect()),
        Value::Object(o) => Json::Object(o.iter().map(|(k, v)| (k.clone(), to_json(v))).collect()),
    }
}

#[derive(Clone)]
enum Attr {
    Bool(bool),
    Int(i32),
    Long(i64),
    Text(String),
    Listener(Listener),
}

#[derive(Clone)]
struct Opened {
    id: i32,
    close: Option<assist::Session<Slot>>,
}

#[derive(Default)]
struct Container {
    items: Vec<Option<(i32, i32)>>,
    listeners: Vec<Listener>,
}
impl Container {
    fn new(capacity: usize, spec: &Value) -> Self {
        let mut items = vec![None; capacity];
        for row in spec.as_array().into_iter().flatten() {
            items[num(&row[0]) as usize] = Some((num(&row[1]), num(&row[2])));
        }
        Self {
            items,
            listeners: Vec::new(),
        }
    }
    fn amount(&self, id: i32) -> i32 {
        self.items
            .iter()
            .flatten()
            .filter(|(i, _)| *i == id)
            .fold(0i32, |sum, (_, a)| sum.wrapping_add(*a))
    }
}

struct Global {
    time: i64,
    exp: assist::Exp,
}

/// TimerManager's three lists, of indices into the model's spawned timers.
#[derive(Default)]
struct Timers {
    active: Vec<usize>,
    new: Vec<usize>,
    remove: Vec<usize>,
}

struct Timer {
    run_interval: i32,
    next_execution: i32,
}

struct Row {
    player: bool,
    index: i32,
    active: bool,
    location: Option<[i32; 3]>,
    name: String,
    artificial: bool,
    levels: Map<String, Value>,
    attributes: BTreeMap<String, Attr>,
    extension: Option<assist::Session<Slot>>,
    skull: SkullManager<Slot>,
    hints: HintIconManager<Slot>,
    warnings: WarningMessages,
    diaries: diaries::Manager,
    containers: [Container; 3],
    opened: Option<Opened>,
    global: Global,
    timers: Timers,
}

pub struct Model {
    input: Value,
    rows: BTreeMap<i32, Row>,
    events: Vec<Value>,
    calls: HashMap<String, i64>,
    fired: BTreeSet<usize>,
    enhanced: bool,
    clock: i64,
    sessions: Vec<assist::Session<Slot>>,
    timers: Vec<Timer>,
    completed_levels: Vec<i32>,
    labels: HashMap<(char, usize), String>,
    counters: HashMap<&'static str, usize>,
    /// Every labelled listener's allocation, held weakly: a label is keyed
    /// by the object's address, so a dropped listener's address must not be
    /// handed to a later one (it would take the old label).
    labelled: Vec<rs09_server::inventory::container_listener::WeakHandle>,
}

fn container_index(kind: Kind) -> usize {
    match kind {
        Kind::Inventory => 0,
        Kind::Equipment => 1,
        Kind::Bank => 2,
    }
}

impl Model {
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(Value::Array(row));
        let count = {
            let count = self.calls.entry(event.into()).or_default();
            *count += 1;
            *count
        };
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for (index, hook) in hooks.iter().enumerate() {
            if hook["event"] == event
                && hook["at"].as_i64().unwrap_or(1) == count
                && self.fired.insert(index)
            {
                for action in hook["actions"].as_array().unwrap() {
                    self.act(action)?;
                }
            }
        }
        if self.input["failures"][event].as_i64() == Some(count) {
            return Err(Error::Binding("Failure".into()));
        }
        Ok(())
    }
    fn answer(&self, key: &str) -> Option<bool> {
        let answers = self.input["answers"][key].as_array()?;
        let index = (self.calls[key] as usize - 1).min(answers.len() - 1);
        answers[index].as_bool()
    }
    fn label(&mut self, prefix: &'static str, identity: usize) -> Value {
        let key = (prefix.chars().next().unwrap(), identity);
        if let Some(label) = self.labels.get(&key) {
            return json!(label);
        }
        let counter = self.counters.entry(prefix).or_default();
        let label = format!("{prefix}{counter}");
        *counter += 1;
        self.labels.insert(key, label.clone());
        json!(label)
    }
    fn session_label(&mut self, session: &assist::Session<Slot>) -> Value {
        self.label("session", session.identity())
    }
    fn listener_label(&mut self, listener: &Listener) -> Value {
        self.labelled.push(listener.downgrade());
        self.label("listener", listener.identity())
    }
    fn timer_label(&mut self, timer: usize) -> Value {
        self.label("timer", timer)
    }
    fn attr_value(&mut self, attr: &Attr) -> Value {
        match attr {
            Attr::Bool(b) => json!(b),
            Attr::Int(i) => json!(i),
            Attr::Long(l) => json!(l),
            Attr::Text(t) => json!(t),
            Attr::Listener(l) => self.listener_label(l),
        }
    }
    fn row(&mut self, slot: i32) -> Result<&mut Row> {
        self.rows.get_mut(&slot).ok_or(Error::Null)
    }
    /// `(Player) entity(id)`: null for a missing node, a failing cast for an NPC.
    fn player(&self, value: &Value) -> Result<Option<Slot>> {
        let Some(id) = value.as_i64() else {
            return Ok(None);
        };
        match self.rows.get(&(id as i32)) {
            None => Ok(None),
            Some(row) if row.player => Ok(Some(Slot(id as i32))),
            Some(_) => Err(Error::Cast),
        }
    }
    fn need(&self, value: &Value) -> Result<Slot> {
        self.player(value)?.ok_or(Error::Null)
    }
    fn session(&self, value: &Value) -> Result<assist::Session<Slot>> {
        self.sessions
            .get(num(value) as usize)
            .cloned()
            .ok_or(Error::Index)
    }
    fn case_item_name(&self, id: i32) -> Option<String> {
        match self.input["names"]
            .as_object()
            .and_then(|n| n.get(&id.to_string()))
        {
            Some(name) => name.as_str().map(str::to_owned),
            None => Some(format!("Item {id}")),
        }
    }
    fn container(&mut self, player: &Slot, kind: Kind) -> Result<&mut Container> {
        Ok(&mut self.row(player.0)?.containers[container_index(kind)])
    }

    // ------------------------------------------------------------ the timers stand-in

    /// Skulled.onRegister: the white skull, skulled.
    fn skulled_on_register(&mut self, slot: i32) -> Result<()> {
        if !self.rows.get(&slot).is_some_and(|r| r.player) {
            return Ok(());
        }
        skull::set_skull_icon(self, &Slot(slot), 0)?;
        answers::set_skulled(self, &Slot(slot), true)?;
        Ok(())
    }
    /// Skulled.run and onRemoval: reset.
    fn skulled_reset(&mut self, slot: i32) -> Result<()> {
        if !self.rows.get(&slot).is_some_and(|r| r.player) {
            return Ok(());
        }
        skull::reset(self, &Slot(slot))
    }
    /// TimerManager.removeTimer(timer): never runs again, queued for removal,
    /// onRemoval with any failure printed.
    fn remove_timer_object(&mut self, slot: i32, timer: usize) {
        self.timers[timer].next_execution = i32::MAX;
        self.rows.get_mut(&slot).unwrap().timers.remove.push(timer);
        if self.skulled_reset(slot).is_err() {
            self.events.push(json!(["stackTrace"]));
        }
    }
    /// TimerManager.removeTimer(identifier): both lists walked with their
    /// iterators (a list grown during the walk fails the next step).
    fn remove_timer_named(&mut self, slot: i32, identifier: &str) -> Result<()> {
        for list in 0..2 {
            let get = |m: &Model| -> Vec<usize> {
                let t = &m.rows[&slot].timers;
                if list == 0 {
                    t.active.clone()
                } else {
                    t.new.clone()
                }
            };
            let expected = get(self).len();
            let mut cursor = 0;
            loop {
                let current = get(self);
                if cursor == current.len() {
                    break;
                }
                if current.len() != expected {
                    return Err(Error::Binding("ConcurrentModification".into()));
                }
                let timer = current[cursor];
                cursor += 1;
                if identifier == skull::SKULLED_TIMER {
                    self.remove_timer_object(slot, timer);
                }
            }
        }
        Ok(())
    }
    /// TimerManager.processTimers at world tick 0 with no modal open.
    fn process_timers(&mut self, slot: i32) -> Result<()> {
        {
            let t = &mut self.rows.get_mut(&slot).unwrap().timers;
            let remove = std::mem::take(&mut t.remove);
            t.active.retain(|x| !remove.contains(x));
            t.new.retain(|x| !remove.contains(x));
        }
        let active = self.rows[&slot].timers.active.clone();
        for timer in active {
            if self.timers[timer].next_execution > 0 {
                continue;
            }
            match self.skulled_reset(slot) {
                Ok(()) => self.remove_timer_object(slot, timer),
                Err(_) => {
                    self.emit("log", vec![json!("ERR"), json!("timer failure")])?;
                    self.events.push(json!(["stackTrace"]));
                    self.remove_timer_object(slot, timer);
                }
            }
        }
        let new = std::mem::take(&mut self.rows.get_mut(&slot).unwrap().timers.new);
        for timer in new {
            self.rows.get_mut(&slot).unwrap().timers.active.push(timer);
            self.timers[timer].next_execution = self.timers[timer].run_interval;
        }
        Ok(())
    }

    // ------------------------------------------------------------ the interface manager

    fn open_plain(&mut self, player: &Slot, id: i32) -> Result<()> {
        self.emit("open", vec![json!(player.0), json!(id)])?;
        self.row(player.0)?.opened = Some(Opened { id, close: None });
        Ok(())
    }
    fn target(&self, value: &Value) -> Option<Target<Slot, Option<[i32; 3]>>> {
        let map = value.as_object()?;
        if let Some(id) = map.get("node") {
            let row = self.rows.get(&num(id))?;
            return Some(if row.player {
                Target::Player(Slot(num(id)))
            } else {
                Target::Entity(Slot(num(id)))
            });
        }
        Some(Target::Node(map.get("location").and_then(|l| {
            l.as_array().map(|a| [num(&a[0]), num(&a[1]), num(&a[2])])
        })))
    }
}

// ---------------------------------------------------------------- services

impl skull::Host for Model {
    type Player = Slot;
    type Timer = usize;
    fn skull(&mut self, player: &Slot) -> Result<&mut SkullManager<Slot>> {
        Ok(&mut self.row(player.0)?.skull)
    }
    fn remove_timer(&mut self, player: &Slot, identifier: &str) -> Result<()> {
        self.remove_timer_named(player.0, identifier)
    }
    fn spawn_timer(&mut self, identifier: &str, run_interval: i32) -> Result<Option<usize>> {
        self.emit(
            "timerInstance",
            vec![json!(identifier), json!([run_interval])],
        )?;
        if !self.input["registry"].as_bool().unwrap_or(true)
            || identifier.to_lowercase() != skull::SKULLED_TIMER
        {
            return Ok(None);
        }
        self.timers.push(Timer {
            run_interval,
            next_execution: 0,
        });
        Ok(Some(self.timers.len() - 1))
    }
    fn register_timer(&mut self, player: &Slot, timer: usize) -> Result<()> {
        self.row(player.0)?.timers.new.push(timer);
        self.skulled_on_register(player.0)
    }
    fn appearance_skull_icon(&mut self, player: &Slot, icon: i32) -> Result<()> {
        self.emit("skullIcon", vec![json!(player.0), json!(icon)])
    }
    fn update_appearance(&mut self, player: &Slot) -> Result<()> {
        self.emit("updateAppearance", vec![json!(player.0)])
    }
    fn appearance_sync(&mut self, player: &Slot) -> Result<()> {
        self.emit("appearanceSync", vec![json!(player.0)])
    }
    fn lock_teleport(&mut self, player: &Slot, ticks: i32) -> Result<()> {
        self.emit("lockTeleport", vec![json!(player.0), json!(ticks)])
    }
    fn unlock_teleport(&mut self, player: &Slot) -> Result<()> {
        self.emit("unlockTeleport", vec![json!(player.0)])
    }
    fn enhanced_deep_wilderness(&mut self) -> bool {
        self.enhanced
    }
    fn listener_attribute(&mut self, player: &Slot) -> Result<ListenerAttribute> {
        self.emit(
            "getAttribute",
            vec![json!(player.0), json!(skull::LISTENER_ATTRIBUTE)],
        )?;
        Ok(
            match self
                .row(player.0)?
                .attributes
                .get(skull::LISTENER_ATTRIBUTE)
            {
                None => ListenerAttribute::Absent,
                Some(Attr::Listener(l)) => ListenerAttribute::Listener(l.clone()),
                Some(_) => ListenerAttribute::Other,
            },
        )
    }
    fn set_listener_attribute(&mut self, player: &Slot, listener: &Listener) -> Result<()> {
        let label = self.listener_label(listener);
        self.emit(
            "setAttribute",
            vec![json!(player.0), json!(skull::LISTENER_ATTRIBUTE), label],
        )?;
        self.row(player.0)?.attributes.insert(
            skull::LISTENER_ATTRIBUTE.into(),
            Attr::Listener(listener.clone()),
        );
        Ok(())
    }
    fn set_risk_attribute(&mut self, player: &Slot, value: i64) -> Result<()> {
        self.emit(
            "setAttribute",
            vec![json!(player.0), json!(skull::RISK_ATTRIBUTE), json!(value)],
        )?;
        self.row(player.0)?
            .attributes
            .insert(skull::RISK_ATTRIBUTE.into(), Attr::Long(value));
        Ok(())
    }
    fn remove_attribute(&mut self, player: &Slot, key: &str) -> Result<()> {
        self.emit("removeAttribute", vec![json!(player.0), json!(key)])?;
        self.row(player.0)?.attributes.remove(key);
        Ok(())
    }
    fn add_listener(&mut self, player: &Slot, kind: Kind, listener: &Listener) -> Result<()> {
        self.emit("listeners", vec![json!(player.0), json!(kind_name(kind))])?;
        let label = self.listener_label(listener);
        self.emit(
            "listenerAdd",
            vec![json!(player.0), json!(kind_name(kind)), label],
        )?;
        self.container(player, kind)?
            .listeners
            .push(listener.clone());
        Ok(())
    }
    fn remove_listener(&mut self, player: &Slot, kind: Kind, listener: &Listener) -> Result<()> {
        self.emit("listeners", vec![json!(player.0), json!(kind_name(kind))])?;
        let label = self.listener_label(listener);
        self.emit(
            "listenerRemove",
            vec![json!(player.0), json!(kind_name(kind)), label],
        )?;
        let list = &mut self.container(player, kind)?.listeners;
        if let Some(position) = list.iter().position(|l| l.same(listener)) {
            list.remove(position);
        }
        Ok(())
    }
    fn items(&mut self, player: &Slot, kind: Kind) -> Result<Vec<Option<(i32, i32)>>> {
        self.emit("toArray", vec![json!(player.0), json!(kind_name(kind))])?;
        Ok(self.container(player, kind)?.items.clone())
    }
    fn alchemy_value(&mut self, id: i32) -> Result<i32> {
        self.emit("alchemy", vec![json!(id), json!(true)])?;
        Ok(self.input["alchemy"][id.to_string()]
            .as_i64()
            .map_or(0, |v| v as i32))
    }
}

impl hint_icons::Host for Model {
    type Player = Slot;
    type Entity = Slot;
    type Node = Option<[i32; 3]>;
    fn hint_icons(&mut self, player: &Slot) -> Result<&mut HintIconManager<Slot>> {
        Ok(&mut self.row(player.0)?.hints)
    }
    fn index(&mut self, entity: &Slot) -> Result<i32> {
        self.emit("index", vec![json!(entity.0)])?;
        Ok(self.row(entity.0)?.index)
    }
    fn node_location(&mut self, node: &Option<[i32; 3]>) -> Result<Option<[i32; 3]>> {
        self.emit("location", vec![json!("node")])?;
        Ok(*node)
    }
    fn send(&mut self, player: &Slot, frame: Frame) -> Result<()> {
        let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.emit(
            "send",
            vec![json!(player.0), json!(frame.opcode), json!(hex)],
        )
    }
}

impl warnings::Host for Model {
    type Player = Slot;
    fn open_component(&mut self, player: &Slot, id: i32) -> Result<()> {
        self.open_plain(player, id)
    }
    fn set_varp(&mut self, player: &Slot, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(
            "varp",
            vec![json!(player.0), json!(id), json!(value), json!(save)],
        )
    }
    fn message(&mut self, player: &Slot, text: &str) -> Result<()> {
        self.emit("message", vec![json!(player.0), json!(text)])
    }
}

impl assist::Host for Model {
    type Player = Slot;
    type GlobalData = Slot;
    fn ironman_restricted(&mut self, player: &Slot) -> Result<bool> {
        self.emit("ironmanRestriction", vec![json!(player.0)])?;
        Ok(self.answer("ironmanRestriction").unwrap_or(false))
    }
    fn face(&mut self, player: &Slot, target: &Slot) -> Result<()> {
        self.emit("face", vec![json!(player.0), json!(target.0)])
    }
    fn add_extension(&mut self, player: &Slot, session: &assist::Session<Slot>) -> Result<()> {
        let label = self.session_label(session);
        self.emit(
            "addExtension",
            vec![json!(player.0), json!("AssistSession"), label],
        )?;
        self.row(player.0)?.extension = Some(session.clone());
        Ok(())
    }
    fn extension(&mut self, player: &Slot) -> Result<Option<assist::Session<Slot>>> {
        self.emit(
            "getExtension",
            vec![json!(player.0), json!("AssistSession")],
        )?;
        Ok(self.row(player.0)?.extension.clone())
    }
    fn remove_extension(&mut self, player: &Slot) -> Result<()> {
        self.emit(
            "removeExtension",
            vec![json!(player.0), json!("AssistSession")],
        )?;
        self.row(player.0)?.extension = None;
        Ok(())
    }
    fn lock(&mut self, player: &Slot) -> Result<()> {
        self.emit("lock", vec![json!(player.0)])
    }
    fn unlock(&mut self, player: &Slot) -> Result<()> {
        self.emit("unlock", vec![json!(player.0)])
    }
    fn open_component(&mut self, player: &Slot, session: &assist::Session<Slot>) -> Result<()> {
        self.emit("open", vec![json!(player.0), json!(assist::COMPONENT)])?;
        self.row(player.0)?.opened = Some(Opened {
            id: assist::COMPONENT,
            close: Some(session.clone()),
        });
        Ok(())
    }
    fn close_interface(&mut self, player: &Slot) -> Result<bool> {
        self.emit("close", vec![json!(player.0)])?;
        if let Some(opened) = self.row(player.0)?.opened.clone() {
            self.emit("componentClose", vec![json!(player.0), json!(opened.id)])?;
            let closed = match &opened.close {
                None => true,
                Some(session) => assist::close_event(self, session, player)?,
            };
            if closed {
                self.row(player.0)?.opened = None;
            }
        }
        Ok(self.row(player.0)?.opened.is_none())
    }
    fn restore_tabs(&mut self, player: &Slot) -> Result<()> {
        self.emit("restoreTabs", vec![json!(player.0)])
    }
    fn message(&mut self, player: &Slot, text: &str) -> Result<()> {
        self.emit("message", vec![json!(player.0), json!(text)])
    }
    fn send_string(&mut self, player: &Slot, text: &str, interface: i32, child: i32) -> Result<()> {
        self.emit(
            "string",
            vec![json!(player.0), json!(text), json!(interface), json!(child)],
        )
    }
    fn interface_config(
        &mut self,
        player: &Slot,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        self.emit(
            "ifaceConfig",
            vec![json!(player.0), json!(interface), json!(child), json!(hide)],
        )
    }
    fn username(&mut self, player: &Slot) -> Result<String> {
        self.emit("username", vec![json!(player.0)])?;
        Ok(self.row(player.0)?.name.clone())
    }
    fn animate(&mut self, player: &Slot, id: i32) -> Result<()> {
        self.emit("animate", vec![json!(player.0), json!(id)])
    }
    fn graphics(&mut self, player: &Slot, id: i32) -> Result<()> {
        self.emit("graphics", vec![json!(player.0), json!(id)])
    }
    fn audio(&mut self, player: &Slot, id: i32) -> Result<()> {
        self.emit(
            "audio",
            vec![json!(player.0), json!(id), json!(0), json!(1)],
        )
    }
    fn submit(&mut self, session: &assist::Session<Slot>) -> Result<()> {
        let label = self.session_label(session);
        self.emit("submit", vec![label])
    }
    fn set_varp(&mut self, player: &Slot, id: i32, value: i32) -> Result<()> {
        self.emit(
            "varp",
            vec![json!(player.0), json!(id), json!(value), json!(false)],
        )
    }
    fn now(&mut self) -> Result<i64> {
        self.emit("clock", vec![])?;
        Ok(self.clock)
    }
    fn global_data(&mut self, player: &Slot) -> Result<Slot> {
        self.emit("globalData", vec![json!(player.0)])?;
        Ok(player.clone())
    }
    fn assist_time(&mut self, data: &Slot) -> i64 {
        self.rows[&data.0].global.time
    }
    fn set_assist_time(&mut self, data: &Slot, time: i64) {
        self.rows.get_mut(&data.0).unwrap().global.time = time;
    }
    fn assist_experience(&mut self, data: &Slot) -> assist::Exp {
        self.rows[&data.0].global.exp.clone()
    }
    fn set_assist_experience(&mut self, data: &Slot, exp: assist::Exp) {
        self.rows.get_mut(&data.0).unwrap().global.exp = exp;
    }
    fn skills_refresh(&mut self, player: &Slot) -> Result<()> {
        self.emit("skillsRefresh", vec![json!(player.0)])
    }
    fn level(&mut self, player: &Slot, slot: i32) -> Result<i32> {
        self.emit("level", vec![json!(player.0), json!(slot)])?;
        Ok(self
            .row(player.0)?
            .levels
            .get(&slot.to_string())
            .map_or(1, num))
    }
    fn add_experience(&mut self, player: &Slot, slot: i32, experience: f64) -> Result<()> {
        self.emit(
            "addExperience",
            vec![json!(player.0), json!(slot), dbl(experience)],
        )
    }
    fn location(&mut self, player: &Slot) -> Result<Option<Location>> {
        self.emit("location", vec![json!(player.0)])?;
        Ok(self
            .row(player.0)?
            .location
            .map(|[x, y, z]| Location { x, y, z }))
    }
    fn is_active(&mut self, player: &Slot) -> Result<bool> {
        self.emit("isActive", vec![json!(player.0)])?;
        Ok(self.row(player.0)?.active)
    }
}

impl diary::Host for Model {
    type Player = Slot;
    fn diaries(&mut self, player: &Slot) -> Result<&mut diaries::Manager> {
        Ok(&mut self.row(player.0)?.diaries)
    }
    fn completed_levels(&mut self) -> &mut Vec<i32> {
        &mut self.completed_levels
    }
    fn send_string(&mut self, player: &Slot, text: &str, interface: i32, child: i32) -> Result<()> {
        self.emit(
            "string",
            vec![json!(player.0), json!(text), json!(interface), json!(child)],
        )
    }
    fn message(&mut self, player: &Slot, text: &str) -> Result<()> {
        self.emit("sendMessage", vec![json!(player.0), json!(text)])
    }
    fn messages(&mut self, player: &Slot, lines: &[String]) -> Result<()> {
        self.emit("sendMessages", vec![json!(player.0), json!(lines)])
    }
    fn npc_name(&mut self, id: i32) -> Result<Option<String>> {
        self.emit("npcDefinition", vec![json!(id)])?;
        self.emit("npcName", vec![json!(id)])?;
        Ok(
            match self.input["npcNames"]
                .as_object()
                .and_then(|n| n.get(&id.to_string()))
            {
                Some(name) => name.as_str().map(str::to_owned),
                None => Some(format!("NPC {id}")),
            },
        )
    }
    fn open_tab(&mut self, player: &Slot, slot: i32, id: i32) -> Result<()> {
        self.emit("openTab", vec![json!(player.0), json!(slot), json!(id)])
    }
    fn is_opened(&mut self, player: &Slot) -> Result<bool> {
        self.emit("isOpened", vec![json!(player.0)])?;
        Ok(self.row(player.0)?.opened.is_some())
    }
    fn open_component(&mut self, player: &Slot, id: i32) -> Result<()> {
        self.open_plain(player, id)
    }
    fn run_script(&mut self, player: &Slot, id: i32, types: &str, args: &[i32]) -> Result<()> {
        self.emit(
            "runScript",
            vec![json!(player.0), json!(id), json!(types), json!(args)],
        )
    }
    fn is_artificial(&mut self, player: &Slot) -> Result<bool> {
        self.emit("isArtificial", vec![json!(player.0)])?;
        Ok(self.row(player.0)?.artificial)
    }
    fn debug_enabled(&mut self, player: &Slot) -> Result<bool> {
        self.emit("getAttribute", vec![json!(player.0), json!("debug")])?;
        match self.row(player.0)?.attributes.get("debug") {
            None => Ok(false),
            Some(Attr::Bool(b)) => Ok(*b),
            Some(_) => Err(Error::Cast),
        }
    }
    fn debug_message(&mut self, player: &Slot, text: &str) -> Result<()> {
        self.emit("message", vec![json!(player.0), json!(text)])
    }
    fn contains(&mut self, player: &Slot, kind: Kind, (id, amount): (i32, i32)) -> Result<bool> {
        self.emit(
            "contains",
            vec![
                json!(player.0),
                json!(kind_name(kind)),
                json!(id),
                json!(amount),
            ],
        )?;
        Ok(self.container(player, kind)?.amount(id) >= amount)
    }
    fn remove(&mut self, player: &Slot, kind: Kind, (id, amount): (i32, i32)) -> Result<bool> {
        self.emit(
            "remove",
            vec![
                json!(player.0),
                json!(kind_name(kind)),
                json!(id),
                json!(amount),
            ],
        )?;
        let container = self.container(player, kind)?;
        if container.amount(id) < amount {
            return Ok(false);
        }
        let mut left = amount;
        for slot in container.items.iter_mut() {
            if left <= 0 {
                break;
            }
            if let Some((item, held)) = *slot
                && item == id
            {
                let take = left.min(held);
                left -= take;
                *slot = if held == take {
                    None
                } else {
                    Some((id, held - take))
                };
            }
        }
        Ok(true)
    }
    fn add(&mut self, player: &Slot, kind: Kind, (id, amount): (i32, i32)) -> Result<bool> {
        self.emit(
            "add",
            vec![
                json!(player.0),
                json!(kind_name(kind)),
                json!(id),
                json!(amount),
            ],
        )?;
        let container = self.container(player, kind)?;
        if let Some(slot) = container.items.iter_mut().find(|s| s.is_none()) {
            *slot = Some((id, amount));
            return Ok(true);
        }
        Ok(false)
    }
    fn free_slots(&mut self, player: &Slot, kind: Kind) -> Result<i32> {
        self.emit("freeSlots", vec![json!(player.0), json!(kind_name(kind))])?;
        Ok(self
            .container(player, kind)?
            .items
            .iter()
            .filter(|s| s.is_none())
            .count() as i32)
    }
    fn get(&mut self, player: &Slot, kind: Kind, slot: i32) -> Result<Option<(i32, i32)>> {
        self.emit(
            "get",
            vec![json!(player.0), json!(kind_name(kind)), json!(slot)],
        )?;
        let items = &self.container(player, kind)?.items;
        Ok(usize::try_from(slot)
            .ok()
            .and_then(|s| items.get(s).copied().flatten()))
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        self.emit("itemName", vec![json!(id)])?;
        Ok(self.case_item_name(id))
    }
}

// ---------------------------------------------------------------- operations

fn icon(context: Option<hint_icons::Icon<Slot>>) -> Value {
    match context {
        None => Value::Null,
        Some(icon) => {
            let c = icon.read();
            json!([
                c.player.0,
                c.slot,
                c.target_type,
                c.arrow_id,
                c.index,
                c.model_id,
                c.location,
                c.height
            ])
        }
    }
}
fn diary_type(value: &Value) -> Option<usize> {
    value
        .as_str()
        .map(|n| TYPES.iter().position(|t| t.constant == n).unwrap())
}
fn rewards_value(rewards: &[(i32, i32)]) -> Value {
    json!(
        rewards
            .iter()
            .map(|(i, a)| json!([i, a]))
            .collect::<Vec<_>>()
    )
}

impl Model {
    fn skull_op(&mut self, p: &Slot, a: &Value, op: &str) -> Result<Value> {
        let flag = a["value"].as_bool().unwrap_or(false);
        Ok(match op {
            "checkSkull" => {
                let other = a["other"]
                    .as_i64()
                    .and_then(|id| self.rows.get(&(id as i32)))
                    .filter(|r| r.player)
                    .map(|_| Slot(num(&a["other"])));
                skull::check_skull(self, p, other.as_ref())?;
                Value::Null
            }
            "setSkullIcon" => {
                skull::set_skull_icon(self, p, num(&a["icon"]))?;
                Value::Null
            }
            "reset" => {
                skull::reset(self, p)?;
                Value::Null
            }
            "getPlayer" => json!(["entity", skull::Host::skull(self, p)?.player().0]),
            "getLevel" => json!(answers::level(self, p)?),
            "setLevel" => {
                skull::set_level(self, p, num(&a["level"]))?;
                Value::Null
            }
            "isWilderness" => json!(answers::is_wilderness(self, p)?),
            "setWilderness" => {
                answers::set_wilderness(self, p, flag)?;
                Value::Null
            }
            "isSkullCheckDisabled" => json!(answers::is_skull_check_disabled(self, p)?),
            "setSkullCheckDisabled" => {
                answers::set_skull_check_disabled(self, p, flag)?;
                Value::Null
            }
            "isWildernessDisabled" => json!(answers::is_wilderness_disabled(self, p)?),
            "setWildernessDisabled" => {
                answers::set_wilderness_disabled(self, p, flag)?;
                Value::Null
            }
            "hasWildernessProtection" => json!(answers::has_wilderness_protection(self, p)?),
            "isSkulled" => json!(answers::is_skulled(self, p)?),
            "setSkulled" => {
                answers::set_skulled(self, p, flag)?;
                Value::Null
            }
            "isDeepWilderness" => json!(answers::is_deep_wilderness(self, p)?),
            "setDeepWilderness" => {
                skull::set_deep_wilderness(self, p, flag)?;
                Value::Null
            }
            "updateDWSkullIcon" => {
                skull::update_dw_skull_icon(self, p)?;
                Value::Null
            }
            "removeDWSkullIcon" => {
                skull::remove_dw_skull_icon(self, p)?;
                Value::Null
            }
            "listenerUpdate" | "listenerRefresh" => {
                let listener = match self.row(p.0)?.attributes.get(skull::LISTENER_ATTRIBUTE) {
                    None => return Err(Error::Null),
                    Some(Attr::Listener(l)) => l.clone(),
                    Some(_) => return Err(Error::Cast),
                };
                if let Some(own) = listener.value::<DeepWildListener<Slot>>() {
                    if op == "listenerUpdate" {
                        skull::listener_update(self, &own)?;
                    } else {
                        skull::listener_refresh(self, &own)?;
                    }
                } else {
                    let event = if op == "listenerUpdate" {
                        "foreignUpdate"
                    } else {
                        "foreignRefresh"
                    };
                    self.emit(event, vec![])?;
                }
                Value::Null
            }
            _ => panic!("{op}"),
        })
    }

    fn hint_op(&mut self, p: &Slot, a: &Value, op: &str) -> Result<Value> {
        let target = self.target(&a["target"]);
        let target = target.as_ref();
        let arrow = a["arrow"]
            .as_i64()
            .map_or(hint_icons::DEFAULT_ARROW, |v| v as i32);
        let model = a["model"]
            .as_i64()
            .map_or(hint_icons::DEFAULT_MODEL, |v| v as i32);
        let int = |key: &str| num(&a[key]);
        let slot_icon = |m: &mut Model| -> Result<hint_icons::Icon<Slot>> {
            hint_icons::Host::hint_icons(m, p)?
                .icon(int("slot"))?
                .ok_or(Error::Null)
        };
        Ok(match op {
            "register" => json!(hint_icons::register(self, p, target)?),
            "registerHeight" => json!(hint_icons::register_height(self, p, target, int("height"))?),
            "registerArrow" => json!(hint_icons::register_arrow(self, p, target, arrow)?),
            "registerModel" => json!(hint_icons::register_model(self, p, target, arrow, model)?),
            "registerSlot" => json!(hint_icons::register_slot(
                self,
                p,
                target,
                arrow,
                model,
                int("slot")
            )?),
            "registerSlotHeight" => json!(hint_icons::register_slot_height(
                self,
                p,
                target,
                arrow,
                model,
                int("slot"),
                int("height")
            )?),
            "registerFull" => json!(hint_icons::register_typed(
                self,
                p,
                target,
                arrow,
                model,
                int("slot"),
                int("height"),
                int("type")
            )?),
            "remove" => {
                hint_icons::remove(self, p, int("slot"))?;
                Value::Null
            }
            "clear" => {
                hint_icons::clear(self, p)?;
                Value::Null
            }
            "freeSlot" => json!(hint_icons::Host::hint_icons(self, p)?.free_slot()),
            "getIcon" => icon(hint_icons::Host::hint_icons(self, p)?.icon(int("slot"))?),
            "setTargetType" => {
                slot_icon(self)?.write().target_type = int("value");
                Value::Null
            }
            "setArrowId" => {
                slot_icon(self)?.write().arrow_id = int("value");
                Value::Null
            }
            "setHeight" => {
                slot_icon(self)?.write().height = int("value");
                Value::Null
            }
            "constants" => json!([
                hint_icons::MAXIMUM_SIZE,
                hint_icons::DEFAULT_ARROW,
                hint_icons::DEFAULT_MODEL,
                hint_icons::ARROW_CIRCLE_MODEL
            ]),
            _ => panic!("{op}"),
        })
    }

    fn warn_op(&mut self, p: &Slot, a: &Value, op: &str) -> Result<Value> {
        Ok(match op {
            "open" => {
                warnings::open(self, p)?;
                Value::Null
            }
            "getMessage" => {
                let index = num(&a["index"]);
                let position = self.row(p.0)?.warnings.message(index);
                json!([index, position])
            }
            "toggle" => {
                self.row(p.0)?.warnings.message(num(&a["index"]));
                let other = self.player(&a["other"])?;
                warnings::toggle(self, other.as_ref())?;
                Value::Null
            }
            _ => panic!("{op}"),
        })
    }

    fn register_session(&mut self, session: &assist::Session<Slot>) {
        if !self.sessions.iter().any(|s| s.same(session)) {
            self.sessions.push(session.clone());
        }
    }

    fn assist_op(&mut self, a: &Value, op: &str) -> Result<Value> {
        // As the oracle: the session label, then the node, are resolved first.
        let s = if a["session"].is_null() {
            None
        } else {
            Some(self.session(&a["session"])?)
        };
        let session = || s.clone().ok_or(Error::Null);
        let p = self.player(&a["node"])?;
        let need = |p: &Option<Slot>| p.clone().ok_or(Error::Null);
        let f = |key: &str| a[key].as_f64().unwrap();
        Ok(match op {
            "new" => {
                let made = assist::new(p.clone(), self.player(&a["partner"])?);
                self.sessions.push(made.clone());
                self.session_label(&made)
            }
            "newEmpty" => {
                let made = assist::empty();
                self.sessions.push(made.clone());
                self.session_label(&made)
            }
            "module" => {
                let made: assist::Session<Slot> = assist::empty();
                self.sessions.push(made.clone());
                let target = self.player(&a["target"])?;
                assist::module_open(self, &need(&p)?, &need(&target)?)?;
                self.session_label(&made)
            }
            "extend" => {
                let partner = self.player(&a["partner"])?;
                let made = assist::extend(self, &need(&p)?, partner.as_ref())?;
                if self
                    .row(need(&p)?.0)?
                    .extension
                    .as_ref()
                    .is_some_and(|e| e.same(&made))
                {
                    self.register_session(&made);
                }
                Value::Null
            }
            "getExtension" => {
                let found = assist::Host::extension(self, &need(&p)?)?;
                match found {
                    None => Value::Null,
                    Some(found) => {
                        self.register_session(&found);
                        self.session_label(&found)
                    }
                }
            }
            "open" => {
                assist::open(self, &session()?)?;
                Value::Null
            }
            "close" => json!(assist::Host::close_interface(self, &need(&p)?)?),
            "closeEvent" => json!(assist::close_event(self, &session()?, &need(&p)?)?),
            "toggleIcon" => {
                session()?;
                assist::toggle_icon(self, &need(&p)?, a["hide"].as_bool().unwrap_or(false))?;
                Value::Null
            }
            "addExperience" => {
                assist::add_experience(self, &session()?, num(&a["skill"]), f("exp"))?;
                Value::Null
            }
            "refresh" => {
                assist::refresh(self, &session()?)?;
                Value::Null
            }
            "toggleButton" => {
                assist::toggle_button(&session()?, num(&a["id"]) as i8)?;
                Value::Null
            }
            "isToggled" => json!(assist::is_toggled(&session()?, num(&a["skill"]))?),
            "getSkillIndex" => {
                session()?;
                json!(assist::skill_index(num(&a["skill"])))
            }
            "getSkills" => json!(session()?.read().skills.clone()),
            "getTimeLeft" => json!(assist::time_left(self, &session()?)?),
            "getTotalExperience" => dbl(assist::total_experience(&session()?)?),
            "getPlayer" => json!(
                session()?
                    .read()
                    .player
                    .as_ref()
                    .map(|s| json!(["entity", s.0]))
            ),
            "load" => {
                assist::load(self, &session()?)?;
                Value::Null
            }
            "save" => {
                assist::save(self, &session()?)?;
                Value::Null
            }
            "reset" => {
                assist::reset(self, &session()?)?;
                Value::Null
            }
            "pulse" => json!(assist::pulse(self, &session()?)?),
            "translate" => json!(assist::translate_experience(
                self,
                &session()?,
                &need(&p)?,
                num(&a["slot"]),
                f("exp"),
                f("mod")
            )?),
            "isRestricted" => json!(session()?.read().restricted),
            "getExp" => {
                let exp = session()?.read().exp.clone();
                let values: Vec<Value> = exp.read().iter().map(|v| dbl(*v)).collect();
                json!(values)
            }
            "setExp" => {
                let values: Vec<f64> = a["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                session()?.write().exp = Shared::new(values);
                Value::Null
            }
            "getPartener" => json!(
                session()?
                    .read()
                    .partener
                    .as_ref()
                    .map(|s| json!(["entity", s.0]))
            ),
            "setPartener" => {
                session()?.write().partener = p.clone();
                Value::Null
            }
            "setGlobal" => {
                let row = self.row(need(&p)?.0)?;
                if let Some(time) = a.get("time") {
                    row.global.time = long(time);
                }
                if let Some(values) = a.get("values") {
                    row.global.exp = Shared::new(
                        values
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_f64().unwrap())
                            .collect(),
                    );
                }
                Value::Null
            }
            _ => panic!("{op}"),
        })
    }

    fn diary_op(&mut self, p: &Slot, a: &Value, op: &str) -> Result<Value> {
        let kind = diary_type(&a["type"]);
        let level = a["level"].as_i64().map_or(0, |v| v as i32);
        let index = a["index"].as_i64().map_or(0, |v| v as i32);
        let actor = |m: &Model| -> Result<Slot> {
            m.need(if a.get("actor").is_some_and(|v| !v.is_null()) {
                &a["actor"]
            } else {
                &a["node"]
            })
        };
        Ok(match op {
            "parse" => {
                let data: Vec<Json> = a["data"].as_array().unwrap().iter().map(to_json).collect();
                diary::parse(self, p, &data)?;
                Value::Null
            }
            "openTab" => {
                diary::open_tab(self, p)?;
                Value::Null
            }
            "updateTask" => {
                let actor = actor(self)?;
                diary::manager_update_task(
                    self,
                    p,
                    &actor,
                    kind,
                    level,
                    index,
                    a["complete"].as_bool().unwrap_or(false),
                )?;
                Value::Null
            }
            "finishTask" => {
                let actor = actor(self)?;
                diary::finish_task(self, p, &actor, kind, level, index)?;
                Value::Null
            }
            "hasCompletedTask" => json!(diary::has_completed_task(self, p, kind, level, index)?),
            "setStarted" => {
                diary::set_started(self, p, kind, level)?;
                Value::Null
            }
            "setCompleted" => {
                diary::manager_set_completed(self, p, kind, level, index)?;
                Value::Null
            }
            "getDiary" => kind.map_or(Value::Null, |k| json!(["diary", TYPES[k].constant])),
            "getKaramjaGlove" => json!(diary::karamja_glove(self, p)?),
            "getArmour" => json!(diary::armour(self, p)?),
            "checkMiningReward" => json!(diary::check_mining_reward(self, p, num(&a["reward"]))?),
            "checkSmithReward" => {
                let bar = a["bar"]
                    .as_str()
                    .map(|b| diary::BARS.iter().position(|x| *x == b).unwrap());
                json!(diary::check_smith_reward(self, p, bar)?)
            }
            "hasGlove" => json!(diary::has_glove(self, p)?),
            "hasArmour" => json!(diary::has_armour(self, p)?),
            "isComplete" => json!(diary::diary_complete(self, p, kind)?),
            "getPlayer" => json!(["entity", p.0]),
            "getDiarys" => json!(
                TYPES
                    .iter()
                    .map(|t| json!(["diary", t.constant]))
                    .collect::<Vec<_>>()
            ),
            "resetRewards" => {
                diary::reset_rewards(self, p)?;
                Value::Null
            }
            _ => panic!("{op}"),
        })
    }

    fn achievement_op(&mut self, p: &Slot, a: &Value, op: &str) -> Result<Value> {
        let kind = diary_type(&a["type"]);
        let level = a["level"].as_i64().map_or(0, |v| v as i32);
        let index = a["index"].as_i64().map_or(0, |v| v as i32);
        let actor = self.player(if a.get("actor").is_some_and(|v| !v.is_null()) {
            &a["actor"]
        } else {
            &a["node"]
        })?;
        let actor = || actor.clone().ok_or(Error::Null);
        let k = kind.ok_or(Error::Null)?;
        let read = |m: &mut Model| -> Result<diaries::Diary> {
            diary::Host::diaries(m, p)?
                .diaries
                .iter()
                .find(|d| d.kind == k)
                .cloned()
                .ok_or(Error::Null)
        };
        let write = |m: &mut Model, f: &dyn Fn(&mut diaries::Diary) -> Result<()>| -> Result<()> {
            let d = diary::Host::diaries(m, p)?
                .diaries
                .iter_mut()
                .find(|d| d.kind == k)
                .ok_or(Error::Null)?;
            f(d)
        };
        Ok(match op {
            "open" => {
                diary::open(self, p, &actor()?, k)?;
                Value::Null
            }
            "drawStatus" => {
                diary::draw_status(self, p, &actor()?, k)?;
                Value::Null
            }
            "updateTask" => {
                diary::update_task(
                    self,
                    p,
                    &actor()?,
                    k,
                    level,
                    index,
                    a["complete"].as_bool().unwrap_or(false),
                )?;
                Value::Null
            }
            "finishTask" => {
                diary::finish_task_diary(self, p, &actor()?, k, level, index)?;
                Value::Null
            }
            "resetTask" => {
                diary::reset_task(self, p, &actor()?, k, level, index)?;
                Value::Null
            }
            "setLevelStarted" => {
                write(self, &|d| diary::set_level_started(d, level))?;
                Value::Null
            }
            "setCompleted" => {
                write(self, &|d| diary::set_completed(d, level, index))?;
                Value::Null
            }
            "isStartedLevel" => json!(diary::started(&read(self)?, level)?),
            "isStarted" => json!(diary::is_started(&read(self)?)?),
            "isCompleteTask" => json!(diary::task_complete(&read(self)?, level, index)?),
            "isCompleteLevel" => json!(diary::level_complete(&read(self)?, level)?),
            "isCompleteCumulative" => {
                json!(diary::cumulative_complete(
                    &read(self)?,
                    level,
                    a["cumulative"].as_bool().unwrap_or(false)
                )?)
            }
            "isComplete" => json!(diary::is_complete(&read(self)?)?),
            "getLevel" => json!(diary::completed_level(&read(self)?)?),
            "getReward" => json!(diary::reward_level(&read(self)?)?),
            "getLevelName" => json!(diary::level_name(&read(self)?, level)?),
            "getStatus" => json!(diary::status(&read(self)?, level)?),
            "setLevelRewarded" => {
                write(self, &|d| diary::set_level_rewarded(d, level))?;
                Value::Null
            }
            "isLevelRewarded" => json!(diary::rewarded(&read(self)?, level)?),
            "getType" => json!(TYPES[k].constant),
            "removeRewardsFor" => json!(diary::remove_rewards_for(self, p, k, level)?),
            "addRewardsFor" => json!(diary::add_rewards_for(self, p, k, level)?),
            "flagRewarded" => json!(diary::flag_rewarded(self, p, k, level)?),
            "canReplaceReward" => json!(diary::can_replace_reward(self, p, k, level)?),
            "grantReplacement" => json!(diary::grant_replacement(self, p, k, level)?),
            "hasCompletedLevel" => json!(diary::has_completed_level(self, p, k, level)?),
            "hasClaimedLevelRewards" => json!(diary::has_claimed_level_rewards(self, p, k, level)?),
            "canClaimLevelRewards" => json!(diary::can_claim_level_rewards(self, p, k, level)?),
            "getRewards" => rewards_value(diary::rewards(k, level)?),
            _ => panic!("{op}"),
        })
    }

    fn act(&mut self, a: &Value) -> Result<Value> {
        let op = a["op"].as_str().unwrap();
        let (group, member) = op.split_once('.').unwrap_or((op, ""));
        let node = &a["node"];
        Ok(match group {
            "skull" => {
                let p = self.need(node)?;
                self.skull_op(&p, a, member)?
            }
            "hint" => {
                let p = self.need(node)?;
                self.hint_op(&p, a, member)?
            }
            "warn" => {
                let p = self.need(node)?;
                self.warn_op(&p, a, member)?
            }
            "assist" => self.assist_op(a, member)?,
            "diary" => {
                let p = self.need(node)?;
                self.diary_op(&p, a, member)?
            }
            "ad" => {
                let p = self.need(node)?;
                self.achievement_op(&p, a, member)?
            }
            "setAttribute" => {
                let value = match &a["value"] {
                    Value::Bool(b) => Attr::Bool(*b),
                    Value::String(s) => Attr::Text(s.clone()),
                    Value::Number(n) => Attr::Int(n.as_i64().unwrap() as i32),
                    Value::Object(o) if o.contains_key("long") => Attr::Long(long(&o["long"])),
                    Value::Object(_) => Attr::Listener(Listener::new(Foreign)),
                    other => panic!("{other}"),
                };
                self.row(num(node))?
                    .attributes
                    .insert(a["key"].as_str().unwrap().into(), value);
                Value::Null
            }
            "clearCalls" => {
                self.calls.clear();
                Value::Null
            }
            "clock" => {
                self.clock = long(&a["now"]);
                Value::Null
            }
            "deep" => {
                self.enhanced = a["value"].as_bool().unwrap_or(false);
                Value::Null
            }
            "active" => {
                self.row(num(node))?.active = a["value"].as_bool().unwrap_or(true);
                Value::Null
            }
            "move" => {
                self.row(num(node))?.location = a["location"]
                    .as_array()
                    .map(|l| [num(&l[0]), num(&l[1]), num(&l[2])]);
                Value::Null
            }
            "invSet" => {
                let p = self.need(node)?;
                let item = a["item"].as_array().map(|i| (num(&i[0]), num(&i[1])));
                let container = self.container(&p, kind_of(&a["container"]))?;
                *container
                    .items
                    .get_mut(num(&a["slot"]) as usize)
                    .ok_or(Error::Index)? = item;
                Value::Null
            }
            "timerRun" => {
                let timer = num(&a["timer"]) as usize;
                if timer >= self.timers.len() {
                    return Err(Error::Index);
                }
                self.skulled_reset(num(node))?;
                json!(false)
            }
            "timerRemove" => {
                self.remove_timer_named(num(node), a["identifier"].as_str().unwrap())?;
                Value::Null
            }
            "timerProcess" => {
                self.process_timers(num(node))?;
                Value::Null
            }
            "skullIcons" => json!(
                SkullIcon::ALL
                    .iter()
                    .map(|i| json!([i.name(), i.id()]))
                    .collect::<Vec<_>>()
            ),
            "skullIconForId" => json!(SkullIcon::for_id(num(&a["id"])).name()),
            "threshold" => json!(skull::DEEP_WILD_DROP_RISK_THRESHOLD),
            "diaryTable" => json!(
                TYPES
                    .iter()
                    .map(|t| json!([
                        t.constant,
                        t.name,
                        t.child,
                        t.level_names,
                        t.achievements,
                        t.rewards
                            .iter()
                            .map(|l| rewards_value(l))
                            .collect::<Vec<_>>(),
                        t.info,
                        t.npcs
                    ]))
                    .collect::<Vec<_>>()
            ),
            "forChild" => {
                diary::for_child(num(&a["child"])).map_or(Value::Null, |k| json!(TYPES[k].constant))
            }
            "hasRewardEquipment" => {
                let p = self.need(node)?;
                json!(diary::has_reward_equipment(
                    self,
                    &p,
                    diary_type(&a["type"]).ok_or(Error::Null)?
                )?)
            }
            "diaryRewards" => rewards_value(diary::rewards(
                diary_type(&a["type"]).unwrap(),
                num(&a["level"]),
            )?),
            "diaryNpc" => json!(diary::npc(
                diary_type(&a["type"]).unwrap(),
                num(&a["level"])
            )?),
            "diaryAchievements" => json!(diary::achievements(
                diary_type(&a["type"]).unwrap(),
                num(&a["level"])
            )?),
            "completedLevels" => json!(self.completed_levels),
            "barOrdinal" => json!(
                diary::BARS
                    .iter()
                    .position(|b| *b == a["bar"].as_str().unwrap())
                    .unwrap()
            ),
            _ => panic!("{op}"),
        })
    }

    fn timer_list(&mut self, list: Vec<usize>) -> Value {
        let mut out = Vec::new();
        for t in list {
            let label = self.timer_label(t);
            out.push(json!([
                label,
                skull::SKULLED_TIMER,
                self.timers[t].run_interval,
                self.timers[t].next_execution
            ]));
        }
        json!(out)
    }

    fn describe(&mut self, slot: i32) -> Value {
        let mut out = Map::new();
        let s = self.rows[&slot].skull.clone();
        out.insert(
            "skull".into(),
            json!([
                s.wilderness,
                s.wilderness_disabled,
                s.level,
                s.skull_causes.iter().map(|c| c.0).collect::<Vec<_>>(),
                s.skulled,
                s.skull_check_disabled,
                s.deep_wilderness
            ]),
        );
        let hints: Vec<Value> = self.rows[&slot]
            .hints
            .icons
            .iter()
            .map(|i| icon(i.clone()))
            .collect();
        out.insert("hints".into(), json!(hints));
        let attributes: Vec<(String, Attr)> = self.rows[&slot]
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut map = Map::new();
        for (key, value) in attributes {
            let value = self.attr_value(&value);
            map.insert(key, value);
        }
        out.insert("attributes".into(), Value::Object(map));
        let mut extensions = Map::new();
        if let Some(session) = self.rows[&slot].extension.clone() {
            extensions.insert("AssistSession".into(), self.session_label(&session));
        }
        out.insert("extensions".into(), Value::Object(extensions));
        let mut containers = Map::new();
        for kind in [Kind::Inventory, Kind::Equipment, Kind::Bank] {
            let c = &self.rows[&slot].containers[container_index(kind)];
            let items: Vec<Value> = c
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| item.map(|(id, amount)| json!([i, id, amount])))
                .collect();
            let listeners = c.listeners.clone();
            let labels: Vec<Value> = listeners.iter().map(|l| self.listener_label(l)).collect();
            containers.insert(kind_name(kind).into(), json!([items, labels]));
        }
        out.insert("containers".into(), Value::Object(containers));
        let opened = self.rows[&slot]
            .opened
            .clone()
            .map(|o| json!([o.id, o.close.is_some()]));
        out.insert("opened".into(), json!(opened));
        let exp = self.rows[&slot].global.exp.clone();
        let alias = self
            .sessions
            .iter()
            .find(|s| s.read().exp.same(&exp))
            .cloned();
        let alias = alias.map_or(Value::Null, |s| self.session_label(&s));
        let values: Vec<Value> = exp.read().iter().map(|v| dbl(*v)).collect();
        out.insert(
            "global".into(),
            json!([self.rows[&slot].global.time, values, alias]),
        );
        let diaries: Vec<Value> = self.rows[&slot]
            .diaries
            .diaries
            .iter()
            .map(|d| {
                let completed: Vec<Vec<usize>> = d
                    .completed
                    .iter()
                    .map(|level| {
                        level
                            .iter()
                            .enumerate()
                            .filter(|(_, t)| **t)
                            .map(|(i, _)| i)
                            .collect()
                    })
                    .collect();
                json!([TYPES[d.kind].constant, d.started, d.rewarded, completed])
            })
            .collect();
        out.insert("diaries".into(), json!(diaries));
        out.insert("warnings".into(), json!(self.rows[&slot].warnings.messages));
        let t = &self.rows[&slot].timers;
        let (active, new, remove) = (t.active.clone(), t.new.clone(), t.remove.clone());
        let lists = [
            self.timer_list(active),
            self.timer_list(new),
            self.timer_list(remove),
        ];
        out.insert("timers".into(), json!(lists));
        Value::Object(out)
    }
}

pub fn evaluate(input: &Value) -> Value {
    let mut model = Model {
        input: input.clone(),
        rows: BTreeMap::new(),
        events: Vec::new(),
        calls: HashMap::new(),
        fired: BTreeSet::new(),
        enhanced: input["enhanced"].as_bool().unwrap_or(false),
        clock: input["clock"].as_i64().unwrap_or(1_000_000_000_000),
        sessions: Vec::new(),
        timers: Vec::new(),
        completed_levels: Vec::new(),
        labels: HashMap::new(),
        labelled: Vec::new(),
        counters: HashMap::new(),
    };
    for spec in input["nodes"].as_array().into_iter().flatten() {
        let id = num(&spec["id"]);
        let player = spec["kind"].as_str().unwrap_or("player") == "player";
        model.rows.insert(
            id,
            Row {
                player,
                index: spec["index"].as_i64().map_or(id + 1, |v| v as i32),
                active: spec["active"].as_bool().unwrap_or(true),
                location: spec["location"]
                    .as_array()
                    .map(|l| [num(&l[0]), num(&l[1]), num(&l[2])]),
                name: spec["name"]
                    .as_str()
                    .map_or_else(|| format!("player{id}"), str::to_owned),
                artificial: spec["artificial"].as_bool().unwrap_or(false),
                levels: spec["levels"].as_object().cloned().unwrap_or_default(),
                attributes: BTreeMap::new(),
                extension: None,
                skull: SkullManager::new(Slot(id)),
                hints: HintIconManager::default(),
                warnings: WarningMessages::default(),
                diaries: diaries::Manager::default(),
                containers: [
                    Container::new(28, &spec["inventory"]),
                    Container::new(14, &spec["equipment"]),
                    Container::new(496, &spec["bank"]),
                ],
                opened: None,
                global: Global {
                    time: 0,
                    exp: Shared::new(vec![0.0; 9]),
                },
                timers: Timers::default(),
            },
        );
    }
    let mut results = Vec::new();
    for action in input["actions"].as_array().unwrap() {
        match model.act(action) {
            Ok(value) => results.push(json!({ "value": value })),
            Err(error) => results.push(json!({ "error": name(&error) })),
        }
    }
    let mut players = Map::new();
    let slots: Vec<i32> = model
        .rows
        .iter()
        .filter(|(_, r)| r.player)
        .map(|(s, _)| *s)
        .collect();
    for slot in slots {
        let row = model.describe(slot);
        players.insert(slot.to_string(), row);
    }
    let sessions = model.sessions.clone();
    let described: Vec<Value> = sessions
        .iter()
        .map(|s| {
            let f = s.read();
            let values: Vec<Value> = f.exp.read().iter().map(|v| dbl(*v)).collect();
            json!([
                f.player.as_ref().map(|p| p.0),
                f.partener.as_ref().map(|p| p.0),
                f.skills,
                f.restricted,
                values,
                f.kill,
                f.time,
                assist::COMPONENT,
                true
            ])
        })
        .collect();
    let mut spawned = Vec::new();
    for t in 0..model.timers.len() {
        let label = model.timer_label(t);
        spawned.push(json!([
            label,
            skull::SKULLED_TIMER,
            model.timers[t].run_interval
        ]));
    }
    json!({
        "results": results,
        "events": model.events,
        "players": players,
        "sessions": described,
        "completedLevels": model.completed_levels,
        "timers": spawned,
        "enhanced": model.enhanced,
    })
}
