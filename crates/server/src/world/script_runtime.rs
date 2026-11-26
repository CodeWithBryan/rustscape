//! ScriptProcessor on the actual owning-world entity, queue and clocks. Content,
//! UI, animator, geometry and projectile services remain explicit host bindings.
//! A context retains its final entity; it has no mutable global current-owner slot.
use super::component_objects::{Captures, NativeObject};
use super::{
    interaction_listeners::Function,
    movement::{self, Kind},
    player_walk::{self as walk, NodeHandle, Scripts, State},
};
use crate::scripts::{
    self, Arguments, Destination, EntityKind, Error, Level, Location, NodeKind, Path, Result,
    Runtime,
};

/// A native callback stores its actual capture fields and an executable Rust
/// body. The body receives this object, never an observation ID or wire index.
pub struct Native<B> {
    captures: Captures,
    body: fn(&Self, &mut State, &mut B, &NodeHandle, Arguments<NodeHandle>) -> Result<bool>,
}
impl<B: 'static> NativeObject for Native<B> {
    fn captures(&self) -> Captures {
        self.captures.clone()
    }
}
impl<B> Native<B> {
    pub fn new(
        captures: Captures,
        body: fn(&Self, &mut State, &mut B, &NodeHandle, Arguments<NodeHandle>) -> Result<bool>,
    ) -> Self {
        Self { captures, body }
    }
    pub fn fields(&self) -> &Captures {
        &self.captures
    }
    pub fn invoke(
        &self,
        state: &mut State,
        bindings: &mut B,
        owner: &NodeHandle,
        arguments: Arguments<NodeHandle>,
    ) -> Result<bool> {
        node(state, owner)?;
        validate_arguments(state, &arguments)?;
        validate_captures(state, self.captures.clone())?;
        (self.body)(self, state, bindings, owner, arguments)
    }
}
/// Check every supported physical capture before a native body has any effects.
/// Metadata is inspected outside object guards and cyclic objects are visited once.
pub(crate) fn validate_captures(state: &State, captures: Captures) -> Result<()> {
    use std::collections::BTreeSet;
    let mut pending = vec![captures];
    let mut visited = BTreeSet::new();
    while let Some(c) = pending.pop() {
        for n in c.nodes {
            node(state, &n)?;
        }
        for pin in c.pins {
            if visited.insert((0, pin.ownership().0))
                && let Some(n) = pin.player()
            {
                node(state, &n)?;
            }
        }
        for container in c.containers {
            if !visited.insert((1, container.identity())) {
                continue;
            }
            if let Some(Some(n)) = container.equipment_owner() {
                node(state, &n)?;
            }
            let mut listeners = container.listener_list().values();
            if container.is_bank() {
                let bank =
                    crate::inventory::bank::Bank::from_container(container).map_err(error)?;
                if let Some(n) = bank.owner() {
                    node(state, &n)?;
                }
                listeners.push(Some(bank.listener()));
            }
            for listener in listeners.into_iter().flatten() {
                if visited.insert((2, listener.identity())) {
                    for n in listener.retained_nodes() {
                        node(state, &n)?;
                    }
                }
            }
        }
        for object in c.objects {
            if visited.insert((3, object.identity())) {
                pending.push(object.captures());
            }
        }
        for object in c.attributes {
            let id = std::sync::Arc::as_ptr(&object) as *const () as usize;
            if visited.insert((3, id)) {
                pending.push(object.captures());
            }
        }
        for processor in c.processors {
            if !visited.insert((4, processor.fields.identity())) {
                continue;
            }
            processor.validate_in(state)?;
            let (nodes, scripts) = processor.stored_fields();
            pending.push(Captures {
                nodes,
                scripts,
                ..Default::default()
            });
        }
        for script in c.scripts {
            if !visited.insert((5, script.identity())) {
                continue;
            }
            let (domain, form, execution) = {
                let s = script.read();
                (s.domain(), s.form.clone(), s.execution.clone())
            };
            if !domain.is_some_and(|d| d.same(&state.identity())) {
                return Err(Error::Binding(
                    "native callback captures a foreign/unscoped script".into(),
                ));
            }
            let nodes = match form {
                scripts::Form::UseWith { used, with, .. }
                | scripts::Form::QueuedUseWith { used, with, .. } => vec![used, with],
                _ => vec![],
            };
            pending.push(Captures {
                nodes,
                objects: execution.into_iter().map(|f| f.header()).collect(),
                ..Default::default()
            });
        }
        for locks in c.action_locks {
            if !visited.insert((6, locks.fields.identity())) {
                continue;
            }
            if !locks.domain().is_some_and(|d| d.same(&state.identity())) {
                return Err(Error::Binding(
                    "native callback captures foreign/unscoped action locks".into(),
                ));
            }
            let (fields, map) = locks.stored_fields();
            pending.push(Captures {
                locks: fields,
                lock_maps: vec![map],
                ..Default::default()
            });
        }
        for lock in c.locks {
            if !visited.insert((7, lock.fields.identity())) {
                continue;
            }
            if !lock.domain().is_some_and(|d| d.same(&state.identity())) {
                return Err(Error::Binding(
                    "native callback captures a foreign/unscoped lock".into(),
                ));
            }
            pending.push(Captures {
                objects: lock.elapse().into_iter().collect(),
                ..Default::default()
            });
        }
        for map in c.lock_maps {
            if !visited.insert((8, map.identity())) {
                continue;
            }
            let fields = map.read().values().cloned().collect();
            pending.push(Captures {
                locks: fields,
                ..Default::default()
            });
        }
    }
    Ok(())
}

