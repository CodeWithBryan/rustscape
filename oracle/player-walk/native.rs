use rs09_protocol::game::actions::ClientAction;
use rs09_server::{
    definitions::{item::Shared, scenery::SceneryDefinition},
    pulses,
    world::{
        doors::{Location, Scenery},
        interaction::{Approach, ApproachKind, OptionData, OptionHandle},
        movement::DestinationService,
        player_walk::{Error, Event, Node, Query, Result, Scripts, Services, State},
        walk_request::Interfaces,
    },
};
use serde_json::{Value, json};
#[path = "../ground-target/native.rs"]
pub mod ground_target;
use std::collections::HashMap;
fn n(v: &Value, key: &str, d: i32) -> i32 {
    v[key].as_i64().map_or(d, |n| n as i32)
}
fn handler(v: &Value) -> Option<usize> {
    if v.get("handler").is_some_and(Value::is_null) {
        None
    } else {
        Some(n(v, "handler", 1) as usize)
    }
}
fn input_location(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(l: Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
fn bits(v: f64) -> String {
    format!("{:x}", v.to_bits())
}
pub struct Model {
    input: Value,
    pub events: Vec<Value>,
    calls: HashMap<String, i32>,
    pub player: Node,
    pub scenery: Option<rs09_server::world::player_walk::NodeHandle>,
    pub option: Option<OptionHandle>,
    ground: ground_target::Extra,
}
impl Model {
    fn emit(&mut self, state: &mut State, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(state, a)?;
                }
            }
        }
        Ok(())
    }
    pub fn act(&mut self, state: &mut State, a: &Value) -> Result<()> {
        let player = self.player;
        match a["op"].as_str().unwrap() {
            "approach" => {
                let approach = Approach {
                    player,
                    target: self
                        .ground
                        .target
                        .as_ref()
                        .or(self.scenery.as_ref())
                        .map(|n| n.node),
                    option: self.option.clone(),
                    kind: ApproachKind::Option,
                };
                let p = state.context(self).interaction_motion(approach)?;
                state.context(self).run_motion(player, p)
            }
            "scenerySet" => {
                let h = state.scenery(self.scenery.as_ref().unwrap().node)?;
                let mut h = h.write();
                if let Some(location) = a["location"].as_array() {
                    h.location = Location::new(
                        location[0].as_i64().unwrap() as i32,
                        location[1].as_i64().unwrap() as i32,
                        location[2].as_i64().unwrap() as i32,
                    );
                }
                if a.get("active").is_some() {
                    h.active = a["active"] == true;
                }
                if a.get("shape").is_some() {
                    h.shape = n(a, "shape", 10);
                }
                if a.get("rotation").is_some() {
                    h.rotation = n(a, "rotation", 0);
                }
                if a.get("walking").is_some() {
                    h.definition.as_ref().unwrap().write().walking_flag = n(a, "walking", 0);
                }
                Ok(())
            }
            "optionReplace" => {
                let mut o = OptionData::new(
                    Some(a["name"].as_str().unwrap_or("Open").into()),
                    n(a, "index", 0),
                );
                o.handler = handler(a);
                self.option = Some(Shared::new(o));
                Ok(())
            }
            "optionHandler" => {
                self.option.as_ref().unwrap().write().handler = handler(a);
                Ok(())
            }
            "mutatePulse" => {
                let p = state.current(player)?.ok_or(Error::Null)?;
                let mut fields = p.state.write();
                fields.mover = None;
                fields.force_run = false;
                fields.base.write().checks = None;
                Ok(())
            }
            "arrival" => {
                let mut p = state.current(player)?.ok_or(Error::Null)?;
                rs09_server::world::movement::Context::pulse(&mut state.context(self), &mut p)
                    .map_err(Error::Movement)?;
                Ok(())
            }
            "fail" => Err(Error::Binding("oracle".into())),
            "request" => {
                let dest_x = n(a, "x", 3210);
                let dest_y = n(a, "y", 3216);
                let is_run = a["running"].as_bool().unwrap_or(false);
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
                state.context(self).process(player, &action)?;
                Ok(())
            }
            "tick" => {
                state.context(self).pulses()?;
                state.context(self).entity(player)?;
                state.ticks = state.ticks.wrapping_add(1);
                Ok(())
            }
            _ => ground_target::act(self, state, a),
        }
    }
    fn name(state: &State, n: Node) -> Value {
        match state.actor(n) {
            Ok(a) => json!(a.wire_index.to_string()),
            Err(_) => match state.ground(n) {
                Ok(item) => json!(item.read().item.read().name),
                Err(_) => json!(state.scenery(n).unwrap().read().name),
            },
        }
    }
}
impl Services for Model {
    fn destination(
        &mut self,
        state: &mut State,
        mover: Option<Node>,
        target: Option<Node>,
        service: DestinationService,
    ) -> Result<Option<Location>> {
        let (name, token) = match service {
            DestinationService::Option(t) => ("option", t),
            DestinationService::UseWith(t) => ("use", t),
            _ => return Err(Error::Unsupported("oracle destination")),
        };
        self.emit(
            state,
            "destination",
            vec![
                json!(name),
                json!(token),
                mover.map_or(Value::Null, |n| Self::name(state, n)),
                target.map_or(Value::Null, |n| Self::name(state, n)),
            ],
        )?;
        Ok(input_location(&self.input["optionDestination"]))
    }
    fn door_destination(
        &mut self,
        state: &mut State,
        mover: Option<Node>,
        target: Option<Node>,
    ) -> Result<Option<Location>> {
        self.emit(
            state,
            "doorDestination",
            vec![
                mover.map_or(Value::Null, |n| Self::name(state, n)),
                target.map_or(Value::Null, |n| Self::name(state, n)),
            ],
        )?;
        Ok(if self.input.get("doorDestination").is_some() {
            input_location(&self.input["doorDestination"])
        } else {
            let [x, y, z] = state
                .scenery(target.ok_or(Error::Null)?)?
                .read()
                .location
                .coordinates();
            Some(Location::new(x + 1, y, z))
        })
    }
    fn teleport_permitted(
        &mut self,
        state: &mut State,
        location: Option<Location>,
    ) -> Result<bool> {
        self.emit(state, "permitted", vec![coord(location)])?;
        Ok(self.input["permitted"] != false)
    }
    fn interaction_arrival(&mut self, state: &mut State, approach: &Approach) -> Result<bool> {
        let option = approach.option.as_ref().ok_or(Error::Null)?.read();
        let name = option.name();
        let index = option.index();
        let handler = option.handler;
        drop(option);
        self.emit(
            state,
            "interactionArrival",
            vec![
                Self::name(state, approach.player),
                approach
                    .target
                    .map_or(Value::Null, |n| Self::name(state, n)),
                json!(name),
                json!(index),
                json!(handler),
            ],
        )?;
        Ok(true)
    }
    fn query(&mut self, state: &mut State, q: Query<'_>) -> Result<bool> {
        match q {
            Query::Locked(_) => {
                self.emit(state, "locked", vec![])?;
                Ok(self.input["locked"] == true)
            }
            Query::CloseDialogue(n) => {
                self.emit(state, "dialogue", vec![Self::name(state, n)])?;
                Ok(true)
            }
            Query::CloseInterface(_) => Err(Error::Unsupported("fixture has no interface")),
            Query::Timer(_, s) => {
                self.emit(state, "timer", vec![json!(s)])?;
                Ok(self.input[s] == true)
            }
            Query::Marathon(_) => {
                self.emit(state, "marathon", vec![])?;
                Ok(false)
            }
            Query::ZoneMove(_, a, b) => {
                self.emit(state, "zone", vec![coord(Some(a)), coord(Some(b))])?;
                let i = self.calls["zone"] as usize - 1;
                Ok(self.input["zoneAnswers"]
                    .get(i)
                    .and_then(Value::as_bool)
                    .unwrap_or(true))
            }
        }
    }
    fn event(&mut self, state: &mut State, event: Event<'_>) -> Result<()> {
        let (event, args) = match event {
            Event::CloseChatbox(_) => return Err(Error::Unsupported("fixture has no chatbox")),
            Event::Debug(n, s) => ("debug", vec![Self::name(state, n), json!(s)]),
            Event::Minimap(n) => ("minimap", vec![Self::name(state, n)]),
            Event::Face(n, t) => (
                "face",
                vec![
                    Self::name(state, n),
                    t.map_or(Value::Null, |n| Self::name(state, n)),
                ],
            ),
            Event::FaceLocationNone(n) => ("faceLocation", vec![Self::name(state, n)]),
            Event::Scripts(n, s) => match s {
                Scripts::Reset => ("resetScripts", vec![Self::name(state, n)]),
                Scripts::Weak => ("weak", vec![Self::name(state, n)]),
                Scripts::Normal => ("normal", vec![Self::name(state, n)]),
                Scripts::Pre => ("pre", vec![Self::name(state, n)]),
                Scripts::Post(b) => ("post", vec![Self::name(state, n), json!(b)]),
            },
            Event::Message(n, s) => ("message", vec![Self::name(state, n), json!(s)]),
            Event::Energy(_, d) => ("energy", vec![json!(bits(d))]),
            Event::RunToggled(_, b) => ("toggle", vec![json!(b)]),
            Event::RegionMove(_, a, b) => ("move", vec![coord(a), coord(b)]),
            Event::TickEvent(n, t) => ("tickEvent", vec![Self::name(state, n), json!(t)]),
            Event::Skills(_) => ("skills", vec![]),
            Event::Timers(_) => ("timers", vec![]),
            Event::Masks(n) => ("masks", vec![Self::name(state, n)]),
            Event::Exception(text) => (
                "exception",
                vec![json!(if text.contains("Null") {
                    "Null"
                } else {
                    "Binding"
                })],
            ),
            Event::Log(l, s) => (
                "log",
                vec![
                    json!(match l {
                        pulses::Level::Warn => "WARN",
                        pulses::Level::Error => "ERR",
                    }),
                    json!(s),
                ],
            ),
            Event::PulseLength(s, t) => ("length", vec![json!(s), json!(t)]),
            Event::CountPulse(s) => ("count", vec![json!(s)]),
            Event::BotTotal(_) | Event::OtherTotal(_) => {
                return Err(Error::Unsupported("metrics disabled"));
            }
        };
        self.emit(state, event, args)
    }
    fn interfaces(&self, _: &State, _: Node) -> Result<Interfaces> {
        Ok(Interfaces::default())
    }
    fn region(&mut self, state: &mut State, l: &Location) -> Result<Option<bool>> {
        self.emit(state, "region", vec![json!(l.coordinates())])?;
        Ok(Some(true))
    }
    fn clipping(&mut self, state: &mut State, z: i32, x: i32, y: i32) -> Result<i32> {
        self.emit(state, "clip", vec![json!(z), json!(x), json!(y)])?;
        let mut f = 0;
        for row in self.input["flags"].as_array().into_iter().flatten() {
            if row[0] == x && row[1] == y {
                f = row[2].as_i64().unwrap() as i32;
            }
        }
        Ok(f)
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
}
pub fn create(input: &Value) -> (State, Model) {
    let mut state = State::default();
    state.bfs = input["bfs"] == true;
    let player = state.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    state
        .actor(player)
        .unwrap()
        .queue
        .state
        .write()
        .entity
        .energy = input["energy"].as_f64().unwrap_or(100.0);
    let (scenery, option) = if let Some(s) = input.get("scenery") {
        let rotation = n(s, "rotation", 0);
        let mut object = Scenery::new(
            12348,
            input_location(&s["location"]).unwrap_or(Location::new(3211, 3217, 0)),
            n(s, "shape", 10),
            rotation,
            Some("object".into()),
            false,
        )
        .unwrap();
        let size = s["size"]
            .as_array()
            .cloned()
            .unwrap_or(vec![json!(1), json!(1)]);
        object.node_size = size[0].as_i64().unwrap() as i32;
        object.definition = Some(Shared::new(SceneryDefinition {
            size_x: size[0].as_i64().unwrap() as i32,
            size_y: size[1].as_i64().unwrap() as i32,
            walking_flag: n(s, "walking", 0),
            ..SceneryDefinition::default()
        }));
        if s.get("active").is_some() {
            object.active = s["active"] == true;
        }
        let target = state.scenery_target(Shared::new(object)).unwrap();
        let mut option = OptionData::new(Some("Open".into()), 0);
        option.handler = handler(input);
        (Some(target), Some(Shared::new(option)))
    } else {
        (None, None)
    };
    let mut model = Model {
        input: input.clone(),
        events: vec![],
        calls: HashMap::new(),
        player,
        scenery,
        option,
        ground: ground_target::Extra::default(),
    };
    if input.get("ground").is_some() {
        ground_target::initialize(&mut model, &mut state);
    }
    (state, model)
}
fn snapshot(state: &State, m: &Model, error: Option<&str>) -> Value {
    let actor = state.actor(m.player).unwrap();
    let q = &actor.queue;
    let entity = q.entity();
    let current=state.current(m.player).unwrap().map(|p|json!({"running":p.base().read().running,"last":coord(p.last()),"interact":coord(p.interact()),"near":p.near(),"force":p.force_run()}));
    let mut value = json!({"error":error,"events":m.events,"ticks":state.ticks,"location":coord(entity.location),"foot":coord(q.foot_print()),"direction":entity.direction.unwrap().code(),"running":q.running(),"walk":q.walk_dir(),"run":q.run_dir(),"energy":bits(entity.energy),"toggled":entity.run_toggled,"points":q.points().read().iter().map(|p|{let p=p.read();[p.x(),p.y()]}).collect::<Vec<_>>(),"current":current,"managed":actor.manager.current().map(|p|p.read().running),"pending":state.pending().iter().map(|p|p.read().running).collect::<Vec<_>>()});
    if let Some(target) = &m.scenery {
        let object = state.scenery(target.node).unwrap();
        let o = object.read();
        let d = o.definition.as_ref().unwrap().read();
        value["scenery"] = json!([
            o.location.coordinates(),
            o.shape,
            o.rotation,
            o.active,
            o.node_size,
            d.size_x,
            d.size_y,
            d.walking_flag
        ]);
        let option = m.option.as_ref().unwrap().read();
        value["option"] = json!([option.name(), option.index(), option.handler]);
    }
    if m.input.get("ground").is_some() {
        ground_target::snapshot(m, state, &mut value);
    }
    value
}
pub fn evaluate(input: &Value) -> Value {
    let (mut state, mut m) = create(input);
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let error = m.act(&mut state, a).err().map(|_| "Binding");
        state.collect();
        out.push(snapshot(&state, &m, error));
    }
    json!(out)
}
