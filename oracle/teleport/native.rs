//! Native replay of the teleport corpus. The case's inputs become the required
//! services of `teleport::Host`; every service records its call. The teleport
//! manager and pulses, the pulse runner and `pulses::update_base`, the
//! Animator with its Animation/Graphics objects, ActionLocks, the walking
//! queue (reset and the final move), the zone monitor's teleport walk and
//! `Entity.isTeleBlocked`, the script processor's delay field and the
//! attribute store are the real native ones.
use rs09_server::{
    definitions::{animation as defs, item::Shared},
    player::attributes::{AttributeValue, GameAttributes},
    pulses, scripts,
    world::{
        action_locks::{self, ActionLocks, Kind as LockKind},
        animation::{self, Animation, Animator, Graphics},
        doors::Location,
        player_walk::{NodeHandle, State},
        teleport::{self, Manager, TeleportPulse, TeleportType},
        walking,
        zones::{self, Behavior, Borders, EntityKind, Monitor, RegionZone, Restriction, Zone},
    },
};
use serde_json::{Map, Value, json};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::Arc,
};

/// An error by its oracle name ("Binding" for an injected failure).
type Fail = String;
const FAILED: &str = "Binding";
const NULL_LOCATION: &str = "The entity location provided was null.Are you sure anything down the stack trace isn't providing an NPC with a null location?";

fn num(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn place(value: &Value) -> Option<Location> {
    value
        .as_array()
        .map(|a| Location::new(num(&a[0]), num(&a[1]), num(&a[2])))
}
fn coord(location: Option<&Location>) -> Value {
    location.map_or(Value::Null, |l| json!(l.coordinates()))
}

fn tp_err(name: Fail) -> teleport::Error {
    match name.as_str() {
        "Null" => teleport::Error::Null,
        "ClassCast" => teleport::Error::ClassCast,
        "ConcurrentModification" => teleport::Error::ConcurrentModification,
        _ => teleport::Error::Binding(name),
    }
}
fn tp_name(error: teleport::Error) -> Fail {
    match error {
        teleport::Error::Null => "Null".into(),
        teleport::Error::ClassCast => "ClassCast".into(),
        teleport::Error::ConcurrentModification => "ConcurrentModification".into(),
        teleport::Error::Binding(name) => name,
    }
}
fn zone_err(name: Fail) -> zones::Error {
    match name.as_str() {
        "Null" => zones::Error::Null,
        "ClassCast" => zones::Error::ClassCast,
        "ConcurrentModification" => zones::Error::ConcurrentModification,
        _ => zones::Error::Binding(name),
    }
}
fn zone_name(error: zones::Error) -> Fail {
    match error {
        zones::Error::Null => "Null".into(),
        zones::Error::ClassCast => "ClassCast".into(),
        zones::Error::ConcurrentModification => "ConcurrentModification".into(),
        zones::Error::Binding(name) => name,
        other => format!("{other:?}"),
    }
}
fn walk_err(name: Fail) -> walking::Error {
    match name.as_str() {
        "Null" => walking::Error::Null,
        "ConcurrentModification" => walking::Error::ConcurrentModification,
        _ => walking::Error::Binding(name),
    }
}
fn walk_name(error: walking::Error) -> Fail {
    match error {
        walking::Error::Null => "Null".into(),
        walking::Error::ConcurrentModification => "ConcurrentModification".into(),
        walking::Error::Binding(name) => name,
        other => format!("{other:?}"),
    }
}
fn anim_err(name: Fail) -> animation::Error {
    match name.as_str() {
        "Null" => animation::Error::Null,
        _ => animation::Error::Binding(name),
    }
}
fn anim_name(error: animation::Error) -> Fail {
    match error {
        animation::Error::Null => "Null".into(),
        animation::Error::Index => "Index".into(),
        animation::Error::Binding(name) => name,
    }
}
fn lock_err(name: Fail) -> action_locks::Error {
    match name.as_str() {
        "Null" => action_locks::Error::Null,
        _ => action_locks::Error::Binding(name),
    }
}
fn lock_name(error: action_locks::Error) -> Fail {
    match error {
        action_locks::Error::Null => "Null".into(),
        action_locks::Error::Binding(name) => name,
    }
}
fn pulse_err(name: Fail) -> pulses::Error {
    match name.as_str() {
        "Null" => pulses::Error::Null,
        "ConcurrentModification" => pulses::Error::ConcurrentModification,
        _ => pulses::Error::Binding(name),
    }
}
fn pulse_name(error: &pulses::Error) -> Fail {
    match error {
        pulses::Error::Null => "Null".into(),
        pulses::Error::Index => "Index".into(),
        pulses::Error::ConcurrentModification => "ConcurrentModification".into(),
        pulses::Error::QueueFull => "QueueFull".into(),
        pulses::Error::Binding(name) => name.clone(),
    }
}
fn unreachable<T>(what: &str) -> Result<T, Fail> {
    Err(format!("not driven: {what}"))
}

/// The "tele-pulse" attribute value: the pulse object itself.
struct PulseValue(TeleportPulse<usize>);
impl std::fmt::Display for PulseValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.kind().class_name())
    }
}