pub const MOVEMENT: usize = 0;
pub const STUN: usize = 6;

/// Original Player.hasModalOpen, with its retained component receivers and
/// exclusions. It deliberately differs from postMovement's two separate gates.
pub fn has_modal_open(manager: &super::interfaces::Manager) -> bool {
    let (opened, chatbox) = {
        let f = manager.fields.read();
        (f.opened.clone(), f.chatbox.clone())
    };
    let excluded = [372, 421, 137];
    let mut modal = false;
    for component in [opened, chatbox].into_iter().flatten() {
        for (i, id) in excluded.iter().enumerate() {
            if *id == component.read().id {
                break;
            } else if i == excluded.len() - 1 {
                modal = true;
            }
        }
    }
    modal
}
pub fn center_location(state: &State, target: &NodeHandle) -> Result<Location> {
    node(state, target)?;
    let location = state
        .node_location(target.node)
        .map_err(error)?
        .ok_or(Error::Null)?;
    let view = state.node_view(target.node).map_err(error)?;
    let [dx, dy] = if view.kind == Kind::Scenery {
        let mut size = view.dimensions;
        if view.direction.ok_or(Error::Null)?.code() % 2 != 0 {
            size.swap(0, 1);
        }
        [size[0] >> 1, size[1] >> 1]
    } else {
        [view.size >> 1; 2]
    };
    let [x, y, z] = location.coordinates();
    Ok(Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z))
}

/// Services operate on the exact retained entity and actual native function.
/// No default success binding stands in for content/UI/animation/projectiles.
pub trait Bindings: walk::Services {
    fn kind(&self, state: &State, owner: &NodeHandle) -> EntityKind {
        match state
            .actor(owner.node)
            .expect("validated script entity")
            .view
            .kind
        {
            Kind::Player => EntityKind::Player,
            _ => EntityKind::Other,
        }
    }
    fn modal_open(&mut self, state: &mut State, owner: &NodeHandle) -> Result<bool>;
    fn interface_open(&mut self, state: &mut State, owner: &NodeHandle) -> Result<bool>;
    fn chatbox_open(&mut self, state: &mut State, owner: &NodeHandle) -> Result<bool>;
    fn close_interfaces(&mut self, state: &mut State, owner: &NodeHandle) -> Result<()>;
    fn reset_animator(&mut self, state: &mut State, owner: &NodeHandle) -> Result<()>;
    fn node_description(&mut self, state: &mut State, node: Option<NodeHandle>) -> Result<String>;
    fn face_location(
        &mut self,
        state: &mut State,
        node: NodeHandle,
        from: Option<Location>,
    ) -> Result<Option<Location>>;
    fn center_location(&mut self, state: &mut State, node: NodeHandle) -> Result<Option<Location>>;
    fn line_of_sight(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        location: &Location,
    ) -> Result<bool>;
    fn face(&mut self, state: &mut State, owner: &NodeHandle, location: Location) -> Result<()>;
    fn execution_equals(
        &mut self,
        state: &mut State,
        left: Option<Function>,
        right: Option<Function>,
    ) -> Result<bool>;
    fn invoke(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        execution: Option<Function>,
        arguments: Arguments<NodeHandle>,
    ) -> Result<bool>;
    fn exception_text(&mut self, state: &mut State, error: &Error) -> Result<String>;
    fn log(&mut self, state: &mut State, owner: &str, level: Level, text: String) -> Result<()>;
    fn lock_entity(&mut self, state: &mut State, owner: &NodeHandle, ticks: i32) -> Result<()>;
}

