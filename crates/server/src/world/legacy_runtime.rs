//! Original interaction bodies bound to the canonical owning-world scheduler.
//! Profile locks, handler registry, zones, masks and event/log delivery belong to
//! the host. Their callbacks may reenter the same world without held node locks.
use super::{
    doors::Location,
    interaction::{Approach, ApproachKind, OptionHandle},
    legacy_interaction::{self as legacy, Error, Result, Task},
    movement::Movement,
    player_walk::{self as walk, Node, NodeHandle, State, World},
};
use crate::pulses;

pub trait Bindings: walk::Services {
    fn interaction_locked(&mut self, state: &mut State, player: Option<Node>) -> Result<bool>;
    fn handler_walk(&mut self, state: &mut State, handler: usize) -> Result<bool>;
    fn handler_walk_for(
        &mut self,
        state: &mut State,
        handler: usize,
        player: Option<Node>,
        target: Option<Node>,
    ) -> Result<bool>;
    fn handler_delayed(
        &mut self,
        state: &mut State,
        handler: usize,
        player: Option<Node>,
    ) -> Result<bool>;
    fn handler_name(&mut self, state: &mut State, handler: usize) -> Result<String>;
    fn handler_handle(
        &mut self,
        state: &mut State,
        handler: usize,
        player: Option<Node>,
        target: Option<Node>,
        option: &str,
    ) -> Result<bool>;
    fn lowercase(&self, text: &str) -> String;
    fn zone(
        &mut self,
        state: &mut State,
        player: Option<Node>,
        target: Option<Node>,
        option: Option<&OptionHandle>,
    ) -> Result<bool>;
    fn combat_stop(&mut self, state: &mut State, player: Option<Node>) -> Result<()>;
    fn face_player(
        &mut self,
        state: &mut State,
        player: Option<Node>,
        location: Option<Location>,
    ) -> Result<()>;
    fn dispatch(
        &mut self,
        state: &mut State,
        player: Option<Node>,
        target: Option<Node>,
        option: &str,
    ) -> Result<()>;
    fn exception(&mut self, state: &mut State, error: &Error) -> Result<()>;
    fn error_message(&mut self, _: &mut State, error: &Error) -> Result<Option<String>> {
        Ok(match error {
            Error::Null => None,
            Error::Binding(message) => message.clone(),
        })
    }
    fn log(&mut self, state: &mut State, class: &str, message: &str) -> Result<()>;
}
fn error(e: walk::Error) -> Error {
    match e {
        walk::Error::Null => Error::Null,
        walk::Error::Binding(s) => Error::Binding(Some(s)),
        _ => Error::Binding(Some(e.to_string())),
    }
}
/// A manager receiver and its owning node are retained before construction.
#[derive(Clone)]
pub struct CapturedManager {
    owner: NodeHandle,
    manager: pulses::Manager,
}
pub enum Pulse {
    Movement(Movement),
    Plain(pulses::Handle),
}
pub struct Context<'a, S: Bindings> {
    world: World<'a, S>,
    _captures: Vec<NodeHandle>,
}
impl<'a, S: Bindings> Context<'a, S> {
    pub fn new(
        state: &'a mut State,
        services: &'a mut S,
        player: Option<Node>,
        target: Option<Node>,
    ) -> Result<Self> {
        let mut captures = Vec::new();
        for node in [player, target].into_iter().flatten() {
            captures.push(state.retain_node(node).map_err(error)?);
        }
        Ok(Self {
            world: state.context(services),
            _captures: captures,
        })
    }
    fn owner(&mut self, manager: &CapturedManager) -> Result<Node> {
        let node = manager.owner.node;
        if self.world.state.retain_node(node).map_err(error)? != manager.owner {
            return Err(Error::Binding(Some("manager from another world".into())));
        }
        Ok(node)
    }
}
impl<S: Bindings> legacy::Context for Context<'_, S> {
    type Manager = CapturedManager;
    type Pulse = Pulse;
    fn interaction_locked(&mut self, player: Option<Node>) -> Result<bool> {
        self.world
            .services
            .interaction_locked(self.world.state, player)
    }
    fn movement_locked(&mut self, player: Option<Node>) -> Result<bool> {
        self.world
            .services
            .query(
                self.world.state,
                walk::Query::Locked(player.ok_or(Error::Null)?),
            )
            .map_err(error)
    }
    fn location(&mut self, node: Option<Node>) -> Result<Option<Location>> {
        self.world
            .state
            .node_location(node.ok_or(Error::Null)?)
            .map_err(error)
    }
    fn debug(&mut self, player: Option<Node>, text: &str) -> Result<()> {
        self.world
            .services
            .event(
                self.world.state,
                walk::Event::Debug(player.ok_or(Error::Null)?, text),
            )
            .map_err(error)
    }
    fn handler_walk(&mut self, handler: usize) -> Result<bool> {
        self.world.services.handler_walk(self.world.state, handler)
    }
    fn handler_walk_for(
        &mut self,
        handler: usize,
        player: Option<Node>,
        target: Option<Node>,
    ) -> Result<bool> {
        self.world
            .services
            .handler_walk_for(self.world.state, handler, player, target)
    }
    fn handler_delayed(&mut self, handler: usize, player: Option<Node>) -> Result<bool> {
        self.world
            .services
            .handler_delayed(self.world.state, handler, player)
    }
    fn handler_name(&mut self, handler: usize) -> Result<String> {
        self.world.services.handler_name(self.world.state, handler)
    }
    fn handler_handle(
        &mut self,
        handler: usize,
        player: Option<Node>,
        target: Option<Node>,
        option: &str,
    ) -> Result<bool> {
        self.world
            .services
            .handler_handle(self.world.state, handler, player, target, option)
    }
    fn lowercase(&self, text: &str) -> String {
        self.world.services.lowercase(text)
    }
    fn manager(&mut self, player: Option<Node>) -> Result<Option<Self::Manager>> {
        let player = player.ok_or(Error::Null)?;
        let manager = self
            .world
            .state
            .actor(player)
            .map_err(error)?
            .manager
            .clone();
        let owner = self.world.state.retain_node(player).map_err(error)?;
        Ok(Some(CapturedManager { owner, manager }))
    }
    fn movement(&mut self, task: Task, handler: Option<usize>) -> Result<Self::Pulse> {
        let kind = match task.kind {
            legacy::Kind::Walk => ApproachKind::Option,
            legacy::Kind::Invalid => ApproachKind::Invalid,
            legacy::Kind::Delayed => {
                return Err(Error::Binding(Some("delayed movement constructor".into())));
            }
        };
        let approach = Approach {
            player: task.player.ok_or(Error::Null)?,
            target: task.target,
            option: task.option,
            kind,
        };
        self.world
            .interaction_motion_captured(approach, handler)
            .map(Pulse::Movement)
            .map_err(error)
    }
    fn delayed(&mut self, task: Task) -> Result<Self::Pulse> {
        self.world
            .delayed_interaction(task)
            .map(Pulse::Plain)
            .map_err(error)
    }
    fn run(&mut self, manager: Self::Manager, pulse: Self::Pulse) -> Result<()> {
        let node = self.owner(&manager)?;
        match pulse {
            Pulse::Movement(p) => self.world.run_motion_captured(node, manager.manager, p),
            Pulse::Plain(p) => self.world.run_task_captured(node, manager.manager, &p),
        }
        .map_err(error)
    }
    fn clear(&mut self, manager: Self::Manager) -> Result<()> {
        let node = self.owner(&manager)?;
        self.world
            .clear_standard(node, manager.manager)
            .map_err(error)
    }
    fn unhandled(&mut self, manager: Self::Manager, player: Option<Node>) -> Result<()> {
        let node = self.owner(&manager)?;
        self.world
            .unhandled_captured(node, manager.manager, player.ok_or(Error::Null)?)
            .map_err(error)
    }
    fn zone(
        &mut self,
        player: Option<Node>,
        target: Option<Node>,
        option: Option<&OptionHandle>,
    ) -> Result<bool> {
        self.world
            .services
            .zone(self.world.state, player, target, option)
    }
    fn combat_stop(&mut self, player: Option<Node>) -> Result<()> {
        self.world.services.combat_stop(self.world.state, player)
    }
    fn player_location(&mut self, player: Option<Node>) -> Result<Option<Location>> {
        self.world
            .state
            .node_location(player.ok_or(Error::Null)?)
            .map_err(error)
    }
    fn face_location(
        &mut self,
        target: Option<Node>,
        from: Option<Location>,
    ) -> Result<Option<Location>> {
        let target = target.ok_or(Error::Null)?;
        let location = self
            .world
            .state
            .node_location(target)
            .map_err(error)?
            .ok_or(Error::Null)?;
        let size = self.world.state.node_size(target).map_err(error)?;
        Ok(Some(legacy::node_face_location(
            &location,
            size,
            &from.ok_or(Error::Null)?,
        )))
    }
    fn face_player(&mut self, player: Option<Node>, location: Option<Location>) -> Result<()> {
        self.world
            .services
            .face_player(self.world.state, player, location)
    }
    fn message(&mut self, player: Option<Node>, text: &str) -> Result<()> {
        self.world
            .services
            .event(
                self.world.state,
                walk::Event::Message(player.ok_or(Error::Null)?, text),
            )
            .map_err(error)
    }
    fn dispatch(&mut self, player: Option<Node>, target: Option<Node>, option: &str) -> Result<()> {
        self.world
            .services
            .dispatch(self.world.state, player, target, option)
    }
    fn exception(&mut self, e: &Error) -> Result<()> {
        self.world.services.exception(self.world.state, e)
    }
    fn error_message(&mut self, e: &Error) -> Result<Option<String>> {
        self.world.services.error_message(self.world.state, e)
    }
    fn log(&mut self, class: &str, message: &str) -> Result<()> {
        self.world.services.log(self.world.state, class, message)
    }
}
pub fn handle<S: Bindings>(
    state: &mut State,
    services: &mut S,
    target: Option<Node>,
    player: Option<Node>,
    option: Option<OptionHandle>,
) -> Result<()> {
    legacy::handle(
        &mut Context::new(state, services, player, target)?,
        target,
        player,
        option,
    )
}
pub fn invalid<S: Bindings>(
    state: &mut State,
    services: &mut S,
    target: Option<Node>,
    player: Option<Node>,
    option: Option<OptionHandle>,
) -> Result<()> {
    legacy::invalid(
        &mut Context::new(state, services, player, target)?,
        target,
        player,
        option,
    )
}
pub fn arrival<S: Bindings>(state: &mut State, services: &mut S, task: &Task) -> Result<bool> {
    legacy::arrival(
        &mut Context::new(state, services, task.player, task.target)?,
        task,
    )
}
