//! Content script invocation in the live world: `ScriptProcessor` bodies
//! (`defineInteraction` handlers and the scripts they queue) run with the
//! whole world.
//!
//! A player's `Entity.tick` runs on the canonical state with
//! [`ScriptHost`] as its services. Every walk service is
//! [`super::walk::WalkHost`]'s, made over the same parts of the world for
//! the call; the script phases (`preMovement`, `postMovement`) run with the
//! host itself as `script_runtime::Bindings`, so a script's execution
//! reaches [`Live::content_script`] where the original invokes the lambda:
//! inside the phase, with its answer deciding what the processor does next.
//!
//! The canonical state is taken out of the world for the tick (the walk
//! kernel holds it) and put back around each script body, which sees the
//! world as every other content body does.
//!
//! Installed bodies: the gathering listeners' interaction scripts and the
//! scripts they queue ([`super::items::woodcutting`]). Anything else is
//! refused by name: where `InteractionListeners.run` would make its script
//! (`Live::script_installed`), and here.
use super::{
    P,
    host::Live,
    inner::Inner,
    walk::{self, WalkHost},
};
use crate::{
    pulses,
    scripts::{self, Arguments, Level, Strength},
    world::{
        component_objects::NativeObject,
        doors::Location,
        interaction::Approach,
        interaction_listeners::{Function, FunctionKind},
        legacy_interaction,
        movement::DestinationService,
        pathfinding::dispatch,
        player_walk::{self, Event, Node, NodeHandle, Query, Services, State},
        script_runtime::{self, Bindings},
        walk_request::Interfaces,
    },
};

/// A `defineInteraction` handler this world runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// `WoodcuttingListener::handleWoodcutting`.
    Woodcutting,
}
impl Interaction {
    pub fn class(self) -> &'static str {
        match self {
            Self::Woodcutting => {
                "content.global.skill.gather.woodcutting.WoodcuttingListener$defineListeners$1"
            }
        }
    }
    /// The handler as the registry's function object.
    pub fn function(self) -> Function {
        Function::new(FunctionKind::Interaction, self.class(), self)
    }
}
impl NativeObject for Interaction {}

/// A script a content body queued (`queueScript(player, delay, strength)`),
/// with what its lambda captured.
pub enum Queued {
    /// A woodcutting body: the task and the tree it names.
    Woodcutting(
        crate::world::gathering::service::Task,
        crate::world::doors::Handle,
    ),
}
impl NativeObject for Queued {}

/// What a content body asks of the player's `ScriptProcessor`.
pub enum ScriptCall {
    /// `delayScript(player, ticks)`.
    Delay(i32),
    /// `keepRunning(player)`.
    KeepRunning,
    /// `restartScript(player)`.
    Restart,
    /// `stopExecuting(player)`.
    Stop,
    /// `clearScripts(player)`.
    Clear,
    /// `queueScript(player, delay, strength, persist, script)`.
    Queue(Function, i32, Strength, bool),
}

fn refused(what: &str) -> scripts::Error {
    scripts::Error::Binding(format!("not ported in the live world: {what}"))
}

/// The walk services and script bindings of a player's `Entity.tick`, with
/// the whole world behind the script bodies.
pub struct ScriptHost<'a> {
    live: &'a mut Live,
    lines: Vec<String>,
}

impl ScriptHost<'_> {
    /// The walk host over the world's parts, as `Live::with_walk_host`
    /// makes it.
    fn host(&mut self) -> WalkHost<'_> {
        let Live {
            inner,
            walk,
            deferred,
            npc_views,
            ..
        } = &mut *self.live;
        let Inner {
            frames,
            players,
            not_ported,
            npcs,
            generators,
            combat,
            ..
        } = inner;
        WalkHost {
            walk,
            frames,
            players,
            not_ported,
            deferred,
            log: &mut self.lines,
            npcs,
            random: generators,
            npc_views: Some(npc_views),
            combat,
        }
    }
}

