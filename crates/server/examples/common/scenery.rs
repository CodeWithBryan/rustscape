//! Controlled probe bindings for the native packet/option/arrival/door path.
//! Empty content registries are explicit. Full action locks, profile Properties,
//! diaries and autowalk logout listeners still require their real services.
use super::{Host, Map, MapServices, Node, State, now};
use rs09_protocol::game::{Frame, actions::ClientAction, updates};
use rs09_server::{
    definitions::scenery as defs,
    pulses,
    world::{
        animation::{Animation, Animator},
        door_option, door_runtime,
        doors::{self, Handle, Location},
        interaction::OptionHandle,
        legacy_interaction as legacy, legacy_runtime,
        movement::DestinationService,
        pathfinding,
        player_walk::{self as walk, Event, Query},
        prepared_animation::{self, Owner},
        scene, scenery_action, scenery_node, scenery_runtime,
    },
};
use std::collections::BTreeMap;
fn werror(e: impl std::fmt::Debug) -> walk::Error {
    walk::Error::Binding(format!("{e:?}"))
}
fn lerror(e: impl std::fmt::Debug) -> legacy::Error {
    legacy::Error::Binding(Some(format!("{e:?}")))
}
fn aerror(e: impl std::fmt::Debug) -> scenery_action::Error {
    scenery_action::Error::Binding(format!("{e:?}"))
}
fn derror(e: impl std::fmt::Debug) -> doors::Error {
    doors::Error::Binding(format!("{e:?}"))
}
// Read allocated arrays without loading regions or calling gameplay services.
// Missing arrays remain null in this optional diagnostic observation.
fn clip_snapshot(scene: &Map, location: &Location) -> Vec<serde_json::Value> {
    let [cx, cy, z] = location.coordinates();
    let mut rows = vec![];
    for dx in -2..=2 {
        for dy in -2..=2 {
            let x = cx + dx;
            let y = cy + dy;
            let key = (z << 22) | ((x >> 3) << 11) | (y >> 3);
            let index = ((x & 7) * 8 + (y & 7)) as usize;
            rows.push(serde_json::json!({"location":[x,y,z],"walking":scene.flags.get(&(key,false)).map(|f|f.read()[index]),"projectile":scene.flags.get(&(key,true)).map(|f|f.read()[index])}));
        }
    }
    rows
}
pub(super) fn process(
    state: &mut State,
    host: &mut Host,
    node: Node,
    action: &ClientAction,
) -> walk::Result<bool> {
    if host.scenery.is_none() {
        return Ok(false);
    }
    let Some(request) = scenery_action::Request::from_action(node, action) else {
        return Ok(false);
    };
    scenery_runtime::process(state, host, request).map_err(werror)?;
    Ok(true)
}
fn program(host: &Host, id: usize) -> legacy::Result<door_option::Program> {
    let registry = host
        .scenery
        .as_ref()
        .ok_or_else(|| lerror("scenery options disabled"))?;
    let handler = registry.handler(id).map_err(lerror)?;
    let program = handler
        .read()
        .value::<door_option::Program>()
        .copied()
        .ok_or_else(|| lerror("unbound native scenery handler"))?;
    Ok(program)
}
struct Actors<'a> {
    state: &'a mut State,
    maps: &'a mut MapServices,
    frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    animations: &'a mut prepared_animation::World<super::AnimationAssets>,
    animators: &'a BTreeMap<Node, Animator<Owner>>,
}
macro_rules! unbound_actor{
    ($name:ident($($arg:ident:$ty:ty),*)->$ret:ty)=>{fn $name(&mut self,$($arg:$ty),*)->scene::Result<$ret>{Err(scene::Error(concat!("unbound player scene service: ",stringify!($name)).into()))}};
}
impl scene::Services<scene::CacheAssets> for Actors<'_> {
    fn is_player(&self, n: Node) -> bool {
        self.maps.players.contains(&n)
    }
    fn location(&mut self, _: &mut Map, n: Node) -> scene::Result<Location> {
        self.state
            .node_location(n)
            .map_err(|e| scene::Error(e.to_string()))?
            .ok_or_else(|| scene::Error("null actor location".into()))
    }
    fn audio(&mut self, _: &mut Map, n: Node, id: i32, delay: Option<i32>) -> scene::Result<()> {
        self.state
            .actor(n)
            .map_err(|e| scene::Error(e.to_string()))?;
        self.frames
            .entry(n)
            .or_default()
            .push(updates::synth_sound(id, 1, delay.unwrap_or(0)));
        Ok(())
    }
    fn message(&mut self, _: &mut Map, n: Node, s: &str) -> scene::Result<()> {
        self.state
            .actor(n)
            .map_err(|e| scene::Error(e.to_string()))?;
        let mut payload = s.as_bytes().to_vec();
        payload.push(0);
        self.frames.entry(n).or_default().push(Frame {
            opcode: 70,
            payload,
        });
        Ok(())
    }
    fn move_near(&mut self, scene: &mut Map, n: Node, l: Location) -> scene::Result<bool> {
        self.state
            .context(&mut Routing {
                scene,
                maps: self.maps,
            })
            .find_path(n, l, pathfinding::dispatch::SMART, true)
            .map(|p| p.move_near)
            .map_err(|e| scene::Error(e.to_string()))
    }
    unbound_actor!(lock(_s: &mut Map, _n: Node, _ticks: i32) -> ());
    unbound_actor!(logout_listener(_s: &mut Map, _n: Node, _l: Option<Location>) -> ());
    unbound_actor!(reset_path(_s: &mut Map, _n: Node) -> ());
    unbound_actor!(add_path(_s: &mut Map, _n: Node, _x: i32, _y: i32) -> ());
    unbound_actor!(diary(_s: &mut Map, _n: Node, _d: doors::Diary, _level: i32, _task: i32) -> ());
    unbound_actor!(ground_create(_s: &mut Map, _n: Option<usize>) -> ());
    unbound_actor!(pulse(_s: &mut Map, _p: &pulses::Handle) -> bool);
    unbound_actor!(stop(_s: &mut Map, _p: &pulses::Handle) -> ());
    fn now_millis(&mut self) -> i64 {
        now()
    }
}
impl door_runtime::Bindings<scene::CacheAssets> for Actors<'_> {
    type Properties = ();
    fn lowercase(&self, s: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(s)
    }
    fn player_location(&mut self, p: Option<Node>) -> doors::Result<Option<Location>> {
        self.state
            .node_location(p.ok_or(doors::Error::Null)?)
            .map_err(derror)
    }
    fn animate(&mut self, p: Option<Node>, id: i32, forced: bool) -> doors::Result<()> {
        let animator = self
            .animators
            .get(&p.ok_or(doors::Error::Null)?)
            .ok_or(doors::Error::Null)?
            .clone();
        self.animations.ticks = self.state.ticks;
        let animation = Some(Animation::new(id));
        if forced {
            animator
                .force_animation(self.animations, animation)
                .map_err(derror)?;
        } else {
            animator
                .animate(self.animations, animation, None)
                .map_err(derror)?;
        }
        Ok(())
    }
    fn properties(&mut self, _: Option<Node>) -> doors::Result<Option<()>> {
        Err(derror("player Properties are not bound"))
    }
    fn set_teleport(&mut self, _: &(), _: Location) -> doors::Result<()> {
        Err(derror("player Properties teleport is not bound"))
    }
}
// The original DoorActionHandler destination uses the same world's Smart finder
// and mutable navigator. This adapter supplies only what that search can read.
struct Routing<'a> {
    scene: &'a mut Map,
    maps: &'a mut MapServices,
}
impl walk::Services for Routing<'_> {
    fn query(&mut self, _: &mut State, _: Query<'_>) -> walk::Result<bool> {
        Err(walk::Error::Unsupported(
            "door search queried actor services",
        ))
    }
    fn event(&mut self, _: &mut State, _: Event<'_>) -> walk::Result<()> {
        Err(walk::Error::Unsupported("door search emitted actor event"))
    }
    fn interfaces(
        &self,
        _: &State,
        _: Node,
    ) -> walk::Result<rs09_server::world::walk_request::Interfaces> {
        Err(walk::Error::Unsupported("door search queried interfaces"))
    }
    fn region(&mut self, _: &mut State, _: &Location) -> walk::Result<Option<bool>> {
        Err(walk::Error::Unsupported("door search queried activity"))
    }
    fn clipping(&mut self, _: &mut State, z: i32, x: i32, y: i32) -> walk::Result<i32> {
        self.scene
            .context(self.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(werror)
    }
    fn teleport_permitted(&mut self, _: &mut State, l: Option<Location>) -> walk::Result<bool> {
        self.scene
            .context(self.maps)
            .teleport_permitted(l.as_ref())
            .map_err(werror)
    }
    fn npc_supplier(
        &mut self,
        _: &mut State,
        _: Node,
    ) -> walk::Result<Option<pathfinding::dispatch::Supplier>> {
        Ok(Some(pathfinding::dispatch::Supplier::World))
    }
    fn now_millis(&mut self) -> i64 {
        now()
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
}
fn door_context<T>(
    state: &mut State,
    host: &mut Host,
    call: impl FnOnce(
        &mut door_runtime::Context<'_, scene::CacheAssets, Actors<'_>>,
    ) -> doors::Result<T>,
) -> doors::Result<T> {
    let mut services = Actors {
        state,
        maps: &mut host.maps,
        frames: &mut host.frames,
        animations: &mut host.animations,
        animators: &host.animators,
    };
    call(&mut door_runtime::Context {
        scene: &mut host.scene,
        services: &mut services,
    })
}
pub(super) fn destination(
    state: &mut State,
    host: &mut Host,
    p: Option<Node>,
    target: Option<Node>,
    service: DestinationService,
) -> walk::Result<Option<Location>> {
    let id = match service {
        DestinationService::Option(id) | DestinationService::Custom(id) => id,
        _ => return Err(walk::Error::Unsupported("non-scenery destination service")),
    };
    let program = program(host, id).map_err(werror)?;
    let object = state.scenery(target.ok_or(walk::Error::Null)?)?;
    door_context(state, host, |ctx| {
        program.destination(ctx, p, Some(&object))
    })
    .map_err(werror)
}
pub(super) fn door_destination(
    state: &mut State,
    host: &mut Host,
    p: Option<Node>,
    target: Option<Node>,
) -> walk::Result<Option<Location>> {
    let object = state.scenery(target.ok_or(walk::Error::Null)?)?;
    door_context(state, host, |ctx| {
        door_option::Context::door_destination(ctx, p, &object).map(Some)
    })
    .map_err(werror)
}
impl legacy_runtime::Bindings for Host {
    fn interaction_locked(&mut self, state: &mut State, p: Option<Node>) -> legacy::Result<bool> {
        let p = p.ok_or(legacy::Error::Null)?;
        super::script_services::locked(
            state,
            p,
            rs09_server::world::action_locks::Kind::Interaction,
        )
        .map_err(lerror)
    }
    fn handler_walk(&mut self, _: &mut State, id: usize) -> legacy::Result<bool> {
        if super::ground_services::program(self, id)?.is_some() {
            return Ok(true);
        }
        Ok(program(self, id)?.is_walk())
    }
    fn handler_walk_for(
        &mut self,
        _: &mut State,
        id: usize,
        _: Option<Node>,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        if super::ground_services::program(self, id)?.is_some() {
            return Ok(false);
        }
        Ok(program(self, id)?.is_walk_for())
    }
    fn handler_delayed(
        &mut self,
        _: &mut State,
        id: usize,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        if super::ground_services::program(self, id)?.is_some() {
            return Ok(true);
        }
        Ok(program(self, id)?.is_delayed())
    }
    fn handler_name(&mut self, _: &mut State, id: usize) -> legacy::Result<String> {
        if super::ground_services::program(self, id)?.is_some() {
            return Ok("PickupPlugin".into());
        }
        program(self, id)?;
        Ok("DoorManagingPlugin".into())
    }
    fn handler_handle(
        &mut self,
        state: &mut State,
        id: usize,
        p: Option<Node>,
        target: Option<Node>,
        option: &str,
    ) -> legacy::Result<bool> {
        if let Some(program) = super::ground_services::program(self, id)? {
            return super::ground_services::pickup(state, self, program, p, target, option)
                .map_err(lerror);
        }
        let program = program(self, id)?;
        let object = state
            .scenery(target.ok_or(legacy::Error::Null)?)
            .map_err(lerror)?;
        let location = object.read().location.clone();
        let before = self
            .scenery_trace
            .as_ref()
            .map(|_| clip_snapshot(&self.scene, &location));
        let frame_start = p.and_then(|p| self.frames.get(&p)).map_or(0, Vec::len);
        let result = door_context(state, self, |ctx| {
            program.handle(ctx, p, Some(&object), Some(option))
        });
        let after = self
            .scenery_trace
            .as_ref()
            .map(|_| clip_snapshot(&self.scene, &location));
        if let Some(file) = &mut self.scenery_trace {
            use std::io::Write;
            let updates:Vec<_>=self.scene.updates.values().flatten().map(|u|{let h=u.object.read();serde_json::json!({"id":h.id,"location":h.location.coordinates(),"shape":h.shape,"rotation":h.rotation,"remove":u.remove})}).collect();
            let frames:Vec<_>=p.and_then(|p|self.frames.get(&p)).into_iter().flatten().skip(frame_start).map(|f|serde_json::json!({"opcode":f.opcode,"payload":f.payload.iter().map(|b|format!("{b:02x}")).collect::<String>()})).collect();
            let row = serde_json::json!({"event":"handler","tick":state.ticks,"player":p,"target":target,"id":object.read().id,"option":option,"accepted":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:?}")),"updates":updates,"allocatedClippingBefore":before,"allocatedClippingAfter":after,"frames":frames});
            writeln!(file, "{row}").map_err(lerror)?;
        }
        result.map_err(lerror)
    }
    fn lowercase(&self, s: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(s)
    }
    fn zone(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
        _: Option<&OptionHandle>,
    ) -> legacy::Result<bool> {
        Ok(false)
    }
    fn combat_stop(&mut self, _: &mut State, _: Option<Node>) -> legacy::Result<()> {
        Ok(())
    }
    fn face_player(
        &mut self,
        _: &mut State,
        p: Option<Node>,
        l: Option<Location>,
    ) -> legacy::Result<()> {
        self.masks
            .get(&p.ok_or(legacy::Error::Null)?)
            .ok_or(legacy::Error::Null)?
            .face_location(l)
            .map_err(lerror)?;
        Ok(())
    }
    fn dispatch(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
        _: &str,
    ) -> legacy::Result<()> {
        Ok(())
    }
    fn exception(&mut self, _: &mut State, e: &legacy::Error) -> legacy::Result<()> {
        self.errors.push(e.to_string());
        eprintln!("scenery interaction: {e}");
        Ok(())
    }
    fn log(&mut self, _: &mut State, class: &str, message: &str) -> legacy::Result<()> {
        eprintln!("{class}: {message}");
        Ok(())
    }
}
struct Morph;
impl defs::PlayerContext for Morph {
    fn varbit(&mut self, _: i32, _: Node) -> defs::Result<Option<i32>> {
        Err(defs::Error::Binding("player varbits are not bound".into()))
    }
    fn varp(&mut self, _: i32, _: Node) -> defs::Result<i32> {
        Err(defs::Error::Binding("player varps are not bound".into()))
    }
}
impl scenery_runtime::Bindings for Host {
    fn scenery_object(
        &mut self,
        _: &mut State,
        x: i32,
        y: i32,
        z: i32,
        id: i32,
        shape: i32,
    ) -> scenery_action::Result<Option<Handle>> {
        self.scene
            .context(&mut self.maps)
            .object(Location::new(x, y, z), id, shape)
            .map_err(aerror)
    }
    fn scenery_construct(
        &mut self,
        _: &mut State,
        id: i32,
        l: Location,
    ) -> scenery_action::Result<Handle> {
        scenery_node::construct(&mut self.scene.definitions, id, l, 10, 0, false).map_err(aerror)
    }
    fn scenery_child(
        &mut self,
        _: &mut State,
        h: &Handle,
        p: Node,
    ) -> scenery_action::Result<Option<Handle>> {
        scenery_node::child(&mut self.scene.definitions, &mut Morph, h, Some(p))
            .map(Some)
            .map_err(aerror)
    }
    fn scenery_interaction(
        &mut self,
        state: &mut State,
        h: &Handle,
    ) -> scenery_action::Result<rs09_server::world::scenery_options::Captured> {
        let registry = self
            .scenery
            .as_ref()
            .ok_or(scenery_action::Error::Null)?
            .clone();
        registry
            .initialize(state, &mut self.scene.definitions, h, |s| {
                Ok(rs09_game_core::java_lower::lowercase_root(s))
            })
            .map_err(aerror)?;
        registry.capture(state, h).map_err(aerror)
    }
    fn scenery_varbit(&mut self, _: &mut State, _: i32) -> scenery_action::Result<Option<i32>> {
        Err(aerror("varbit definitions are not bound"))
    }
    fn scenery_listeners(
        &mut self,
        _: &mut State,
        _: i32,
        _: Option<String>,
        _: Node,
        _: &Handle,
    ) -> scenery_action::Result<bool> {
        Ok(false)
    }
    fn scenery_plugin(
        &mut self,
        _: &mut State,
        _: Node,
        _: &Handle,
    ) -> scenery_action::Result<bool> {
        Ok(false)
    }
}
#[cfg(test)]
#[path = "scenery_tests.rs"]
mod tests;
