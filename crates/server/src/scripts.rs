//! Entity interaction/queued-script state. Content executors, world nodes,
//! pathfinding, interfaces and clocks are explicit native bindings.
use crate::{definitions::item::Shared, player::attributes::AttributeLocation};
pub type Location = AttributeLocation;
pub type Node = usize;
pub type Execution = usize;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Weak,
    Normal,
    Strong,
    Soft,
}
#[derive(Clone, Debug)]
pub enum Form<N = Node> {
    Basic,
    Interaction {
        distance: i32,
    },
    UseWith {
        distance: i32,
        used: N,
        with: N,
    },
    Queued {
        strength: Strength,
    },
    QueuedUseWith {
        strength: Strength,
        used: N,
        with: N,
    },
}
impl<N> Form<N> {
    fn strength(&self) -> Option<Strength> {
        match self {
            Self::Queued { strength } | Self::QueuedUseWith { strength, .. } => Some(*strength),
            _ => None,
        }
    }
    fn distance(&self) -> i32 {
        match self {
            Self::Interaction { distance } | Self::UseWith { distance, .. } => *distance,
            _ => 10,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Basic => "Script",
            Self::Interaction { .. } => "Interaction",
            Self::UseWith { .. } => "UseWithInteraction",
            Self::Queued { .. } => "QueuedScript",
            Self::QueuedUseWith { .. } => "QueuedUseWith",
        }
    }
}
pub struct Script<E = Execution, N = Node> {
    domain: Option<crate::world::player_walk::Identity>,
    pub execution: Option<E>,
    pub persist: bool,
    pub state: i32,
    pub next_execution: i32,
    pub form: Form<N>,
    pub class_name: String,
}
impl<E, N> Script<E, N> {
    pub fn new(form: Form<N>, execution: Option<E>, persist: bool) -> Self {
        let class_name = form.name().into();
        Self {
            domain: None,
            execution,
            persist,
            state: 0,
            next_execution: 0,
            form,
            class_name,
        }
    }
    pub fn new_in(
        domain: crate::world::player_walk::Identity,
        form: Form<N>,
        execution: Option<E>,
        persist: bool,
    ) -> Self {
        Self {
            domain: Some(domain),
            ..Self::new(form, execution, persist)
        }
    }
    pub fn domain(&self) -> Option<crate::world::player_walk::Identity> {
        self.domain.clone()
    }
}
pub type Handle<E = Execution, N = Node> = Shared<Script<E, N>>;
/// Runtime callbacks and nodes are actual retained objects. The default numeric
/// parameters remain solely for the independent recording/replay adapters.
pub type Runtime =
    Processor<crate::world::interaction_listeners::Function, crate::world::player_walk::NodeHandle>;