pub fn error(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn node(state: &State, handle: &NodeHandle) -> Result<()> {
    if !state.contains_handle(handle) {
        return Err(Error::Binding(
            "script node belongs to another world/allocation".into(),
        ));
    }
    Ok(())
}
fn validate_arguments(state: &State, arguments: &Arguments<NodeHandle>) -> Result<()> {
    match arguments {
        Arguments::Void { .. } => Ok(()),
        Arguments::Interaction { target, .. } => node(state, target),
        Arguments::UseWith { used, with, .. } => {
            node(state, used)?;
            node(state, with)
        }
    }
}
pub struct Context<'a, B: Bindings> {
    owner: NodeHandle,
    kind: EntityKind,
    bindings: &'a mut B,
}
impl<'a, B: Bindings> Context<'a, B> {
    pub fn new(state: &State, bindings: &'a mut B, processor: &Runtime) -> Result<Self> {
        let owner = processor.validate_in(state)?;
        let kind = bindings.kind(state, &owner);
        Ok(Self {
            owner,
            kind,
            bindings,
        })
    }
    fn clock(&self, state: &State, slot: usize) -> Result<i32> {
        state
            .clocks(&self.owner)
            .map_err(error)?
            .read()
            .get(slot)
            .copied()
            .ok_or(Error::Index)
    }
}
macro_rules! owner_binding {
    ($name:ident->$ret:ty) => {
        fn $name(&mut self, _: &Runtime, state: &mut State) -> Result<$ret> {
            self.bindings.$name(state, &self.owner)
        }
    };
}
impl<B: Bindings> scripts::Context<State, Function, NodeHandle> for Context<'_, B> {
    fn entity_kind(&self, _: &State) -> EntityKind {
        self.kind
    }
    fn ticks(&mut self, _: &Runtime, state: &mut State) -> Result<i32> {
        Ok(state.ticks)
    }
    fn delayed(&mut self, _: &Runtime, state: &mut State) -> Result<bool> {
        // Entity.delayed reads Entity.scripts, which may be distinct from the
        // retained receiver whose method is currently executing.
        Ok(state.scripts(&self.owner).map_err(error)?.delay() > state.ticks)
    }
    fn stunned(&mut self, _: &Runtime, state: &mut State) -> Result<bool> {
        Ok(self.clock(state, STUN)? >= state.ticks)
    }
    owner_binding!(modal_open->bool);
    owner_binding!(interface_open->bool);
    owner_binding!(chatbox_open->bool);
    owner_binding!(close_interfaces->());
    owner_binding!(reset_animator->());
    fn running(&mut self, _: &Runtime, state: &mut State) -> Result<bool> {
        Ok(state.actor(self.owner.node).map_err(error)?.queue.running())
    }
    fn set_movement_clock(&mut self, value: i32, _: &Runtime, state: &mut State) -> Result<()> {
        let clocks = state.clocks(&self.owner).map_err(error)?;
        *clocks.write().get_mut(MOVEMENT).ok_or(Error::Index)? = value;
        Ok(())
    }
    fn finished_moving(&mut self, _: &Runtime, state: &mut State) -> Result<bool> {
        Ok(self.clock(state, MOVEMENT)? < state.ticks)
    }
    fn entity_location(&mut self, _: &Runtime, state: &mut State) -> Result<Option<Location>> {
        state.node_location(self.owner.node).map_err(error)
    }
    fn node_kind(&self, target: NodeHandle, state: &State) -> NodeKind {
        if state.ground(target.node).is_ok() {
            return NodeKind::GroundItem;
        }
        match state.node_view(target.node).map(|v| v.kind) {
            Ok(Kind::Npc) => NodeKind::Npc,
            Ok(Kind::Scenery) => NodeKind::Scenery,
            _ => NodeKind::Other,
        }
    }
    fn node_active(&mut self, target: NodeHandle, _: &Runtime, state: &mut State) -> Result<bool> {
        node(state, &target)?;
        self.bindings.node_active(state, target.node).map_err(error)
    }
    fn node_description(
        &mut self,
        target: Option<NodeHandle>,
        _: &Runtime,
        state: &mut State,
    ) -> Result<String> {
        if let Some(target) = &target {
            node(state, target)?;
        }
        self.bindings.node_description(state, target)
    }
    fn node_location(
        &mut self,
        target: NodeHandle,
        _: &Runtime,
        state: &mut State,
    ) -> Result<Option<Location>> {
        node(state, &target)?;
        state.node_location(target.node).map_err(error)
    }
    fn face_location(
        &mut self,
        target: NodeHandle,
        from: Option<Location>,
        _: &Runtime,
        state: &mut State,
    ) -> Result<Option<Location>> {
        node(state, &target)?;
        self.bindings.face_location(state, target, from)
    }
    fn center_location(
        &mut self,
        target: NodeHandle,
        _: &Runtime,
        state: &mut State,
    ) -> Result<Option<Location>> {
        node(state, &target)?;
        self.bindings.center_location(state, target)
    }
    fn destination(
        &mut self,
        flag: Destination,
        target: NodeHandle,
        _: &Runtime,
        state: &mut State,
    ) -> Result<Option<Location>> {
        node(state, &target)?;
        let flag = match flag {
            Destination::Entity => movement::Flag::Entity,
            Destination::Item => movement::Flag::Item,
        };
        flag.destination(
            &mut state.context(self.bindings),
            Some(self.owner.node),
            Some(target.node),
        )
        .map_err(error)
    }
    fn pathfind(
        &mut self,
        target: Option<NodeHandle>,
        _: &Runtime,
        state: &mut State,
    ) -> Result<Path> {
        let target = target.ok_or(Error::Null)?;
        node(state, &target)?;
        let path = state
            .context(self.bindings)
            .find_node_path(self.owner.node, target.node)
            .map_err(error)?;
        Ok(Path {
            move_near: path.move_near,
            points: path.points,
        })
    }
    fn line_of_sight(
        &mut self,
        location: &Location,
        _: &Runtime,
        state: &mut State,
    ) -> Result<bool> {
        self.bindings.line_of_sight(state, &self.owner, location)
    }
    fn face(&mut self, location: Location, _: &Runtime, state: &mut State) -> Result<()> {
        self.bindings.face(state, &self.owner, location)
    }
    fn message(&mut self, text: &str, _: &Runtime, state: &mut State) -> Result<()> {
        self.bindings
            .event(state, walk::Event::Message(self.owner.node, text))
            .map_err(error)
    }
    fn execution_equals(
        &mut self,
        left: Option<Function>,
        right: Option<Function>,
        _: &Runtime,
        state: &mut State,
    ) -> Result<bool> {
        self.bindings.execution_equals(state, left, right)
    }
    fn execute(
        &mut self,
        execution: Option<Function>,
        arguments: Arguments<NodeHandle>,
        _: &Runtime,
        state: &mut State,
    ) -> Result<bool> {
        validate_arguments(state, &arguments)?;
        self.bindings
            .invoke(state, &self.owner, execution, arguments)
    }
    fn exception_text(&mut self, error: &Error, _: &Runtime, state: &mut State) -> Result<String> {
        self.bindings.exception_text(state, error)
    }
    fn log(
        &mut self,
        owner: &str,
        level: Level,
        text: String,
        _: &Runtime,
        state: &mut State,
    ) -> Result<()> {
        self.bindings.log(state, owner, level, text)
    }
    fn lock_entity(&mut self, ticks: i32, _: &Runtime, state: &mut State) -> Result<()> {
        self.bindings.lock_entity(state, &self.owner, ticks)
    }
}
/// Use this at the existing tick/pulse phase boundary. It captures the canonical
/// receiver once, preserving late Entity.scripts reads inside its Context.
pub fn phase<B: Bindings>(
    state: &mut State,
    bindings: &mut B,
    owner: &NodeHandle,
    phase: Scripts,
) -> Result<()> {
    let processor = state.scripts(owner).map_err(error)?;
    let mut ctx = Context::new(state, bindings, &processor)?;
    match phase {
        Scripts::Weak => {
            processor.remove_weak();
            Ok(())
        }
        Scripts::Normal => {
            processor.remove_normal();
            Ok(())
        }
        Scripts::Reset => processor.reset(state, &mut ctx),
        Scripts::Pre => processor.pre_movement(state, &mut ctx),
        Scripts::Post(moved) => processor.post_movement(moved, state, &mut ctx),
    }
}
