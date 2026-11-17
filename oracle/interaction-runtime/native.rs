use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::actions::ClientAction;
use rs09_server::{
    pulses,
    world::{
        doors::Location,
        interaction::{Approach, OptionHandle},
        legacy_interaction::{self as legacy, Task},
        legacy_runtime::{self, Bindings},
        movement::DestinationService,
        player_walk::{self as walk, Event, Node, Query, Services, State},
        walk_request::Interfaces,
    },
};
use serde_json::{Value, json};
use std::collections::HashMap;
#[allow(dead_code)]
mod walking {
    include!("../player-walk/native.rs");
    pub fn read_snapshot(state: &State, model: &Model, error: Option<&str>) -> Value {
        snapshot(state, model, error)
    }
}
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |v| v as i32)
}
fn coord(l: Option<Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l.coordinates()))
}
fn lerror(e: walk::Error) -> legacy::Error {
    match e {
        walk::Error::Null => legacy::Error::Null,
        _ => legacy::Error::Binding(None),
    }
}
fn werror(e: legacy::Error) -> walk::Error {
    match e {
        legacy::Error::Null => walk::Error::Null,
        _ => walk::Error::Binding("oracle".into()),
    }
}
fn name(state: &State, node: Option<Node>) -> Value {
    node.map_or(Value::Null, |n| {
        state
            .actor(n)
            .map(|a| json!(a.wire_index.to_string()))
            .unwrap_or_else(|_| {
                state
                    .scenery(n)
                    .map_or(Value::Null, |s| json!(s.read().name))
            })
    })
}
pub struct Model {
    pub base: walking::Model,
    input: Value,
    calls: HashMap<String, i32>,
    face: Option<Location>,
}
impl Model {
    fn emit(&mut self, state: &mut State, event: &str, args: Vec<Value>) -> legacy::Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.base.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["interactionHooks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for action in hook["actions"].as_array().unwrap() {
                    self.act(state, action)?;
                }
            }
        }
        Ok(())
    }
    fn flag(&self, handler: usize, key: &str, default: bool) -> bool {
        let c = if handler == 1 {
            &self.input["handlerConfig"]
        } else {
            &self.input["handlers"][handler.to_string()]
        };
        c[key].as_bool().unwrap_or(default)
    }
    pub fn act(&mut self, state: &mut State, a: &Value) -> legacy::Result<()> {
        let player = self.base.player;
        match a["op"].as_str().unwrap() {
            "handle" => legacy_runtime::handle(
                state,
                self,
                self.base.scenery.as_ref().map(|n| n.node),
                Some(player),
                self.base.option.clone(),
            ),
            "invalid" => legacy_runtime::invalid(
                state,
                self,
                self.base.scenery.as_ref().map(|n| n.node),
                Some(player),
                self.base.option.clone(),
            ),
            "set" => {
                for (k, v) in a.as_object().unwrap() {
                    if k != "op" {
                        self.input[k] = v.clone();
                    }
                }
                Ok(())
            }
            "fail" => Err(legacy::Error::Binding(None)),
            "managerReplace" => {
                state.actor_mut(player).map_err(lerror)?.manager =
                    pulses::Manager::new([1, 2, 3, 4]);
                Ok(())
            }
            "sceneryReplace" => {
                let old = state
                    .scenery(self.base.scenery.as_ref().unwrap().node)
                    .map_err(lerror)?;
                let old = old.read();
                let mut object = rs09_server::world::doors::Scenery::new(
                    old.id,
                    old.location.clone(),
                    old.shape,
                    old.rotation,
                    Some("replacement".into()),
                    old.close,
                )
                .unwrap();
                object.node_size = old.node_size;
                object.direction = old.direction;
                object.active = old.active;
                object.definition = old.definition.clone();
                drop(old);
                self.base.scenery = Some(
                    state
                        .scenery_target(rs09_server::definitions::item::Shared::new(object))
                        .map_err(lerror)?,
                );
                Ok(())
            }
            "checksClear" => {
                if let Some(p) = state.actor(player).map_err(lerror)?.manager.current() {
                    p.write().checks = None;
                }
                Ok(())
            }
            "tick" => {
                state.context(self).pulses().map_err(lerror)?;
                state.context(self).entity(player).map_err(lerror)?;
                state.ticks = state.ticks.wrapping_add(1);
                Ok(())
            }
            "request" => {
                let dest_x = n(a, "x", 3210);
                let dest_y = n(a, "y", 3216);
                let is_run = a["running"] == true;
                let action = match a["kind"].as_str().unwrap_or("world") {
                    "interact" => ClientAction::InteractWalk {
                        dest_x,
                        dest_y,
                        is_run,
                    },
                    "minimap" => ClientAction::MinimapWalk {
                        dest_x,
                        dest_y,
                        is_run,
                        clicked_x: 0,
                        clicked_y: 0,
                        rotation: 0,
                    },
                    _ => ClientAction::WorldspaceWalk {
                        dest_x,
                        dest_y,
                        is_run,
                    },
                };
                state
                    .context(self)
                    .process(player, &action)
                    .map_err(lerror)?;
                Ok(())
            }
            _ => self.base.act(state, a).map_err(lerror),
        }
    }
    pub fn snapshot(&self, state: &State, error: Option<&str>) -> Value {
        let mut v = walking::read_snapshot(state, &self.base, error);
        v["faceLocation"] = coord(self.face.clone());
        v["managedClass"] = state
            .actor(self.base.player)
            .unwrap()
            .manager
            .current()
            .map_or(Value::Null, |p| json!(p.read().class_name));
        v["pendingClasses"] = json!(
            state
                .pending()
                .iter()
                .map(|p| p.read().class_name.clone())
                .collect::<Vec<_>>()
        );
        v["interactionLocked"] = json!(self.input["interactionLocked"] == true);
        v["interactionZone"] = json!(self.input["interactionZone"] == true);
        v
    }
}
impl Services for Model {
    fn query(&mut self, s: &mut State, q: Query<'_>) -> walk::Result<bool> {
        self.base.query(s, q)
    }
    fn event(&mut self, s: &mut State, e: Event<'_>) -> walk::Result<()> {
        if let Event::Exception(text) = e {
            return self
                .emit(
                    s,
                    "exception",
                    vec![json!(if text.contains("Null") {
                        "Null"
                    } else {
                        "Binding"
                    })],
                )
                .map_err(werror);
        }
        self.base.event(s, e)
    }
    fn interfaces(&self, s: &State, n: Node) -> walk::Result<Interfaces> {
        self.base.interfaces(s, n)
    }
    fn region(&mut self, s: &mut State, l: &Location) -> walk::Result<Option<bool>> {
        self.base.region(s, l)
    }
    fn clipping(&mut self, s: &mut State, z: i32, x: i32, y: i32) -> walk::Result<i32> {
        self.base.clipping(s, z, x, y)
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
    fn destination(
        &mut self,
        s: &mut State,
        m: Option<Node>,
        t: Option<Node>,
        d: DestinationService,
    ) -> walk::Result<Option<Location>> {
        self.base.destination(s, m, t, d)
    }
    fn door_destination(
        &mut self,
        s: &mut State,
        m: Option<Node>,
        t: Option<Node>,
    ) -> walk::Result<Option<Location>> {
        self.base.door_destination(s, m, t)
    }
    fn teleport_permitted(&mut self, s: &mut State, l: Option<Location>) -> walk::Result<bool> {
        self.base.teleport_permitted(s, l)
    }
    fn interaction_arrival(&mut self, s: &mut State, a: &Approach) -> walk::Result<bool> {
        legacy_runtime::arrival(s, self, &Task::from(a)).map_err(werror)
    }
    fn delayed_interaction(&mut self, s: &mut State, t: &Task) -> walk::Result<bool> {
        legacy_runtime::arrival(s, self, t).map_err(werror)
    }
}
impl Bindings for Model {
    fn interaction_locked(&mut self, s: &mut State, _: Option<Node>) -> legacy::Result<bool> {
        self.emit(s, "interactionLocked", vec![])?;
        Ok(self.input["interactionLocked"] == true)
    }
    fn handler_walk(&mut self, s: &mut State, h: usize) -> legacy::Result<bool> {
        self.emit(s, "handlerWalk", vec![json!(h)])?;
        Ok(self.flag(h, "walk", true))
    }
    fn handler_walk_for(
        &mut self,
        s: &mut State,
        h: usize,
        _: Option<Node>,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        self.emit(s, "handlerWalkFor", vec![json!(h)])?;
        Ok(self.flag(h, "walkFor", false))
    }
    fn handler_delayed(
        &mut self,
        s: &mut State,
        h: usize,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        self.emit(s, "handlerDelayed", vec![json!(h)])?;
        Ok(self.flag(h, "delayed", true))
    }
    fn handler_name(&mut self, _: &mut State, _: usize) -> legacy::Result<String> {
        Ok("Handler".into())
    }
    fn handler_handle(
        &mut self,
        s: &mut State,
        h: usize,
        p: Option<Node>,
        t: Option<Node>,
        o: &str,
    ) -> legacy::Result<bool> {
        self.emit(
            s,
            "handlerHandle",
            vec![json!(h), name(s, p), name(s, t), json!(o)],
        )?;
        Ok(self.flag(h, "handled", true))
    }
    fn lowercase(&self, s: &str) -> String {
        lowercase_root(s)
    }
    fn zone(
        &mut self,
        s: &mut State,
        _: Option<Node>,
        t: Option<Node>,
        o: Option<&OptionHandle>,
    ) -> legacy::Result<bool> {
        let (n, i, h) = o.map_or((Value::Null, Value::Null, Value::Null), |o| {
            let o = o.read();
            (json!(o.name()), json!(o.index()), json!(o.handler))
        });
        self.emit(s, "interactionZone", vec![name(s, t), n, i, h])?;
        Ok(self.input["interactionZone"] == true)
    }
    fn combat_stop(&mut self, s: &mut State, _: Option<Node>) -> legacy::Result<()> {
        self.emit(s, "combatStop", vec![])
    }
    fn face_player(
        &mut self,
        s: &mut State,
        _: Option<Node>,
        l: Option<Location>,
    ) -> legacy::Result<()> {
        self.face = l.clone();
        self.emit(s, "facePlayer", vec![coord(l)])
    }
    fn dispatch(
        &mut self,
        s: &mut State,
        _: Option<Node>,
        t: Option<Node>,
        o: &str,
    ) -> legacy::Result<()> {
        self.emit(s, "interactionDispatch", vec![name(s, t), json!(o)])
    }
    fn exception(&mut self, s: &mut State, e: &legacy::Error) -> legacy::Result<()> {
        self.emit(
            s,
            "exception",
            vec![json!(if matches!(e, legacy::Error::Null) {
                "Null"
            } else {
                "Binding"
            })],
        )
    }
    fn log(&mut self, s: &mut State, _: &str, message: &str) -> legacy::Result<()> {
        self.event(s, Event::Log(pulses::Level::Error, message))
            .map_err(lerror)
    }
}
pub fn create(input: &Value) -> (State, Model) {
    let (state, base) = walking::create(input);
    (
        state,
        Model {
            base,
            input: input.clone(),
            calls: HashMap::new(),
            face: None,
        },
    )
}
pub fn evaluate(input: &Value) -> Value {
    let (mut state, mut model) = create(input);
    let mut result = vec![];
    for action in input["actions"].as_array().unwrap() {
        model.base.events.clear();
        let error = model.act(&mut state, action).err().map(|_| "Binding");
        state.collect();
        result.push(model.snapshot(&state, error));
    }
    json!(result)
}