pub type RuntimeScript =
    Handle<crate::world::interaction_listeners::Function, crate::world::player_walk::NodeHandle>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Player,
    AiPlayer,
    Other,
}
impl EntityKind {
    fn player(self) -> bool {
        self != Self::Other
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Npc,
    Scenery,
    GroundItem,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Entity,
    Item,
}
#[derive(Clone, Debug)]
pub struct Path {
    pub move_near: bool,
    pub points: Vec<[i32; 2]>,
}
#[derive(Clone, Copy, Debug)]
pub enum Arguments<N = Node> {
    Void { stage: i32 },
    Interaction { target: N, stage: i32 },
    UseWith { used: N, with: N, stage: i32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fine,
    Error,
}
const OWNER: &str = "ScriptProcessor";
pub trait Context<S, E = Execution, N = Node> {
    fn entity_kind(&self, state: &S) -> EntityKind;
    fn ticks(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<i32>;
    fn delayed(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn stunned(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn modal_open(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn interface_open(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn chatbox_open(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn close_interfaces(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<()>;
    fn reset_animator(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<()>;
    fn running(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn set_movement_clock(
        &mut self,
        value: i32,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<()>;
    fn finished_moving(&mut self, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn entity_location(
        &mut self,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<Option<Location>>;
    fn node_kind(&self, node: N, state: &S) -> NodeKind;
    fn node_active(&mut self, node: N, processor: &Processor<E, N>, state: &mut S) -> Result<bool>;
    fn node_description(
        &mut self,
        node: Option<N>,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<String>;
    fn node_location(
        &mut self,
        node: N,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<Option<Location>>;
    fn face_location(
        &mut self,
        node: N,
        location: Option<Location>,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<Option<Location>>;
    fn center_location(
        &mut self,
        node: N,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<Option<Location>>;
    fn destination(
        &mut self,
        flag: Destination,
        node: N,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<Option<Location>>;
    fn pathfind(
        &mut self,
        node: Option<N>,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<Path>;
    fn line_of_sight(
        &mut self,
        location: &Location,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<bool>;
    fn face(
        &mut self,
        location: Location,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<()>;
    fn message(&mut self, text: &str, processor: &Processor<E, N>, state: &mut S) -> Result<()>;
    fn execution_equals(
        &mut self,
        left: Option<E>,
        right: Option<E>,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<bool>;
    fn execute(
        &mut self,
        execution: Option<E>,
        arguments: Arguments<N>,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<bool>;
    fn exception_text(
        &mut self,
        error: &Error,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<String>;
    fn log(
        &mut self,
        owner: &str,
        level: Level,
        text: String,
        processor: &Processor<E, N>,
        state: &mut S,
    ) -> Result<()>;
    fn lock_entity(&mut self, ticks: i32, processor: &Processor<E, N>, state: &mut S)
    -> Result<()>;
}
pub struct Fields<E = Execution, N = Node> {
    entity: Option<N>,
    domain: Option<crate::world::player_walk::Identity>,
    pub approach: Option<Handle<E, N>>,
    pub operable: Option<Handle<E, N>>,
    pub target: Option<N>,
    pub current: Option<Handle<E, N>>,
    queue: Vec<Handle<E, N>>,
    pub delay: i32,
    pub interacted: bool,
    pub range_called: bool,
    pub range: i32,
    pub persistent: bool,
    pub target_destination: Option<Location>,
}
impl<E, N> Default for Fields<E, N> {
    fn default() -> Self {
        Self {
            entity: None,
            domain: None,
            approach: None,
            operable: None,
            target: None,
            current: None,
            queue: Vec::new(),
            delay: 0,
            interacted: false,
            range_called: false,
            range: 10,
            persistent: false,
            target_destination: None,
        }
    }
}
/// Retained ScriptProcessor identity. Clone aliases the same physical fields.
/// Read/write guards are short; every host callback sees the live shared object.
pub struct Processor<E = Execution, N = Node> {
    pub fields: Shared<Fields<E, N>>,
}
impl<E, N> Clone for Processor<E, N> {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
        }
    }
}
impl<E, N> Default for Processor<E, N> {
    fn default() -> Self {
        Self {
            fields: Shared::new(Fields::default()),
        }
    }
}
macro_rules! field {
    ($name:ident,$set:ident,$ty:ty) => {
        pub fn $name(&self) -> $ty {
            clone_field(&self.fields.read().$name)
        }
        pub fn $set(&self, value: $ty) {
            let _old = {
                let mut f = self.fields.write();
                std::mem::replace(&mut f.$name, value)
            };
        }
    };
}
fn clone_field<T: Clone>(value: &T) -> T {
    value.clone()
}
impl<E: Clone, N: Clone> Processor<E, N> {
    pub fn with_entity(entity: N) -> Self {
        let p = Self::default();
        p.fields.write().entity = Some(entity);
        p
    }
    pub fn with_entity_in(entity: N, domain: crate::world::player_walk::Identity) -> Self {
        let p = Self::with_entity(entity);
        p.fields.write().domain = Some(domain);
        p
    }
    pub fn domain(&self) -> Option<crate::world::player_walk::Identity> {
        self.fields.read().domain.clone()
    }
    pub fn entity(&self) -> Option<N> {
        self.fields.read().entity.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.fields.same(&other.fields)
    }
    pub(crate) fn stored_fields(&self) -> (Vec<N>, Vec<Handle<E, N>>) {
        let f = self.fields.read();
        let nodes = f.entity.iter().chain(f.target.iter()).cloned().collect();
        let scripts = f
            .approach
            .iter()
            .chain(f.operable.iter())
            .chain(f.current.iter())
            .chain(f.queue.iter())
            .cloned()
            .collect();
        (nodes, scripts)
    }
    pub(crate) fn clear_unreachable(&self) {
        // Drop callback/target objects after releasing the field guard. Native
        // payload destructors are allowed to inspect their former receivers.
        let old = {
            let mut f = self.fields.write();
            (
                f.approach.take(),
                f.operable.take(),
                f.current.take(),
                f.target.take(),
                std::mem::take(&mut f.queue),
            )
        };
        drop(old);
    }
    field!(approach, set_approach, Option<Handle<E, N>>);
    field!(operable, set_operable, Option<Handle<E, N>>);
    field!(target, set_target, Option<N>);
    field!(current, set_current, Option<Handle<E, N>>);
    field!(delay, set_delay, i32);
    field!(interacted, set_interacted, bool);
    field!(range_called, set_range_called, bool);
    field!(range, set_range_raw, i32);
    field!(persistent, set_persistent, bool);
    field!(target_destination, set_target_destination, Option<Location>);

    pub fn queue(&self) -> Vec<Handle<E, N>> {
        self.fields.read().queue.clone()
    }
    pub fn active(&self) -> Option<Handle<E, N>> {
        self.current()
            .or_else(|| self.operable())
            .or_else(|| self.approach())
    }
    pub fn has_type(&self, strength: Strength) -> bool {
        self.fields
            .read()
            .queue
            .iter()
            .any(|s| s.read().form.strength() == Some(strength))
    }
    pub fn remove_weak(&self) {
        self.remove_strength(Strength::Weak)
    }
    pub fn remove_normal(&self) {
        self.remove_strength(Strength::Normal)
    }
    fn remove_strength(&self, strength: Strength) {
        let removed = {
            let mut f = self.fields.write();
            let mut removed = Vec::new();
            f.queue.retain(|s| {
                let keep = s.read().form.strength() != Some(strength);
                if !keep {
                    removed.push(s.clone());
                }
                keep
            });
            removed
        };
        drop(removed);
    }
    pub fn reset<S>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<()> {
        self.set_approach(None);
        self.set_operable(None);
        self.set_current(None);
        self.set_range_called(false);
        self.set_interacted(false);
        self.set_range_raw(10);
        self.set_target(None);
        self.set_persistent(false);
        self.set_target_destination(None);
        if ctx.entity_kind(state).player() {
            ctx.reset_animator(self, state)?
        }
        Ok(())
    }
    pub fn add_to_queue<S>(
        &self,
        script: Option<&Handle<E, N>>,
        strength: Strength,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        let script = script.ok_or(Error::Null)?;
        if script.read().form.strength().is_none() {
            let name = script.read().class_name.clone();
            ctx.log(
                OWNER,
                Level::Error,
                format!("Tried to queue {name} as a queueable script but it's not!"),
                self,
                state,
            )?;
            return Ok(());
        }
        if strength == Strength::Strong && ctx.entity_kind(state).player() {
            ctx.close_interfaces(self, state)?
        }
        let next = ctx.ticks(self, state)?.wrapping_add(1);
        let current = script.read().next_execution;
        script.write().next_execution = next.max(current);
        self.fields.write().queue.push(script.clone());
        Ok(())
    }
    pub fn execute_script<S>(
        &self,
        script: Option<&Handle<E, N>>,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        let script = script.ok_or(Error::Null)?;
        self.set_current(Some(script.clone()));
        let form = script.read().form.clone();
        let args = match form {
            Form::Basic => None,
            Form::Queued { .. } => Some(Arguments::Void {
                stage: script.read().state,
            }),
            Form::Interaction { .. } => {
                if !ctx.entity_kind(state).player() {
                    return Ok(true);
                }
                let Some(target) = self.target() else {
                    return Ok(true);
                };
                Some(Arguments::Interaction {
                    target,
                    stage: script.read().state,
                })
            }
            Form::UseWith { used, with, .. } | Form::QueuedUseWith { used, with, .. } => {
                if !ctx.entity_kind(state).player() {
                    return Ok(true);
                }
                Some(Arguments::UseWith {
                    used,
                    with,
                    stage: script.read().state,
                })
            }
        };
        if let Some(args) = args {
            let execution = script.read().execution.clone();
            match ctx.execute(execution, args, self, state) {
                Ok(result) => return Ok(result),
                Err(error) => {
                    let text = ctx.exception_text(&error, self, state)?;
                    let name = script.read().class_name.clone();
                    ctx.log(OWNER,Level::Error,format!("Error processing {name} - stopping the script. Exception follows: {text}"),self,state)?;
                    self.reset(state, ctx)?;
                }
            }
        }
        self.set_current(None);
        Ok(true)
    }
    pub fn process_queue<S>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<bool> {
        let mut executed = false;
        let strong = self.has_type(Strength::Strong);
        if strong && ctx.entity_kind(state).player() {
            ctx.close_interfaces(self, state)?
        }
        if strong {
            self.remove_weak()
        }
        let mut remove = Vec::new();
        let count = self.fields.read().queue.len();
        for index in 0..count {
            let script = self
                .fields
                .read()
                .queue
                .get(index)
                .cloned()
                .ok_or(Error::Index)?;
            let form = script.read().form.clone();
            let Some(strength) = form.strength() else {
                continue;
            };
            if ctx.delayed(self, state)? && strength != Strength::Soft {
                continue;
            }
            if matches!(form, Form::QueuedUseWith { .. }) && !ctx.entity_kind(state).player() {
                remove.push(script);
                ctx.log(
                    OWNER,
                    Level::Error,
                    "Tried to queue an item UseWith interaction for a non-player!".into(),
                    self,
                    state,
                )?;
                continue;
            }
            let execution = script.read().next_execution;
            if execution > ctx.ticks(self, state)? {
                continue;
            }
            if strength == Strength::Strong && ctx.entity_kind(state).player() {
                ctx.close_interfaces(self, state)?
            }
            let next = ctx.ticks(self, state)?.wrapping_add(1);
            script.write().next_execution = next;
            let finished = self.execute_script(Some(&script), state, ctx)?;
            {
                let mut s = script.write();
                s.state = s.state.wrapping_add(1);
            }
            if finished && !script.read().persist {
                remove.push(script)
            } else if finished {
                script.write().state = 0
            }
            executed = true;
        }
        self.fields
            .write()
            .queue
            .retain(|script| !remove.iter().any(|removed| script.same(removed)));
        Ok(!executed)
    }
    pub fn process_interact<S>(
        &self,
        script: Option<&Handle<E, N>>,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        let script = script.ok_or(Error::Null)?;
        if self.target().is_none() || !ctx.node_active(self.target().unwrap(), self, state)? {
            let target = ctx.node_description(self.target(), self, state)?;
            ctx.log(
                OWNER,
                Level::Fine,
                format!("Interact target {target} no longer active, cancelling interaction."),
                self,
                state,
            )?;
            self.reset(state, ctx)?;
        }
        let next = script.read().next_execution;
        if next < ctx.ticks(self, state)? {
            let finished = self.execute_script(Some(script), state, ctx)?;
            {
                let mut s = script.write();
                s.state = s.state.wrapping_add(1);
            }
            if finished && script.read().persist {
                script.write().state = 0
            }
            self.set_interacted(true);
        }
        Ok(())
    }
    pub fn in_approach<S>(
        &self,
        script: Option<&Handle<E, N>>,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        let distance = script.ok_or(Error::Null)?.read().form.distance();
        let Some(destination) = self.target_destination() else {
            return Ok(false);
        };
        let location = ctx.entity_location(self, state)?.ok_or(Error::Null)?;
        let [a, b, _] = destination.coordinates();
        let [x, y, _] = location.coordinates();
        let dx = a.wrapping_sub(x);
        let dy = b.wrapping_sub(y);
        let d = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as f64;
        if d.sqrt() <= f64::from(distance) {
            return ctx.line_of_sight(&destination, self, state);
        }
        Ok(false)
    }
    pub fn in_operable<S>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<bool> {
        let Some(destination) = self.target_destination() else {
            return Ok(false);
        };
        let [x, y, z] = destination.coordinates();
        let neighbors = [(-1, 0), (0, -1), (1, 0), (0, 1)]
            .map(|(dx, dy)| Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z).coordinates());
        for neighbor in neighbors {
            if ctx
                .entity_location(self, state)?
                .is_some_and(|l| l.coordinates() == neighbor)
            {
                return ctx.line_of_sight(&destination, self, state);
            }
        }
        Ok(false)
    }
    pub fn set_interaction<S>(
        &self,
        target: Option<N>,
        script: Option<&Handle<E, N>>,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        let target = target.ok_or(Error::Null)?;
        if let (Some(existing), Some(script)) = (self.approach(), script) {
            let left = script.read().execution.clone();
            let right = existing.read().execution.clone();
            if ctx.execution_equals(left, right, self, state)? {
                return Ok(());
            }
        }
        if let (Some(existing), Some(script)) = (self.operable(), script) {
            let left = script.read().execution.clone();
            let right = existing.read().execution.clone();
            if ctx.execution_equals(left, right, self, state)? {
                return Ok(());
            }
        }
        self.reset(state, ctx)?;
        self.set_target(Some(target.clone()));
        if let Some(script) = script {
            self.set_range_raw(script.read().form.distance());
            self.set_persistent(script.read().persist);
            if self.range() == -1 {
                self.set_operable(Some(script.clone()))
            } else {
                self.set_approach(Some(script.clone()))
            }
            let destination = match ctx.node_kind(self.target().ok_or(Error::Null)?, state) {
                NodeKind::Npc => ctx.destination(
                    Destination::Entity,
                    self.target().ok_or(Error::Null)?,
                    self,
                    state,
                )?,
                NodeKind::GroundItem => ctx.destination(
                    Destination::Item,
                    self.target().ok_or(Error::Null)?,
                    self,
                    state,
                )?,
                NodeKind::Other => ctx.node_location(target, self, state)?,
                NodeKind::Scenery => {
                    let path = ctx.pathfind(self.target(), self, state)?;
                    let point = path.points.last().copied();
                    if path.move_near {
                        ctx.node_location(target, self, state)?;
                        return Ok(());
                    }
                    let Some([x, y]) = point else {
                        self.reset(state, ctx)?;
                        return Ok(());
                    };
                    let location = ctx.entity_location(self, state)?.ok_or(Error::Null)?;
                    Some(Location::new(x, y, location.coordinates()[2]))
                }
            };
            self.set_target_destination(destination);
        }
        Ok(())
    }
    fn face_and_process<S>(
        &self,
        pre: bool,
        operable: bool,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        let Some(target) = self.target() else {
            return self.reset(state, ctx);
        };
        let location = if pre {
            let own = ctx.entity_location(self, state)?;
            ctx.face_location(target, own, self, state)?
        } else {
            ctx.center_location(target, self, state)?
        };
        let Some(location) = location else {
            return self.reset(state, ctx);
        };
        ctx.face(location, self, state)?;
        let script = if operable {
            self.operable()
        } else {
            self.approach()
        };
        let Some(script) = script else {
            return self.reset(state, ctx);
        };
        self.process_interact(Some(&script), state, ctx)
    }
    pub fn pre_movement<S>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<()> {
        while !self.process_queue(state, ctx)? {}
        if ctx.stunned(self, state)? {
            return Ok(());
        }
        if ctx.delayed(self, state)? {
            return Ok(());
        }
        let mut can_process = !ctx.delayed(self, state)?;
        if ctx.entity_kind(state) == EntityKind::Player {
            can_process = can_process && !ctx.modal_open(self, state)?
        }
        if !ctx.entity_kind(state).player() {
            return Ok(());
        }
        if !ctx.delayed(self, state)? && can_process && self.target().is_some() {
            if self.operable().is_some() && self.in_operable(state, ctx)? {
                self.face_and_process(true, true, state, ctx)?
            } else if self.approach().is_some() && {
                let Some(script) = self.approach() else {
                    return self.reset(state, ctx);
                };
                self.in_approach(Some(&script), state, ctx)?
            } {
                self.face_and_process(true, false, state, ctx)?
            } else if self.approach().is_none()
                && self.operable().is_none()
                && self.in_operable(state, ctx)?
            {
                ctx.message("Nothing interesting happens.", self, state)?
            }
        }
        Ok(())
    }
    pub fn post_movement<S>(
        &self,
        did_move: bool,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        if did_move {
            let tick = ctx.ticks(self, state)?;
            let running = ctx.running(self, state)?;
            ctx.set_movement_clock(tick.wrapping_add(if running { 0 } else { 1 }), self, state)?
        }
        let mut can_process = !ctx.delayed(self, state)?;
        if ctx.entity_kind(state) == EntityKind::Player {
            can_process = can_process
                && !ctx.interface_open(self, state)?
                && !ctx.chatbox_open(self, state)?
        }
        if !ctx.entity_kind(state).player() {
            return Ok(());
        }
        if !ctx.delayed(self, state)?
            && can_process
            && self.target().is_some()
            && !self.interacted()
        {
            if self.operable().is_some() && self.in_operable(state, ctx)? {
                self.face_and_process(false, true, state, ctx)?
            } else if self.approach().is_some() && {
                let Some(script) = self.approach() else {
                    return self.reset(state, ctx);
                };
                self.in_approach(Some(&script), state, ctx)?
            } {
                self.face_and_process(false, false, state, ctx)?
            } else if self.approach().is_none()
                && self.operable().is_none()
                && self.in_operable(state, ctx)?
            {
                ctx.message("Nothing interesting happens.", self, state)?
            }
        }
        if can_process
            && (self.approach().is_some() || self.operable().is_some())
            && !self.interacted()
            && !did_move
            && ctx.finished_moving(self, state)?
        {
            ctx.message("I can't reach that!", self, state)?;
            self.reset(state, ctx)?;
        }
        if self.interacted() && !self.range_called() && !self.persistent() {
            self.reset(state, ctx)?
        }
        if let Some(target) = self.target() {
            if !ctx.node_active(target, self, state)? {
                self.reset(state, ctx)?
            }
        }
        Ok(())
    }
    pub fn delay_script<S>(
        &self,
        ticks: i32,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        if let Some(script) = self.active() {
            let next = ctx.ticks(self, state)?.wrapping_add(ticks);
            script.write().next_execution = next;
        }
        Ok(false)
    }
    pub fn delay_entity<S>(
        &self,
        ticks: i32,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        self.set_delay(ctx.ticks(self, state)?.wrapping_add(ticks));
        ctx.lock_entity(ticks, self, state)
    }
    pub fn set_range(&self, range: i32) {
        self.set_range_raw(range);
        self.set_range_called(true);
    }
    pub fn keep_running<S>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<bool> {
        self.delay_script(1, state, ctx)
    }
    pub fn restart_script<S>(
        &self,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        let script = self.active().ok_or(Error::Null)?;
        if !script.read().persist {
            let name = script.read().class_name.clone();
            ctx.log(&name,Level::Error,"Tried to call restartScript on a non-persistent script! Either use stopExecuting() or make the script persistent.".into(),self,state)?;
            self.reset(state, ctx)?;
        }
        Ok(true)
    }
    pub fn stop_executing<S>(
        &self,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        if let Some(script) = self.active() {
            if script.read().persist {
                let name = script.read().class_name.clone();
                ctx.log(&name,Level::Error,"Tried to call stopExecuting() on a persistent script! To halt execution of a persistent script, you MUST call clearScripts()!".into(),self,state)?;
                self.reset(state, ctx)?;
            }
        }
        Ok(true)
    }
    pub fn queue_script<S>(
        &self,
        execution: Option<E>,
        delay: i32,
        strength: Strength,
        persist: bool,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        let execution = execution.ok_or(Error::Null)?;
        let form = Form::Queued { strength };
        let script = Shared::new(match self.domain() {
            Some(domain) => Script::new_in(domain, form, Some(execution), persist),
            None => Script::new(form, Some(execution), persist),
        });
        let next = ctx.ticks(self, state)?.wrapping_add(delay);
        script.write().next_execution = next;
        self.add_to_queue(Some(&script), strength, state, ctx)
    }
}
impl Runtime {
    pub fn validate_in(
        &self,
        state: &crate::world::player_walk::State,
    ) -> Result<crate::world::player_walk::NodeHandle> {
        let entity = self.entity().ok_or(Error::Null)?;
        if !self.domain().is_some_and(|d| d.same(&state.identity()))
            || !state.contains_handle(&entity)
            || state.actor(entity.node).is_err()
        {
            return Err(Error::Binding(
                "script processor belongs to another entity/world".into(),
            ));
        }
        Ok(entity)
    }
    pub fn with_world<T>(
        &self,
        state: &mut crate::world::player_walk::State,
        operation: impl FnOnce(&Self, &mut crate::world::player_walk::State) -> Result<T>,
    ) -> Result<T> {
        self.validate_in(state)?;
        operation(self, state)
    }
    pub fn validate_script_in(
        &self,
        state: &crate::world::player_walk::State,
        script: &RuntimeScript,
    ) -> Result<()> {
        self.validate_in(state)?;
        let (domain, form) = {
            let f = script.read();
            (f.domain(), f.form.clone())
        };
        if !domain.is_some_and(|d| d.same(&state.identity())) {
            return Err(Error::Binding("script belongs to another world".into()));
        }
        if let Form::UseWith { used, with, .. } | Form::QueuedUseWith { used, with, .. } = form {
            if !state.contains_handle(&used) || !state.contains_handle(&with) {
                return Err(Error::Binding(
                    "script captures a foreign use target".into(),
                ));
            }
        }
        Ok(())
    }
    pub fn interact_in(
        &self,
        state: &mut crate::world::player_walk::State,
        ctx: &mut impl Context<
            crate::world::player_walk::State,
            crate::world::interaction_listeners::Function,
            crate::world::player_walk::NodeHandle,
        >,
        target: Option<crate::world::player_walk::NodeHandle>,
        script: Option<&RuntimeScript>,
    ) -> Result<()> {
        self.validate_in(state)?;
        if target.as_ref().is_some_and(|n| !state.contains_handle(n)) {
            return Err(Error::Binding("foreign script target".into()));
        }
        if let Some(script) = script {
            self.validate_script_in(state, script)?;
        }
        self.set_interaction(target, script, state, ctx)
    }
    pub fn add_in(
        &self,
        state: &mut crate::world::player_walk::State,
        ctx: &mut impl Context<
            crate::world::player_walk::State,
            crate::world::interaction_listeners::Function,
            crate::world::player_walk::NodeHandle,
        >,
        script: Option<&RuntimeScript>,
        strength: Strength,
    ) -> Result<()> {
        self.validate_in(state)?;
        if let Some(script) = script {
            self.validate_script_in(state, script)?;
        }
        self.add_to_queue(script, strength, state, ctx)
    }
    pub fn create_script(
        &self,
        form: Form<crate::world::player_walk::NodeHandle>,
        execution: Option<crate::world::interaction_listeners::Function>,
        persist: bool,
    ) -> Result<RuntimeScript> {
        let domain = self
            .domain()
            .ok_or_else(|| Error::Binding("unbound script processor domain".into()))?;
        Ok(Shared::new(Script::new_in(
            domain, form, execution, persist,
        )))
    }
}