impl Services for ScriptHost<'_> {
    fn query(&mut self, state: &mut State, query: Query<'_>) -> player_walk::Result<bool> {
        self.host().query(state, query)
    }
    fn event(&mut self, state: &mut State, event: Event<'_>) -> player_walk::Result<()> {
        if let Event::Scripts(node, phase) = event {
            // The processor's phase with this host: a script's execution
            // reaches `invoke` below.
            let owner = state.retain_node(node)?;
            return script_runtime::phase(state, self, &owner, phase)
                .map_err(|e| player_walk::Error::Binding(format!("{e:?}")));
        }
        self.host().event(state, event)
    }
    fn interfaces(&self, _: &State, node: Node) -> player_walk::Result<Interfaces> {
        walk::interfaces_of(&self.live.inner.players, node)
    }
    fn region(&mut self, state: &mut State, l: &Location) -> player_walk::Result<Option<bool>> {
        self.host().region(state, l)
    }
    fn clipping(&mut self, state: &mut State, z: i32, x: i32, y: i32) -> player_walk::Result<i32> {
        self.host().clipping(state, z, x, y)
    }
    fn water_clipping(
        &mut self,
        state: &mut State,
        z: i32,
        x: i32,
        y: i32,
    ) -> player_walk::Result<i32> {
        self.host().water_clipping(state, z, x, y)
    }
    fn now_millis(&mut self) -> i64 {
        self.host().now_millis()
    }
    fn metrics_enabled(&self) -> bool {
        walk::METRICS_ENABLED
    }
    fn external_pulse(
        &mut self,
        state: &mut State,
        pulse: &pulses::Handle,
    ) -> pulses::Result<bool> {
        self.host().external_pulse(state, pulse)
    }
    fn external_inactive(
        &mut self,
        state: &mut State,
        pulse: &pulses::Handle,
    ) -> pulses::Result<bool> {
        self.host().external_inactive(state, pulse)
    }
    fn external_stop(&mut self, state: &mut State, pulse: &pulses::Handle) -> pulses::Result<()> {
        self.host().external_stop(state, pulse)
    }
    fn npc_finder(&mut self, state: &mut State, node: Node) -> player_walk::Result<Option<usize>> {
        self.host().npc_finder(state, node)
    }
    fn npc_supplier(
        &mut self,
        state: &mut State,
        node: Node,
    ) -> player_walk::Result<Option<dispatch::Supplier>> {
        self.host().npc_supplier(state, node)
    }
    fn npc_arrival(&mut self, state: &mut State, node: Node) -> player_walk::Result<bool> {
        self.host().npc_arrival(state, node)
    }
    fn node_active(&mut self, state: &mut State, node: Node) -> player_walk::Result<bool> {
        self.host().node_active(state, node)
    }
    fn interaction_arrival(
        &mut self,
        state: &mut State,
        approach: &Approach,
    ) -> player_walk::Result<bool> {
        self.host().interaction_arrival(state, approach)
    }
    fn delayed_interaction(
        &mut self,
        state: &mut State,
        task: &legacy_interaction::Task,
    ) -> player_walk::Result<bool> {
        self.host().delayed_interaction(state, task)
    }
    fn destination(
        &mut self,
        state: &mut State,
        mover: Option<Node>,
        target: Option<Node>,
        service: DestinationService,
    ) -> player_walk::Result<Option<Location>> {
        self.host().destination(state, mover, target, service)
    }
    fn door_destination(
        &mut self,
        state: &mut State,
        mover: Option<Node>,
        target: Option<Node>,
    ) -> player_walk::Result<Option<Location>> {
        self.host().door_destination(state, mover, target)
    }
    fn teleport_permitted(
        &mut self,
        state: &mut State,
        location: Option<Location>,
    ) -> player_walk::Result<bool> {
        self.host().teleport_permitted(state, location)
    }
}

impl Bindings for ScriptHost<'_> {
    fn modal_open(&mut self, state: &mut State, owner: &NodeHandle) -> scripts::Result<bool> {
        self.host().modal_open(state, owner)
    }
    fn interface_open(&mut self, state: &mut State, owner: &NodeHandle) -> scripts::Result<bool> {
        self.host().interface_open(state, owner)
    }
    fn chatbox_open(&mut self, state: &mut State, owner: &NodeHandle) -> scripts::Result<bool> {
        self.host().chatbox_open(state, owner)
    }
    fn close_interfaces(&mut self, state: &mut State, owner: &NodeHandle) -> scripts::Result<()> {
        self.host().close_interfaces(state, owner)
    }
    fn reset_animator(&mut self, state: &mut State, owner: &NodeHandle) -> scripts::Result<()> {
        self.host().reset_animator(state, owner)
    }
    fn node_description(
        &mut self,
        state: &mut State,
        node: Option<NodeHandle>,
    ) -> scripts::Result<String> {
        self.host().node_description(state, node)
    }
    fn face_location(
        &mut self,
        state: &mut State,
        node: NodeHandle,
        from: Option<Location>,
    ) -> scripts::Result<Option<Location>> {
        self.host().face_location(state, node, from)
    }
    fn center_location(
        &mut self,
        state: &mut State,
        node: NodeHandle,
    ) -> scripts::Result<Option<Location>> {
        self.host().center_location(state, node)
    }
    fn line_of_sight(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        location: &Location,
    ) -> scripts::Result<bool> {
        self.host().line_of_sight(state, owner, location)
    }
    fn face(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        location: Location,
    ) -> scripts::Result<()> {
        self.host().face(state, owner, location)
    }
    fn execution_equals(
        &mut self,
        state: &mut State,
        left: Option<Function>,
        right: Option<Function>,
    ) -> scripts::Result<bool> {
        self.host().execution_equals(state, left, right)
    }
    fn invoke(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        execution: Option<Function>,
        arguments: Arguments<NodeHandle>,
    ) -> scripts::Result<bool> {
        // The lambda's call: the canonical state goes back into the world
        // for the body and comes out again for the rest of the phase.
        let function = execution.ok_or(scripts::Error::Null)?;
        std::mem::swap(&mut self.live.inner.state, state);
        let result = self.live.content_script(owner, &function, arguments);
        std::mem::swap(&mut self.live.inner.state, state);
        result
    }
    fn exception_text(
        &mut self,
        state: &mut State,
        error: &scripts::Error,
    ) -> scripts::Result<String> {
        self.host().exception_text(state, error)
    }
    fn log(
        &mut self,
        state: &mut State,
        owner: &str,
        level: Level,
        text: String,
    ) -> scripts::Result<()> {
        self.host().log(state, owner, level, text)
    }
    fn lock_entity(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        ticks: i32,
    ) -> scripts::Result<()> {
        self.host().lock_entity(state, owner, ticks)
    }
}