struct Row {
    player: bool,
    spec: Map<String, Value>,
    handle: NodeHandle,
    attributes: GameAttributes,
    locks: ActionLocks<()>,
    animator: Animator<usize>,
    clocks: Shared<Vec<i32>>,
    queue: walking::Queue,
    scripts: scripts::Processor,
    disabled_ticks: i32,
    monitor: Monitor<Model>,
    manager: Manager<usize>,
}

pub struct Model {
    input: Value,
    _state: State,
    rows: Vec<Row>,
    dests: Vec<Option<Location>>,
    pulses: Vec<TeleportPulse<usize>>,
    runner: pulses::Runner,
    zones: Vec<Zone<Model>>,
    answers: Vec<VecDeque<bool>>,
    factory: animation::Factory,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    counts: HashMap<String, i64>,
    fired: BTreeSet<usize>,
    ticks: i32,
}

/// A zone whose teleport check records and answers from the case.
struct Scripted(usize);
impl Behavior<Model> for Scripted {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn teleport(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        entity: &NodeHandle,
        kind: i32,
        node: Option<&()>,
    ) -> zones::Result<bool> {
        let e = host.slot(entity);
        host.emit(
            "zone_teleport",
            vec![
                json!(self.0),
                json!(e),
                json!(kind),
                node.map_or(Value::Null, |_| json!("node")),
            ],
        )
        .map_err(zone_err)?;
        Ok(host.answers[self.0].pop_front().unwrap_or(true))
    }
}

