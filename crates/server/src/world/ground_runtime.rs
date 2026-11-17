//! Original ground packet dispatch joined to real option tables and canonical
//! legacy movement/arrival. Lookup, plugins and listeners are owning-world
//! services. A request never bypasses arrival by invoking pickup directly.
use super::{
    doors::Location,
    ground_action::{self as action, Listener, Request},
    ground_item::Handle,
    ground_manager::{Error, Result},
    ground_options::Captured,
    interaction::{Approach, OptionHandle},
    interaction_options, legacy_runtime,
    player_walk::{self as walk, Event, NodeHandle, Scripts, State},
};

pub trait Bindings: legacy_runtime::Bindings {
    /// Invoke the actual GroundItemManager.get on the owning world's chunks.
    fn ground_lookup(
        &mut self,
        state: &mut State,
        id: i32,
        location: Option<Location>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Handle>>;
    fn ground_interaction(&mut self, state: &mut State, item: &Handle) -> Result<Captured>;
    fn ground_plugin(
        &mut self,
        state: &mut State,
        player: &NodeHandle,
        item: &Handle,
        option: &OptionHandle,
    ) -> Result<bool>;
    fn ground_listeners(
        &mut self,
        state: &mut State,
        id: i32,
        kind: Listener,
        name: Option<String>,
        player: &NodeHandle,
        item: &Handle,
    ) -> Result<bool>;
}
fn error(e: impl std::fmt::Display) -> Error {
    Error::Binding(e.to_string())
}
struct Context<'a, S> {
    state: &'a mut State,
    services: &'a mut S,
}
impl<S: Bindings> action::Lookup for Context<'_, S> {
    fn action_location(&mut self, player: &NodeHandle) -> Result<Option<Location>> {
        self.state.node_location(player.node).map_err(error)
    }
    fn lookup(
        &mut self,
        id: i32,
        location: Option<Location>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Handle>> {
        self.services
            .ground_lookup(self.state, id, location, player)
    }
}
impl<S: Bindings> action::Context for Context<'_, S> {
    type Interaction = Captured;
    fn reset_scripts(&mut self, p: &NodeHandle) -> Result<()> {
        self.services
            .event(self.state, Event::Scripts(p.node, Scripts::Reset))
            .map_err(error)
    }
    fn clear_minimap(&mut self, p: &NodeHandle) -> Result<()> {
        self.services
            .event(self.state, Event::Minimap(p.node))
            .map_err(error)
    }
    fn interaction(&mut self, item: &Handle) -> Result<Option<Captured>> {
        self.services.ground_interaction(self.state, item).map(Some)
    }
    fn option(&mut self, c: &Captured, index: i32) -> Result<Option<OptionHandle>> {
        interaction_options::get(&c.table, index).map_err(|e| match e {
            interaction_options::Error::Null => Error::Null,
            interaction_options::Error::Index => Error::Index,
            e => error(e),
        })
    }
    fn invalid(&mut self, capture: &Captured, p: &NodeHandle, _: &Handle) -> Result<()> {
        validate(self.state, capture)?;
        legacy_runtime::invalid(
            self.state,
            self.services,
            Some(capture.target.node),
            Some(p.node),
            Some(capture.null_option()),
        )
        .map_err(error)
    }
    fn plugin_interaction(
        &mut self,
        p: &NodeHandle,
        item: &Handle,
        option: &OptionHandle,
    ) -> Result<bool> {
        self.services.ground_plugin(self.state, p, item, option)
    }
    fn listeners(
        &mut self,
        id: i32,
        kind: Listener,
        name: Option<String>,
        p: &NodeHandle,
        item: &Handle,
    ) -> Result<bool> {
        self.services
            .ground_listeners(self.state, id, kind, name, p, item)
    }
    fn handle(&mut self, c: &Captured, p: &NodeHandle, option: &OptionHandle) -> Result<()> {
        validate(self.state, c)?;
        legacy_runtime::handle(
            self.state,
            self.services,
            Some(c.target.node),
            Some(p.node),
            Some(option.clone()),
        )
        .map_err(error)
    }
}
fn validate(state: &mut State, c: &Captured) -> Result<()> {
    if state.retain_node(c.target.node).map_err(error)? != c.target
        || c.table.read().node() != Some(c.target.node)
    {
        return Err(error("ground interaction belongs to another world"));
    }
    state.ground(c.target.node).map_err(error)?;
    Ok(())
}
pub fn process(state: &mut State, services: &mut impl Bindings, request: &Request) -> Result<()> {
    let player = request.player.as_ref().ok_or(Error::Null)?;
    if state.retain_node(player.node).map_err(error)? != *player {
        return Err(error("ground request player belongs to another world"));
    }
    if state.actor(player.node).map_err(error)?.view.kind != super::movement::Kind::Player {
        return Err(error("ground request requires a player"));
    }
    action::process(&mut Context { state, services }, request)
}
pub fn arrival(
    state: &mut State,
    services: &mut impl Bindings,
    approach: &Approach,
) -> walk::Result<bool> {
    legacy_runtime::arrival(
        state,
        services,
        &super::legacy_interaction::Task::from(approach),
    )
    .map_err(|e| walk::Error::Binding(e.to_string()))
}