impl Live {
    /// Runs a walking operation of a player's tick on the canonical state
    /// with the whole world behind its script bodies, then what the walk
    /// services deferred.
    pub fn with_script_host<T>(
        &mut self,
        f: impl FnOnce(&mut player_walk::World<'_, ScriptHost<'_>>) -> T,
    ) -> T {
        let spare = self.spare_state.take().unwrap_or_default();
        let mut state = std::mem::replace(&mut self.inner.state, spare);
        let mut host = ScriptHost {
            live: self,
            lines: Vec::new(),
        };
        let result = f(&mut state.context(&mut host));
        let lines = host.lines;
        self.spare_state = Some(std::mem::replace(&mut self.inner.state, state));
        self.after_walk(lines);
        result
    }

    /// Whether a `defineInteraction` handler is one this world runs.
    pub fn script_installed(function: &Function) -> bool {
        function.value::<Interaction>().is_some()
    }

    /// A script's execution: the handler or queued lambda by identity.
    pub fn content_script(
        &mut self,
        owner: &NodeHandle,
        function: &Function,
        arguments: Arguments<NodeHandle>,
    ) -> scripts::Result<bool> {
        if !self.inner.players.contains_key(&owner.node) {
            return Err(refused(
                "a content script of an entity that is not a player",
            ));
        }
        let p = P(owner.clone());
        let result = match arguments {
            Arguments::Interaction { target, stage } => {
                match function.value::<Interaction>().as_deref() {
                    Some(Interaction::Woodcutting) => self.woodcutting_script(&p, &target, stage),
                    None => Err(refused("an interaction script this world did not install")),
                }
            }
            Arguments::Void { stage } => match function.value::<Queued>().as_deref() {
                Some(Queued::Woodcutting(task, tree)) => {
                    self.woodcutting_queued(&p, task, tree, stage)
                }
                None => Err(refused("a queued script this world did not queue")),
            },
            Arguments::UseWith { .. } => Err(refused("use-with interaction scripts")),
        };
        self.inner.drain(&p);
        result
    }

    /// A call of a content body into its player's `ScriptProcessor`.
    pub fn script_call(&mut self, p: &P, call: ScriptCall) -> scripts::Result<bool> {
        let owner = p.0.clone();
        self.with_walk_host(|state, host| {
            let processor = state.scripts(&owner).map_err(script_runtime::error)?;
            let mut ctx = script_runtime::Context::new(state, host, &processor)?;
            match call {
                ScriptCall::Delay(ticks) => processor.delay_script(ticks, state, &mut ctx),
                ScriptCall::KeepRunning => processor.keep_running(state, &mut ctx),
                ScriptCall::Restart => processor.restart_script(state, &mut ctx),
                ScriptCall::Stop => processor.stop_executing(state, &mut ctx),
                ScriptCall::Clear => {
                    processor.reset(state, &mut ctx)?;
                    Ok(true)
                }
                ScriptCall::Queue(function, delay, strength, persist) => {
                    processor.queue_script(
                        Some(function),
                        delay,
                        strength,
                        persist,
                        state,
                        &mut ctx,
                    )?;
                    Ok(true)
                }
            }
        })
    }

    /// A clock of the player (`Clocks`' slot).
    pub fn script_clock(&mut self, p: &P, clock: usize) -> scripts::Result<i32> {
        let clocks = self
            .inner
            .state
            .clocks(&p.0)
            .map_err(script_runtime::error)?;
        let value = clocks.read().get(clock).copied();
        value.ok_or(scripts::Error::Index)
    }
    /// Writes a clock of the player.
    pub fn set_script_clock(&mut self, p: &P, clock: usize, value: i32) -> scripts::Result<()> {
        let clocks = self
            .inner
            .state
            .clocks(&p.0)
            .map_err(script_runtime::error)?;
        *clocks.write().get_mut(clock).ok_or(scripts::Error::Index)? = value;
        Ok(())
    }
}
