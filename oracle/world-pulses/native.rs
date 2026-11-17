use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::{
        doors::{Handle, Location},
        interaction::{Approach, OptionHandle},
        legacy_interaction::{self as legacy, Task},
        legacy_runtime::{self, Bindings},
        movement::DestinationService,
        player_walk::{self as walk, Event, Node, Query, Services, State},
        scene::{self, Scene},
        scenery::{self, ReplaceMode},
        walk_request::Interfaces,
    },
};
use serde_json::{Value, json};
#[allow(dead_code)]
#[path = "../interaction-runtime/native.rs"]
mod actors;
#[allow(dead_code)]
#[path = "../scene/native.rs"]
mod maps;
pub use maps::Bindings as SceneBindings;
type Map = Scene<maps::MemoryAssets>;
fn err(_: impl std::fmt::Debug) -> legacy::Error {
    legacy::Error::Binding(None)
}
fn werr(e: legacy::Error) -> walk::Error {
    match e {
        legacy::Error::Null => walk::Error::Null,
        _ => walk::Error::Binding("oracle".into()),
    }
}
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |v| v as i32)
}
fn object(h: &Handle) -> Value {
    let h = h.read();
    json!([
        h.id,
        h.shape,
        h.rotation,
        h.location.coordinates(),
        h.active,
        h.constructed,
        h.replaced.as_ref().map(|p| p.read().id)
    ])
}
pub struct Core {
    pub actor: actors::Model,
    input: Value,
    closed: Handle,
    expiry: Option<Handle>,
    held: Vec<Handle>,
    calls: usize,
}
pub struct Model {
    pub scene: Map,
    pub core: Core,
}
struct Joined<'a> {
    scene: &'a mut Map,
    core: &'a mut Core,
}
struct MapServices<'a> {
    state: &'a mut State,
    core: &'a mut Core,
}
macro_rules! unbound{($name:ident($($arg:ident:$t:ty),* $(,)?)->$r:ty)=>{fn $name(&mut self,$($arg:$t),*)->scene::Result<$r>{Err(scene::Error(concat!("outside mixed scheduler scope: ",stringify!($name)).into()))}};}
impl scene::Services<maps::MemoryAssets> for MapServices<'_> {
    fn is_player(&self, _: usize) -> bool {
        true
    }
    unbound!(location(_s: &mut Map, _n: usize) -> Location);
    unbound!(lock(_s: &mut Map, _n: usize, _t: i32) -> ());
    unbound!(audio(_s: &mut Map, _n: usize, _i: i32, _d: Option<i32>) -> ());
    unbound!(message(_s: &mut Map, _n: usize, _t: &str) -> ());
    unbound!(logout_listener(_s: &mut Map, _n: usize, _l: Option<Location>) -> ());
    unbound!(reset_path(_s: &mut Map, _n: usize) -> ());
    unbound!(add_path(_s: &mut Map, _n: usize, _x: i32, _y: i32) -> ());
    unbound!(move_near(_s: &mut Map, _n: usize, _l: Location) -> bool);
    unbound!(
        diary(
            _s: &mut Map,
            _n: usize,
            _d: rs09_server::world::doors::Diary,
            _l: i32,
            _t: i32,
        ) -> ()
    );
    unbound!(pulse(_s: &mut Map, _p: &pulses::Handle) -> bool);
    unbound!(stop(_s: &mut Map, _p: &pulses::Handle) -> ());
    fn ground_create(&mut self, scene: &mut Map, item: Option<usize>) -> scene::Result<()> {
        self.core.actor.base.events.push(json!(["ground", item]));
        if self.core.input["groundFails"] == true {
            return Err(scene::Error("oracle".into()));
        }
        if self.core.input["groundHandles"] == true {
            Joined {
                scene,
                core: self.core,
            }
            .act(self.state, &json!({"op":"handle"}))
            .map_err(|e| scene::Error(format!("{e:?}")))?;
        }
        Ok(())
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
}
impl Joined<'_> {
    fn replace(&mut self, state: &mut State, delay: i32) -> legacy::Result<()> {
        let l = self.core.closed.read().location.clone();
        let mut services = MapServices {
            state,
            core: self.core,
        };
        let mut map = self.scene.context(&mut services);
        let current = map.object(l.clone(), -1, 0).map_err(err)?.unwrap();
        let target = scenery::Context::construct(&mut map, 10327, l.clone(), 0, 2).map_err(err)?;
        scenery::replace(
            &mut map,
            Some(&current),
            Some(&target),
            ReplaceMode::Temporary {
                clip: true,
                ticks: delay,
            },
        )
        .map_err(err)?;
        let result = map.object(l, -1, 0).map_err(err)?.unwrap();
        map.services.core.held.push(result);
        Ok(())
    }
    fn act(&mut self, state: &mut State, a: &Value) -> legacy::Result<()> {
        match a["op"].as_str().unwrap() {
            "handle" => {
                let p = self.core.actor.base.player;
                let t = self.core.actor.base.scenery.as_ref().map(|n| n.node);
                let o = self.core.actor.base.option.clone();
                legacy_runtime::handle(state, self, t, Some(p), o)
            }
            "tick" => {
                let p = self.core.actor.base.player;
                state.context(self).pulses().map_err(err)?;
                state.context(self).entity(p).map_err(err)?;
                state.ticks = state.ticks.wrapping_add(1);
                self.scene.collect_pulses();
                Ok(())
            }
            "ticks" => {
                for _ in 0..n(a, "count", 1) {
                    self.act(state, &json!({"op":"tick"}))?;
                }
                Ok(())
            }
            "sceneReplace" => self.replace(state, n(a, "ticks", 1)),
            "sceneClient" => {
                let closed = self.core.closed.clone();
                let l = closed.read().location.clone();
                let mut services = MapServices {
                    state,
                    core: self.core,
                };
                let mut map = self.scene.context(&mut services);
                let target = scenery::Context::construct(&mut map, 10327, l, 0, 2).map_err(err)?;
                scenery::replace(
                    &mut map,
                    Some(&closed),
                    Some(&target),
                    ReplaceMode::Temporary {
                        clip: false,
                        ticks: n(a, "ticks", 1),
                    },
                )
                .map_err(err)?;
                Ok(())
            }
            "sceneExpire" => {
                let mut services = MapServices {
                    state,
                    core: self.core,
                };
                let mut map = self.scene.context(&mut services);
                let h = scenery::Context::construct(
                    &mut map,
                    10327,
                    Location::new(3252, 3250, 0),
                    10,
                    2,
                )
                .map_err(err)?;
                map.services.core.expiry = Some(h.clone());
                map.services.core.held.push(h.clone());
                let placed = scenery::add(
                    &mut map,
                    Some(&h),
                    n(a, "ticks", 1),
                    Some(Shared::new(vec![Some(42)].into_boxed_slice())),
                )
                .map_err(err)?;
                map.services.core.held.push(placed);
                Ok(())
            }
            "sceneRespawn" => {
                let h = self.core.closed.clone();
                scenery::remove_respawn(
                    &mut self.scene.context(&mut MapServices {
                        state,
                        core: self.core,
                    }),
                    Some(&h),
                    n(a, "ticks", 1),
                )
                .map_err(err)?;
                Ok(())
            }
            "sceneStop" | "sceneClose" | "sceneInactive" => {
                let closed = self.core.closed.clone();
                let l = closed.read().location.clone();
                let mut services = MapServices {
                    state,
                    core: self.core,
                };
                let mut map = self.scene.context(&mut services);
                let current = map.object(l, -1, 0).map_err(err)?.unwrap();
                match a["op"].as_str().unwrap() {
                    "sceneStop" => {
                        if let Some(p) = map.scene.restore_pulse(&current).map_err(err)? {
                            map.stop_pulse(&p).map_err(err)?;
                        }
                    }
                    "sceneClose" => {
                        scenery::replace(
                            &mut map,
                            Some(&current),
                            Some(&closed),
                            ReplaceMode::Swap {
                                clip: true,
                                permanent: false,
                            },
                        )
                        .map_err(err)?;
                    }
                    _ => current.write().active = false,
                }
                Ok(())
            }
            "sceneCheckActive" => {
                self.core.expiry.as_ref().unwrap().write().active = a["active"] == true;
                Ok(())
            }
            _ => self.core.actor.act(state, a),
        }
    }
}
impl Services for Joined<'_> {
    fn external_pulse(&mut self, s: &mut State, p: &pulses::Handle) -> pulses::Result<bool> {
        self.scene
            .context(&mut MapServices {
                state: s,
                core: self.core,
            })
            .pulse_body(p)
    }
    fn external_inactive(&mut self, s: &mut State, p: &pulses::Handle) -> pulses::Result<bool> {
        if self.scene.owns_pulse(p)
            && p.read().checks.is_some()
            && self.core.input["failChecks"] == true
        {
            return Err(pulses::Error::Binding("oracle".into()));
        }
        self.scene
            .context(&mut MapServices {
                state: s,
                core: self.core,
            })
            .pulse_inactive(p)
    }
    fn external_stop(&mut self, s: &mut State, p: &pulses::Handle) -> pulses::Result<()> {
        self.scene
            .context(&mut MapServices {
                state: s,
                core: self.core,
            })
            .stop_pulse(p)
    }
    fn query(&mut self, s: &mut State, q: Query<'_>) -> walk::Result<bool> {
        self.core.actor.query(s, q)
    }
    fn event(&mut self, s: &mut State, e: Event<'_>) -> walk::Result<()> {
        self.core.actor.event(s, e)
    }
    fn interfaces(&self, s: &State, n: Node) -> walk::Result<Interfaces> {
        self.core.actor.interfaces(s, n)
    }
    fn region(&mut self, s: &mut State, l: &Location) -> walk::Result<Option<bool>> {
        self.core.actor.region(s, l)
    }
    fn clipping(&mut self, s: &mut State, z: i32, x: i32, y: i32) -> walk::Result<i32> {
        self.core.actor.clipping(s, z, x, y)
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
        self.core.actor.destination(s, m, t, d)
    }
    fn door_destination(
        &mut self,
        s: &mut State,
        m: Option<Node>,
        t: Option<Node>,
    ) -> walk::Result<Option<Location>> {
        self.core.actor.door_destination(s, m, t)
    }
    fn teleport_permitted(&mut self, s: &mut State, l: Option<Location>) -> walk::Result<bool> {
        self.core.actor.teleport_permitted(s, l)
    }
    fn interaction_arrival(&mut self, s: &mut State, a: &Approach) -> walk::Result<bool> {
        legacy_runtime::arrival(s, self, &Task::from(a)).map_err(werr)
    }
    fn delayed_interaction(&mut self, s: &mut State, t: &Task) -> walk::Result<bool> {
        legacy_runtime::arrival(s, self, t).map_err(werr)
    }
}
impl Bindings for Joined<'_> {
    fn interaction_locked(&mut self, s: &mut State, p: Option<Node>) -> legacy::Result<bool> {
        self.core.actor.interaction_locked(s, p)
    }
    fn handler_walk(&mut self, s: &mut State, h: usize) -> legacy::Result<bool> {
        self.core.actor.handler_walk(s, h)
    }
    fn handler_walk_for(
        &mut self,
        s: &mut State,
        h: usize,
        p: Option<Node>,
        t: Option<Node>,
    ) -> legacy::Result<bool> {
        self.core.actor.handler_walk_for(s, h, p, t)
    }
    fn handler_delayed(
        &mut self,
        s: &mut State,
        h: usize,
        p: Option<Node>,
    ) -> legacy::Result<bool> {
        self.core.actor.handler_delayed(s, h, p)
    }
    fn handler_name(&mut self, s: &mut State, h: usize) -> legacy::Result<String> {
        self.core.actor.handler_name(s, h)
    }
    fn handler_handle(
        &mut self,
        s: &mut State,
        h: usize,
        p: Option<Node>,
        t: Option<Node>,
        o: &str,
    ) -> legacy::Result<bool> {
        let answer = self.core.actor.handler_handle(s, h, p, t, o)?;
        self.core.calls += 1;
        if self.core.calls == 1 && self.core.input["sceneOnHandle"] == true {
            self.replace(s, n(&self.core.input, "sceneDelay", 1))?;
        }
        Ok(answer)
    }
    fn lowercase(&self, s: &str) -> String {
        self.core.actor.lowercase(s)
    }
    fn zone(
        &mut self,
        s: &mut State,
        p: Option<Node>,
        t: Option<Node>,
        o: Option<&OptionHandle>,
    ) -> legacy::Result<bool> {
        self.core.actor.zone(s, p, t, o)
    }
    fn combat_stop(&mut self, s: &mut State, p: Option<Node>) -> legacy::Result<()> {
        self.core.actor.combat_stop(s, p)
    }
    fn face_player(
        &mut self,
        s: &mut State,
        p: Option<Node>,
        l: Option<Location>,
    ) -> legacy::Result<()> {
        self.core.actor.face_player(s, p, l)
    }
    fn dispatch(
        &mut self,
        s: &mut State,
        p: Option<Node>,
        t: Option<Node>,
        o: &str,
    ) -> legacy::Result<()> {
        self.core.actor.dispatch(s, p, t, o)
    }
    fn exception(&mut self, s: &mut State, e: &legacy::Error) -> legacy::Result<()> {
        self.core.actor.exception(s, e)
    }
    fn error_message(
        &mut self,
        s: &mut State,
        e: &legacy::Error,
    ) -> legacy::Result<Option<String>> {
        self.core.actor.error_message(s, e)
    }
    fn log(&mut self, s: &mut State, c: &str, m: &str) -> legacy::Result<()> {
        self.core.actor.log(s, c, m)
    }
}
pub fn create(input: &Value) -> (State, Model) {
    let (state, actor) = actors::create(input);
    let mut scene = maps::create(&json!({"land":"b03d8cb3010000"}));
    scene.runner = state.runner();
    let closed = scene
        .context(&mut maps::Bindings::default())
        .object(Location::new(3250, 3250, 0), -1, 0)
        .unwrap()
        .unwrap();
    scene.clear_updates();
    (
        state,
        Model {
            scene,
            core: Core {
                actor,
                input: input.clone(),
                closed: closed.clone(),
                expiry: None,
                held: vec![closed],
                calls: 0,
            },
        },
    )
}
impl Model {
    pub fn pulses(&mut self, state: &mut State) -> walk::Result<()> {
        state
            .context(&mut Joined {
                scene: &mut self.scene,
                core: &mut self.core,
            })
            .pulses()?;
        self.scene.collect_pulses();
        Ok(())
    }
    pub fn disconnect(&mut self, state: &mut State) -> walk::Result<()> {
        let player = self.core.actor.base.player;
        self.core.actor.base.scenery = None;
        self.core.actor.base.option = None;
        state
            .context(&mut Joined {
                scene: &mut self.scene,
                core: &mut self.core,
            })
            .disconnect(player)
    }
    pub fn act(&mut self, state: &mut State, a: &Value) -> legacy::Result<()> {
        Joined {
            scene: &mut self.scene,
            core: &mut self.core,
        }
        .act(state, a)
    }
    pub fn snapshot(&mut self, state: &State, error: Option<&str>) -> Value {
        let mut v = self.core.actor.snapshot(state, error);
        let mut rows = vec![];
        for (x, y, t) in [(3250, 3250, 0), (3252, 3250, 10)] {
            if let Some(h) = self
                .scene
                .context(&mut maps::Bindings::default())
                .object(Location::new(x, y, 0), -1, t)
                .unwrap()
            {
                rows.push(object(&h));
            }
        }
        v["worldObjects"] = json!(rows);
        v["worldHeld"] = json!(self.core.held.iter().map(object).collect::<Vec<_>>());
        let mut keys = self.scene.updates.keys().copied().collect::<Vec<_>>();
        keys.sort();
        let mut updates = vec![];
        for k in keys {
            for u in &self.scene.updates[&k] {
                let h = u.object.read();
                updates.push(json!([
                    h.id,
                    u.remove,
                    h.location.coordinates(),
                    h.shape,
                    h.rotation
                ]));
            }
        }
        v["worldUpdates"] = json!(updates);
        self.scene.clear_updates();
        v["worldQueue"] = json!(
            state
                .pending()
                .iter()
                .map(|p| {
                    let p = p.read();
                    json!([p.class_name, p.delay, p.ticks_passed, p.running])
                })
                .collect::<Vec<_>>()
        );
        v
    }
}
pub fn evaluate(input: &Value) -> Value {
    let (mut state, mut m) = create(input);
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.core.actor.base.events.clear();
        let error = m.act(&mut state, a).err().map(|_| "Binding");
        state.collect();
        out.push(m.snapshot(&state, error));
    }
    json!(out)
}