impl Model {
    fn record(&mut self, event: &str, args: Vec<Value>) {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(Value::Array(row));
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<(), Fail> {
        self.record(event, args);
        self.calls += 1;
        let count = {
            let count = self.counts.entry(event.into()).or_default();
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
                    self.run(action)?;
                }
            }
        }
        if self.calls == self.fail_at {
            return Err(FAILED.into());
        }
        Ok(())
    }
    fn read_ticks(&mut self) -> Result<i32, Fail> {
        self.emit("ticks", vec![])?;
        Ok(self.ticks)
    }
    fn slot(&self, handle: &NodeHandle) -> usize {
        self.rows
            .iter()
            .position(|row| row.handle == *handle)
            .expect("a node of the case")
    }
    fn queue_owner(&self, queue: &walking::Queue) -> usize {
        self.rows
            .iter()
            .position(|row| row.queue.state.same(&queue.state))
            .expect("a walking queue of the case")
    }
    fn pulse_ix(&mut self, pulse: &TeleportPulse<usize>) -> usize {
        if let Some(index) = self.pulses.iter().position(|p| p.same(pulse)) {
            return index;
        }
        self.pulses.push(pulse.clone());
        self.pulses.len() - 1
    }
    fn by_handle(&self, handle: &pulses::Handle) -> Option<TeleportPulse<usize>> {
        self.pulses
            .iter()
            .find(|p| p.handle().same(handle))
            .cloned()
    }
    fn dest(&self, location: Option<&Location>) -> Value {
        location
            .and_then(|l| {
                self.dests
                    .iter()
                    .position(|d| d.as_ref().is_some_and(|d| d.same(l)))
            })
            .map_or(Value::Null, |i| json!(i))
    }
    fn label(&mut self, value: Option<&AttributeValue>) -> Value {
        match value {
            None => Value::Null,
            Some(AttributeValue::Bool(b)) => json!(b),
            Some(AttributeValue::Int(n)) => json!(n),
            Some(AttributeValue::Object(o)) => {
                let pulse = o
                    .as_any()
                    .downcast_ref::<PulseValue>()
                    .expect("a pulse")
                    .0
                    .clone();
                json!(["pulse", self.pulse_ix(&pulse)])
            }
            Some(_) => panic!("unexpected attribute value"),
        }
    }
    fn timers(&self, e: usize) -> Vec<Value> {
        self.rows[e]
            .spec
            .get("timers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }
    fn entity(&self, op: &Value) -> usize {
        op.get("e").map_or(0, |v| num(v) as usize)
    }

    fn run(&mut self, op: &Value) -> Result<Value, Fail> {
        let e = self.entity(op);
        match op["op"].as_str().unwrap() {
            "send" => {
                let location = op["loc"]
                    .as_i64()
                    .and_then(|i| self.dests[i as usize].clone());
                let manager = self.rows[e].manager.clone();
                let kind = op["type"]
                    .as_str()
                    .map(|name| TeleportType::from_name(name).unwrap());
                let result = match num(&op["form"]) {
                    1 => manager.send_default(self, location.as_ref()),
                    2 => manager.send_type(self, location.as_ref(), kind),
                    _ => manager.send(self, location.as_ref(), kind, num(&op["tt"])),
                };
                Ok(json!(result.map_err(tp_name)?))
            }
            "pulse" => {
                let runner = self.runner.clone();
                runner
                    .update_all(&mut (), self)
                    .map_err(|e| pulse_name(&e))?;
                Ok(Value::Null)
            }
            "move" => {
                for index in 0..self.rows.len() {
                    let mut queue = self.rows[index].queue.clone();
                    queue.update(self).map_err(walk_name)?;
                }
                Ok(Value::Null)
            }
            "end" => {
                for row in &self.rows {
                    let mut fields = row.queue.state.write();
                    fields.entity.teleporting = false;
                    if row.player {
                        if fields.entity.update_scene {
                            fields.entity.last_scene = fields.entity.location.clone();
                        }
                        fields.entity.update_scene = false;
                    }
                }
                self.ticks = self.ticks.wrapping_add(1);
                Ok(Value::Null)
            }
            "stop" => {
                let current = self.rows[e].manager.current().ok_or("Null")?;
                current.stop(self).map_err(tp_name)?;
                Ok(Value::Null)
            }
            "start" => {
                let current = self.rows[e].manager.current().ok_or("Null")?;
                current.start(self).map_err(tp_name)?;
                Ok(Value::Null)
            }
            "stop_pulse" => {
                let pulse = self.pulses[num(&op["pulse"]) as usize].clone();
                pulse.stop(self).map_err(tp_name)?;
                Ok(Value::Null)
            }
            "teleporting" => Ok(json!(teleport::is_teleporting(self, &e).map_err(tp_name)?)),
            "get_type" => {
                if !self.rows[e].player {
                    return Err("ClassCast".into());
                }
                let book = teleport::Host::spell_book(self, &e).map_err(tp_name)?;
                Ok(json!(teleport::get_type(book).name()))
            }
            "teleport_type" => Ok(json!(self.rows[e].manager.teleport_type())),
            "set_teleport_type" => {
                self.rows[e].manager.set_teleport_type(num(&op["value"]));
                Ok(Value::Null)
            }
            "lock_teleport" => {
                let locks = self.rows[e].locks.clone();
                locks
                    .lock_kind(LockKind::Teleport, num(&op["ticks"]), &mut (), self)
                    .map_err(lock_name)?;
                Ok(Value::Null)
            }
            "lock" => {
                let locks = self.rows[e].locks.clone();
                locks
                    .lock(num(&op["ticks"]), &mut (), self)
                    .map_err(lock_name)?;
                Ok(Value::Null)
            }
            "lock_forever" => {
                let locks = self.rows[e].locks.clone();
                locks.lock_forever(&mut (), self).map_err(lock_name)?;
                Ok(Value::Null)
            }
            "unlock" => {
                self.rows[e].locks.unlock();
                Ok(Value::Null)
            }
            "clear" => {
                self.emit("clear_pulses", vec![json!(e)])?;
                Ok(Value::Null)
            }
            "timer" => {
                let name = op["name"].clone();
                let mut active = self.timers(e);
                active.retain(|t| *t != name);
                if op["on"] == json!(true) {
                    active.push(name);
                }
                self.rows[e]
                    .spec
                    .insert("timers".into(), Value::Array(active));
                Ok(Value::Null)
            }
            "attr" => {
                let key = op["key"].as_str().unwrap();
                let attributes = &mut self.rows[e].attributes;
                if op["remove"] == json!(true) {
                    attributes.remove(key);
                } else {
                    attributes.set(key, value(&op["value"]));
                }
                Ok(Value::Null)
            }
            "spellbook" => {
                self.rows[e]
                    .spec
                    .insert("spellbook".into(), op["value"].clone());
                Ok(Value::Null)
            }
            "zone_add" => {
                let zone = self.zones[num(&op["zone"]) as usize].clone();
                self.rows[e]
                    .monitor
                    .zones()
                    .add(RegionZone::new(zone, Borders::new(0, 0, 0, 0)));
                Ok(Value::Null)
            }
            "zone_remove" => {
                let zone = self.zones[num(&op["zone"]) as usize].clone();
                let list = self.rows[e].monitor.zones();
                if let Some(index) = list.values().iter().position(|z| z.zone().same(&zone)) {
                    list.remove_at(index).map_err(zone_name)?;
                }
                Ok(Value::Null)
            }
            "mutate" => {
                let to = &op["to"];
                self.dests[num(&op["d"]) as usize]
                    .as_ref()
                    .ok_or("Null")?
                    .set(num(&to[0]), num(&to[1]), num(&to[2]));
                Ok(Value::Null)
            }
            "table" => {
                let rows: Vec<Value> = TeleportType::ALL
                    .iter()
                    .map(|t| {
                        let s = t.settings();
                        json!([
                            t.name(),
                            t.ordinal(),
                            s.start_anim,
                            s.end_anim,
                            s.start_gfx,
                            s.end_gfx
                        ])
                    })
                    .collect();
                Ok(json!([rows, teleport::WILDY_TELEPORT]))
            }
            other => panic!("unknown op {other}"),
        }
    }

    fn snapshot(&mut self, e: usize) -> Value {
        let (queue, player) = (self.rows[e].queue.clone(), self.rows[e].player);
        let entity = queue.entity();
        let mut m = Map::new();
        m.insert("location".into(), coord(entity.location.as_ref()));
        m.insert("locationDest".into(), self.dest(entity.location.as_ref()));
        m.insert(
            "teleportLocation".into(),
            coord(entity.teleport_location.as_ref()),
        );
        m.insert(
            "teleportDest".into(),
            self.dest(entity.teleport_location.as_ref()),
        );
        m.insert("teleporting".into(), json!(entity.teleporting));
        m.insert("footprint".into(), coord(queue.foot_print().as_ref()));
        m.insert("points".into(), json!(queue.points().read().len()));
        m.insert("walkDir".into(), json!(queue.walk_dir()));
        let row = &self.rows[e];
        let locks: Vec<i32> = [
            LockKind::Movement,
            LockKind::Teleport,
            LockKind::Interaction,
        ]
        .into_iter()
        .map(|k| row.locks.get(k).unwrap().expiration())
        .collect();
        m.insert("locks".into(), json!(locks));
        let a = &row.animator;
        m.insert(
            "animation".into(),
            a.animation().map_or(Value::Null, |an| {
                json!([an.id(), an.delay(), an.priority().map(|p| p.name())])
            }),
        );
        m.insert(
            "graphics".into(),
            a.graphics()
                .map_or(Value::Null, |g| json!([g.id(), g.height(), g.delay()])),
        );
        m.insert("priority".into(), json!(a.priority().map(|p| p.name())));
        m.insert("animatorTicks".into(), json!(a.deadline()));
        m.insert("animationEnd".into(), json!(row.clocks.read()[1]));
        m.insert("disabledTicks".into(), json!(row.disabled_ticks));
        if player {
            m.insert("scriptDelay".into(), json!(row.scripts.delay()));
            m.insert("scene".into(), json!(entity.update_scene));
            m.insert("lastScene".into(), coord(entity.last_scene.as_ref()));
        }
        let values: BTreeMap<String, Option<AttributeValue>> = row
            .attributes
            .values
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let manager = row.manager.clone();
        let zones: Vec<Value> = row
            .monitor
            .zones()
            .values()
            .iter()
            .map(|z| {
                json!(
                    self.zones
                        .iter()
                        .position(|candidate| candidate.same(z.zone()))
                        .unwrap()
                )
            })
            .collect();
        let mut attrs = Map::new();
        for (key, value) in values {
            let label = self.label(value.as_ref());
            attrs.insert(key, label);
        }
        m.insert("attributes".into(), Value::Object(attrs));
        let current = manager
            .current()
            .map_or(Value::Null, |p| json!(self.pulse_ix(&p)));
        m.insert("current".into(), current);
        let last = manager
            .last()
            .map_or(Value::Null, |p| json!(self.pulse_ix(&p)));
        m.insert("last".into(), last);
        m.insert("teleportType".into(), json!(manager.teleport_type()));
        m.insert("zones".into(), json!(zones));
        Value::Object(m)
    }
}

fn value(v: &Value) -> Option<AttributeValue> {
    match v {
        Value::Null => None,
        Value::Bool(b) => Some(AttributeValue::Bool(*b)),
        Value::Number(n) => Some(AttributeValue::Int(n.as_i64().unwrap() as i32)),
        other => panic!("unexpected attribute {other}"),
    }
}

impl teleport::Host for Model {
    type Entity = usize;
    fn is_player(&mut self, e: &usize) -> teleport::Result<bool> {
        Ok(self.rows[*e].player)
    }
    fn teleporter(&mut self, e: &usize) -> teleport::Result<Manager<usize>> {
        Ok(self.rows[*e].manager.clone())
    }
    fn spell_book(&mut self, e: &usize) -> teleport::Result<i32> {
        self.emit("spell_book", vec![json!(e)]).map_err(tp_err)?;
        Ok(self.rows[*e].spec.get("spellbook").map_or(192, num))
    }
    fn has_timer(&mut self, e: &usize, name: &str) -> teleport::Result<bool> {
        self.emit("get_timer", vec![json!(e), json!(name)])
            .map_err(tp_err)?;
        Ok(self.timers(*e).contains(&json!(name)))
    }
    fn zone_teleport(&mut self, e: &usize, teleport_type: i32) -> teleport::Result<bool> {
        let monitor = self.rows[*e].monitor.clone();
        monitor
            .teleport(self, teleport_type, None)
            .map_err(|e| tp_err(zone_name(e)))
    }
    fn is_tele_blocked(&mut self, e: &usize) -> teleport::Result<bool> {
        let handle = self.rows[*e].handle.clone();
        zones::is_tele_blocked(self, &handle).map_err(|e| tp_err(zone_name(e)))
    }
    fn send_message(&mut self, e: &usize, text: &str) -> teleport::Result<()> {
        self.emit("message", vec![json!(e), json!(text)])
            .map_err(tp_err)
    }
    fn world_ticks(&mut self) -> teleport::Result<i32> {
        self.read_ticks().map_err(tp_err)
    }
    fn set_script_delay(&mut self, e: &usize, value: i32) -> teleport::Result<()> {
        self.emit("script_delay", vec![json!(e), json!(value)])
            .map_err(tp_err)?;
        self.rows[*e].scripts.set_delay(value);
        Ok(())
    }
    fn close_dialogue(&mut self, e: &usize) -> teleport::Result<()> {
        self.emit("close_dialogue", vec![json!(e)]).map_err(tp_err)
    }
    fn tablet_spell(&mut self, e: &usize) -> teleport::Result<bool> {
        self.emit("get_attribute", vec![json!(e), json!("tablet-spell")])
            .map_err(tp_err)?;
        match self.rows[*e].attributes.get("tablet-spell") {
            None => Ok(false),
            Some(AttributeValue::Bool(b)) => Ok(*b),
            Some(_) => Err(teleport::Error::ClassCast),
        }
    }
    fn has_attribute(&mut self, e: &usize, key: &str) -> teleport::Result<bool> {
        self.emit("get_attribute", vec![json!(e), json!(key)])
            .map_err(tp_err)?;
        Ok(self.rows[*e].attributes.get(key).is_some())
    }
    fn set_tele_pulse(&mut self, e: &usize, pulse: &TeleportPulse<usize>) -> teleport::Result<()> {
        let label = json!(["pulse", self.pulse_ix(pulse)]);
        self.emit(
            "set_attribute",
            vec![json!(e), json!("tele-pulse"), label.clone()],
        )
        .map_err(tp_err)?;
        self.rows[*e].attributes.set(
            "tele-pulse",
            Some(AttributeValue::Object(Arc::new(PulseValue(pulse.clone())))),
        );
        self.emit(
            "dispatch",
            vec![
                json!(e),
                json!("AttributeSetEvent"),
                json!("tele-pulse"),
                label,
            ],
        )
        .map_err(tp_err)
    }
    fn remove_attribute(&mut self, e: &usize, key: &str) -> teleport::Result<()> {
        self.emit("remove_attribute", vec![json!(e), json!(key)])
            .map_err(tp_err)?;
        self.rows[*e].attributes.remove(key);
        self.emit(
            "dispatch",
            vec![json!(e), json!("AttributeRemoveEvent"), json!(key)],
        )
        .map_err(tp_err)
    }
    fn reset_walking(&mut self, e: &usize) -> teleport::Result<()> {
        let mut queue = self.rows[*e].queue.clone();
        queue.reset(self, None).map_err(|e| tp_err(walk_name(e)))
    }
    fn clear_pulses(&mut self, e: &usize) -> teleport::Result<()> {
        self.emit("clear_pulses", vec![json!(e)]).map_err(tp_err)
    }
    fn lock(&mut self, e: &usize, ticks: i32) -> teleport::Result<()> {
        let locks = self.rows[*e].locks.clone();
        locks
            .lock(ticks, &mut (), self)
            .map_err(|e| tp_err(lock_name(e)))
    }
    fn lock_forever(&mut self, e: &usize) -> teleport::Result<()> {
        let locks = self.rows[*e].locks.clone();
        locks
            .lock_forever(&mut (), self)
            .map_err(|e| tp_err(lock_name(e)))
    }
    fn unlock(&mut self, e: &usize) -> teleport::Result<()> {
        self.rows[*e].locks.unlock();
        Ok(())
    }
    fn set_disabled_ticks(&mut self, e: &usize, ticks: i32) -> teleport::Result<()> {
        // ImpactHandler.setDisabledTicks: disabledTicks = GameWorld.getTicks() + ticks.
        let now = self.read_ticks().map_err(tp_err)?;
        self.rows[*e].disabled_ticks = now.wrapping_add(ticks);
        Ok(())
    }
    fn submit(&mut self, pulse: &TeleportPulse<usize>) -> teleport::Result<()> {
        let index = self.pulse_ix(pulse);
        self.emit("submit", vec![json!(index)]).map_err(tp_err)?;
        self.runner
            .submit(Some(pulse.handle().clone()))
            .map_err(|e| tp_err(pulse_name(&e)))
    }
    fn close_interfaces(&mut self, e: &usize) -> teleport::Result<()> {
        self.emit("close_interfaces", vec![json!(e)])
            .map_err(tp_err)
    }
    fn location(&mut self, e: &usize) -> teleport::Result<Option<Location>> {
        Ok(self.rows[*e].queue.entity().location)
    }
    fn teleport_location(&mut self, e: &usize) -> teleport::Result<Option<Location>> {
        Ok(self.rows[*e].queue.entity().teleport_location)
    }
    fn set_teleport_location(
        &mut self,
        e: &usize,
        location: Option<Location>,
    ) -> teleport::Result<()> {
        self.rows[*e].queue.state.write().entity.teleport_location = location;
        Ok(())
    }
    fn play_global_audio(
        &mut self,
        location: &Location,
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> teleport::Result<()> {
        self.emit(
            "global_audio",
            vec![
                coord(Some(location)),
                json!(id),
                json!(delay),
                json!(loops),
                json!(radius),
            ],
        )
        .map_err(tp_err)
    }
    fn play_audio(
        &mut self,
        e: &usize,
        id: i32,
        delay: i32,
        loops: i32,
        location: Option<&Location>,
        radius: i32,
    ) -> teleport::Result<()> {
        self.emit(
            "audio",
            vec![
                json!(e),
                json!(id),
                json!(delay),
                json!(loops),
                coord(location),
                json!(radius),
            ],
        )
        .map_err(tp_err)
    }
    fn force_animation(&mut self, e: &usize, animation: Animation) -> teleport::Result<()> {
        let animator = self.rows[*e].animator.clone();
        animator
            .force_animation(self, Some(animation))
            .map_err(|e| tp_err(anim_name(e)))
    }
    fn animate(&mut self, e: &usize, animation: Animation) -> teleport::Result<bool> {
        let animator = self.rows[*e].animator.clone();
        animator
            .animate(self, Some(animation), None)
            .map_err(|e| tp_err(anim_name(e)))
    }
    fn graphics(&mut self, e: &usize, graphics: Graphics) -> teleport::Result<bool> {
        let animator = self.rows[*e].animator.clone();
        animator
            .animate(self, None, Some(graphics))
            .map_err(|e| tp_err(anim_name(e)))
    }
    fn animation_reset(&mut self) -> Animation {
        self.factory.animation_reset()
    }
}

impl pulses::Context<()> for Model {
    fn pulse(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        let body = self.by_handle(pulse).ok_or(pulses::Error::Null)?;
        body.pulse(self).map_err(|e| pulse_err(tp_name(e)))
    }
    fn node_active(&mut self, _: pulses::Node, _: &mut ()) -> pulses::Result<bool> {
        unreachable("node checks").map_err(pulse_err)
    }
    fn exception(&mut self, error: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        // Throwable.printStackTrace: recorded, never a failure point.
        self.record("print_stack_trace", vec![json!(pulse_name(error))]);
        Ok(())
    }
    fn start(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<()> {
        let body = self.by_handle(pulse).ok_or(pulses::Error::Null)?;
        body.start(self).map_err(|e| pulse_err(tp_name(e)))
    }
    fn stop(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<()> {
        let body = self.by_handle(pulse).ok_or(pulses::Error::Null)?;
        body.stop(self).map_err(|e| pulse_err(tp_name(e)))
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        unreachable("scripts").map_err(pulse_err)
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        unreachable("scripts").map_err(pulse_err)
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        unreachable("death").map_err(pulse_err)
    }
    fn send_message(&mut self, _: pulses::Node, _: &str, _: &mut ()) -> pulses::Result<()> {
        unreachable("unhandled action").map_err(pulse_err)
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        // Telemetry is outside the corpus: a constant clock keeps it silent.
        Ok(0)
    }
    fn log(&mut self, _: pulses::Level, message: String, _: &mut ()) -> pulses::Result<()> {
        self.emit("log", vec![json!(message)]).map_err(pulse_err)
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn count_pulse(&mut self, _: &str, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
}

impl animation::Definitions for Model {
    fn definition(&mut self, _: &Animation, id: i32) -> Option<defs::Handle> {
        // AnimationDefinition.forId catches everything: recorded, never a failure point.
        self.record("definition", vec![json!(id)]);
        match self.input["durations"].get(id.to_string()) {
            None => Some(Shared::new(defs::Definition::default())),
            Some(Value::String(s)) if s == "missing" => None,
            Some(Value::Null) => Some(Shared::new(defs::Definition::default())),
            Some(Value::Array(frames)) => {
                let definition = defs::Definition {
                    durations: Some(Shared::new(frames.iter().map(num).collect())),
                    ..Default::default()
                };
                Some(Shared::new(definition))
            }
            Some(other) => panic!("unexpected duration {other}"),
        }
    }
}
impl animation::Context<usize> for Model {
    fn world_ticks(&mut self, _: &Animator<usize>) -> animation::Result<i32> {
        self.read_ticks().map_err(anim_err)
    }
    fn clocks(
        &mut self,
        _: &Animator<usize>,
        entity: Option<&usize>,
    ) -> animation::Result<Option<Shared<Vec<i32>>>> {
        Ok(entity.map(|e| self.rows[*e].clocks.clone()))
    }
    fn register_animation(
        &mut self,
        _: &Animator<usize>,
        entity: Option<&usize>,
        animation: Animation,
    ) -> animation::Result<bool> {
        let args = vec![
            json!(entity),
            json!("Animate"),
            json!(animation.id()),
            json!(animation.delay()),
            json!(animation.priority().map(|p| p.name())),
        ];
        self.emit("register", args).map_err(anim_err)?;
        Ok(true)
    }
    fn register_graphics(
        &mut self,
        _: &Animator<usize>,
        entity: Option<&usize>,
        graphics: Graphics,
    ) -> animation::Result<bool> {
        let args = vec![
            json!(entity),
            json!("SpotAnim"),
            json!(graphics.id()),
            json!(graphics.height()),
            json!(graphics.delay()),
        ];
        self.emit("register", args).map_err(anim_err)?;
        Ok(true)
    }
}

impl action_locks::Context<(), (), NodeHandle> for Model {
    fn ticks(&mut self, _: &mut ()) -> action_locks::Result<i32> {
        self.read_ticks().map_err(lock_err)
    }
    fn elapse(
        &mut self,
        _: Option<()>,
        _: Option<NodeHandle>,
        _: &action_locks::Lock<()>,
        _: &ActionLocks<()>,
        _: &mut (),
    ) -> action_locks::Result<()> {
        unreachable("lock elapse").map_err(lock_err)
    }
}

impl walking::Context for Model {
    fn timer(&mut self, queue: &mut walking::Queue, name: &str) -> walking::Result<bool> {
        let e = self.queue_owner(queue);
        self.emit("get_timer", vec![json!(e), json!(name)])
            .map_err(walk_err)?;
        Ok(self.timers(e).contains(&json!(name)))
    }
    fn marathon(&mut self, _: &mut walking::Queue) -> walking::Result<bool> {
        unreachable("marathon").map_err(walk_err)
    }
    fn energy(&mut self, _: &mut walking::Queue, _: f64) -> walking::Result<()> {
        unreachable("energy").map_err(walk_err)
    }
    fn run_toggled(&mut self, _: &mut walking::Queue, _: bool) -> walking::Result<()> {
        unreachable("run toggle").map_err(walk_err)
    }
    fn zone_move(
        &mut self,
        _: &mut walking::Queue,
        _: Location,
        _: Location,
    ) -> walking::Result<bool> {
        unreachable("walking steps").map_err(walk_err)
    }
    fn region_move(
        &mut self,
        queue: &mut walking::Queue,
        from: Option<Location>,
        to: Option<Location>,
    ) -> walking::Result<()> {
        // RegionManager.move: the chunk move is a service, then
        // entity.zoneMonitor.updateLocation(entity.walkingQueue.footPrint).
        let e = self.queue_owner(queue);
        self.emit(
            "region_move",
            vec![json!(e), coord(from.as_ref()), coord(to.as_ref())],
        )
        .map_err(walk_err)?;
        let footprint = queue.foot_print();
        self.emit("zone_update", vec![json!(e), coord(footprint.as_ref())])
            .map_err(walk_err)
    }
    fn clear_pulses(&mut self, _: &mut walking::Queue) -> walking::Result<()> {
        unreachable("walk back").map_err(walk_err)
    }
    fn route_create(
        &mut self,
        _: &mut walking::Queue,
        _: Location,
    ) -> walking::Result<walking::RouteItem> {
        unreachable("route items").map_err(walk_err)
    }
    fn route_add(&mut self, _: &mut walking::Queue, _: walking::RouteItem) -> walking::Result<()> {
        unreachable("route items").map_err(walk_err)
    }
    fn route_remove(
        &mut self,
        _: &mut walking::Queue,
        _: walking::RouteItem,
    ) -> walking::Result<()> {
        unreachable("route items").map_err(walk_err)
    }
    fn null_location(&mut self, _: &mut walking::Queue) -> walking::Result<()> {
        self.emit("log", vec![json!(NULL_LOCATION)])
            .map_err(walk_err)
    }
}

impl zones::Host for Model {
    type Target = ();
    type Opaque = ();
    fn kind(&mut self, entity: &NodeHandle) -> zones::Result<EntityKind> {
        Ok(if self.rows[self.slot(entity)].player {
            EntityKind::Player
        } else {
            EntityKind::Npc
        })
    }
    fn monitor(&mut self, entity: &NodeHandle) -> zones::Result<Monitor<Self>> {
        Ok(self.rows[self.slot(entity)].monitor.clone())
    }
    fn region(&mut self, _: i32) -> zones::Result<rs09_server::world::region::Handle> {
        unreachable("regions").map_err(zone_err)
    }
    fn target_entity(&mut self, _: &()) -> zones::Result<Option<NodeHandle>> {
        unreachable("targets").map_err(zone_err)
    }
    fn location(&mut self, _: &NodeHandle) -> zones::Result<Option<Location>> {
        unreachable("location").map_err(zone_err)
    }
    fn is_artificial(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unreachable("artificial").map_err(zone_err)
    }
    fn entity_update_location(
        &mut self,
        _: &NodeHandle,
        _: Option<&Location>,
    ) -> zones::Result<()> {
        unreachable("update location").map_err(zone_err)
    }
    fn set_invisible(&mut self, _: &NodeHandle, _: bool) -> zones::Result<()> {
        unreachable("invisible").map_err(zone_err)
    }
    fn entity_continue_attack(
        &mut self,
        _: &NodeHandle,
        _: &NodeHandle,
        _: Option<&()>,
        _: bool,
    ) -> zones::Result<bool> {
        unreachable("combat").map_err(zone_err)
    }
    fn skull_wilderness(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unreachable("skull").map_err(zone_err)
    }
    fn skull_level(&mut self, player: &NodeHandle) -> zones::Result<i32> {
        let e = self.slot(player);
        self.emit("skull_level", vec![json!(e)]).map_err(zone_err)?;
        Ok(self.rows[e].spec.get("skull").map_or(0, num))
    }
    fn packet_message(&mut self, _: &NodeHandle, _: &str) -> zones::Result<()> {
        unreachable("packet message").map_err(zone_err)
    }
    fn player_message(&mut self, player: &NodeHandle, message: &str) -> zones::Result<()> {
        let e = self.slot(player);
        self.emit("message", vec![json!(e), json!(message)])
            .map_err(zone_err)
    }
    fn has_timer(&mut self, entity: &NodeHandle, name: &str) -> zones::Result<bool> {
        let e = self.slot(entity);
        self.emit("get_timer", vec![json!(e), json!(name)])
            .map_err(zone_err)?;
        Ok(self.timers(e).contains(&json!(name)))
    }
    fn teleport_locked(&mut self, entity: &NodeHandle) -> zones::Result<bool> {
        let locks = self.rows[self.slot(entity)].locks.clone();
        locks
            .is_kind_locked(LockKind::Teleport, &mut (), self)
            .map_err(|e| zone_err(lock_name(e)))
    }
    fn target_item_id(&mut self, target: Option<&()>) -> zones::Result<i32> {
        // node.asItem().getId() on the null node TeleportManager passes.
        match target {
            None => Err(zones::Error::Null),
            Some(_) => unreachable("items").map_err(zone_err),
        }
    }
    fn now_millis(&mut self) -> zones::Result<i64> {
        unreachable("clock").map_err(zone_err)
    }
    fn multi_zone(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unreachable("multi").map_err(zone_err)
    }
    fn entity_ignore_multi(&mut self, _: &NodeHandle, _: &NodeHandle) -> zones::Result<bool> {
        unreachable("multi").map_err(zone_err)
    }
    fn familiar_owner(&mut self, _: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        unreachable("familiar").map_err(zone_err)
    }
    fn combat_attacker(&mut self, _: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        unreachable("combat").map_err(zone_err)
    }
    fn combat_time(&mut self, _: &NodeHandle) -> zones::Result<Option<i64>> {
        unreachable("combat").map_err(zone_err)
    }
    fn active(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unreachable("active").map_err(zone_err)
    }
    fn id(&mut self, _: &NodeHandle) -> zones::Result<i32> {
        unreachable("id").map_err(zone_err)
    }
    fn music_playing(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unreachable("music").map_err(zone_err)
    }
    fn music_play_default(&mut self, _: &NodeHandle) -> zones::Result<()> {
        unreachable("music").map_err(zone_err)
    }
    fn music_unlock(&mut self, _: &NodeHandle, _: i32, _: Option<bool>) -> zones::Result<()> {
        unreachable("music").map_err(zone_err)
    }
    fn random_next_int(&mut self, _: i32) -> zones::Result<i32> {
        unreachable("random").map_err(zone_err)
    }
    fn fresh_random_next_int(&mut self, _: i32) -> zones::Result<i32> {
        unreachable("random").map_err(zone_err)
    }
    fn teleport_permitted(&mut self, _: &Location) -> zones::Result<bool> {
        unreachable("teleport permitted").map_err(zone_err)
    }
}

/// Replays one case and returns its expected-shaped record.
pub fn evaluate(input: &Value) -> Value {
    let mut state = State::default();
    let factory = animation::Factory::default();
    let mut model = Model {
        input: input.clone(),
        _state: State::default(),
        rows: Vec::new(),
        dests: input["dests"]
            .as_array()
            .unwrap()
            .iter()
            .map(place)
            .collect(),
        pulses: Vec::new(),
        runner: pulses::Runner::default(),
        zones: Vec::new(),
        answers: Vec::new(),
        factory: factory.clone(),
        events: Vec::new(),
        calls: 0,
        fail_at: -1,
        counts: HashMap::new(),
        fired: BTreeSet::new(),
        ticks: input["ticks"].as_i64().unwrap_or(0) as i32,
    };
    for (index, zone) in input["zones"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let flags = zone["restriction"].as_i64().unwrap_or(0) as i32;
        let restrictions: Vec<Restriction> = Restriction::ALL
            .into_iter()
            .filter(|r| flags & r.flag() != 0)
            .collect();
        model
            .zones
            .push(Zone::new(None, false, &restrictions, Scripted(index)));
        model.answers.push(
            zone["answers"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|b| b.as_bool().unwrap())
                .collect(),
        );
    }
    for (index, spec) in input["entities"].as_array().unwrap().iter().enumerate() {
        let player = spec["kind"] != "npc";
        let location = place(&spec["loc"]);
        let node = state.add_npc_at(index as i32 + 1, None, [1, 2, 3, 4]);
        let handle = state.retain_node(node).unwrap();
        let mut entity = walking::Entity::new(player, location);
        if player {
            entity.last_scene = place(&spec["last"]);
        }
        let mut attributes = GameAttributes::default();
        if let Some(attrs) = spec["attrs"].as_object() {
            for (key, v) in attrs {
                attributes.set(key, value(v));
            }
        }
        let monitor = Monitor::new(handle.clone());
        for z in spec["zones"].as_array().cloned().unwrap_or_default() {
            monitor.zones().add(RegionZone::new(
                model.zones[num(&z) as usize].clone(),
                Borders::new(0, 0, 0, 0),
            ));
        }
        model.rows.push(Row {
            player,
            spec: spec.as_object().unwrap().clone(),
            handle,
            attributes,
            locks: ActionLocks::default(),
            animator: Animator::new(Some(index), factory.clone()),
            clocks: Shared::new(vec![0; 10]),
            queue: walking::Queue::new(entity),
            scripts: scripts::Processor::with_entity(node),
            disabled_ticks: 0,
            monitor,
            manager: Manager::new(index),
        });
    }
    model._state = state;
    let mut steps = Vec::new();
    for op in input["ops"].as_array().unwrap() {
        model.events = Vec::new();
        model.calls = 0;
        model.fail_at = op["failAt"].as_i64().unwrap_or(-1);
        let outcome = model.run(op);
        model.fail_at = -1;
        let (result, error) = match outcome {
            Ok(value) => (value, Value::Null),
            Err(name) => (Value::Null, json!(name)),
        };
        let calls = model.calls;
        let events = std::mem::take(&mut model.events);
        let states: Vec<Value> = (0..model.rows.len()).map(|e| model.snapshot(e)).collect();
        let pulses: Vec<Value> = model
            .pulses
            .iter()
            .map(|p| {
                let handle = p.handle().read();
                json!([
                    handle.class_name,
                    handle.running,
                    handle.ticks_passed,
                    handle.delay,
                    p.counter()
                ])
            })
            .collect();
        let queue: Vec<Value> = model
            .runner
            .current()
            .iter()
            .map(|h| {
                json!(
                    model
                        .pulses
                        .iter()
                        .position(|p| p.handle().same(h))
                        .unwrap()
                )
            })
            .collect();
        steps.push(json!({
            "result": result,
            "error": error,
            "calls": calls,
            "events": events,
            "state": states,
            "pulses": pulses,
            "queue": queue,
            "ticks": model.ticks,
        }));
    }
    Value::Array(steps)
}
