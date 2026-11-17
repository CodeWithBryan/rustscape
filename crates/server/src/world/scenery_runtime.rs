//! Scenery packet orchestration joined to canonical options and legacy arrivals.
//! Content registries, morph/player values and the actual scene remain required
//! host bindings. Dispatch follows the original legacy scheduling and arrival.
use super::{
    doors::{Handle, Location},
    interaction::OptionHandle,
    interaction_options, legacy_runtime,
    player_walk::{self as walk, Event, Node, Scripts, State},
    scenery_action::{self as action, Error, Result},
    scenery_options::Captured,
};

pub trait Bindings: legacy_runtime::Bindings {
    fn scenery_object(
        &mut self,
        state: &mut State,
        x: i32,
        y: i32,
        z: i32,
        id: i32,
        shape: i32,
    ) -> Result<Option<Handle>>;
    fn scenery_construct(
        &mut self,
        state: &mut State,
        id: i32,
        location: Location,
    ) -> Result<Handle>;
    fn scenery_child(
        &mut self,
        state: &mut State,
        object: &Handle,
        player: Node,
    ) -> Result<Option<Handle>>;
    fn scenery_interaction(&mut self, state: &mut State, object: &Handle) -> Result<Captured>;
    fn scenery_varbit(&mut self, state: &mut State, id: i32) -> Result<Option<i32>>;
    fn scenery_listeners(
        &mut self,
        state: &mut State,
        id: i32,
        option: Option<String>,
        player: Node,
        object: &Handle,
    ) -> Result<bool>;
    fn scenery_plugin(&mut self, state: &mut State, player: Node, object: &Handle) -> Result<bool>;
}
fn error(e: impl std::fmt::Display) -> Error {
    Error::Binding(e.to_string())
}
struct Context<'a, S> {
    state: &'a mut State,
    services: &'a mut S,
}
impl<S: Bindings> action::Context for Context<'_, S> {
    type Interaction = Captured;
    type Option = OptionHandle;
    fn player_location(&mut self, p: Node) -> Result<Option<Location>> {
        self.state.node_location(p).map_err(error)
    }
    fn object(&mut self, x: i32, y: i32, z: i32, id: i32, shape: i32) -> Result<Option<Handle>> {
        self.services.scenery_object(self.state, x, y, z, id, shape)
    }
    fn construct(&mut self, id: i32, l: Location) -> Result<Handle> {
        self.services.scenery_construct(self.state, id, l)
    }
    fn child(&mut self, h: &Handle, p: Node) -> Result<Option<Handle>> {
        self.services.scenery_child(self.state, h, p)
    }
    fn interaction(&mut self, h: &Handle) -> Result<Option<Captured>> {
        self.services.scenery_interaction(self.state, h).map(Some)
    }
    fn option(&mut self, c: &Captured, i: i32) -> Result<Option<OptionHandle>> {
        interaction_options::get(&c.table, i).map_err(error)
    }
    fn option_name(o: &OptionHandle) -> Option<String> {
        o.read().name()
    }
    fn option_index(o: &OptionHandle) -> i32 {
        o.read().index()
    }
    fn varbit_definition(&mut self, id: i32) -> Result<Option<i32>> {
        self.services.scenery_varbit(self.state, id)
    }
    fn debug(&mut self, p: Node, s: &str) -> Result<()> {
        self.services
            .event(self.state, Event::Debug(p, s))
            .map_err(error)
    }
    fn invalid(&mut self, p: Node, h: Option<&Handle>) -> Result<()> {
        let target = h
            .map(|h| self.state.scenery_target(h.clone()))
            .transpose()
            .map_err(error)?;
        legacy_runtime::invalid(
            self.state,
            self.services,
            target.as_ref().map(|h| h.node),
            Some(p),
            None,
        )
        .map_err(error)
    }
    fn clear_minimap(&mut self, p: Node) -> Result<()> {
        self.services
            .event(self.state, Event::Minimap(p))
            .map_err(error)
    }
    fn reset_scripts(&mut self, p: Node) -> Result<()> {
        self.services
            .event(self.state, Event::Scripts(p, Scripts::Reset))
            .map_err(error)
    }
    fn listeners(&mut self, id: i32, o: Option<String>, p: Node, h: &Handle) -> Result<bool> {
        self.services.scenery_listeners(self.state, id, o, p, h)
    }
    fn plugin(&mut self, p: Node, h: &Handle) -> Result<bool> {
        self.services.scenery_plugin(self.state, p, h)
    }
    fn handle(&mut self, c: &Captured, p: Node, o: &OptionHandle) -> Result<()> {
        if self.state.retain_node(c.target.node).map_err(error)? != c.target {
            return Err(error("foreign scenery target"));
        }
        legacy_runtime::handle(
            self.state,
            self.services,
            Some(c.target.node),
            Some(p),
            Some(o.clone()),
        )
        .map_err(error)
    }
}
pub fn process(
    state: &mut State,
    services: &mut impl Bindings,
    request: action::Request,
) -> Result<()> {
    let _player = state
        .retain_node(request.player.ok_or(Error::Null)?)
        .map_err(error)?;
    action::process(&mut Context { state, services }, request)
}
pub fn arrival(
    state: &mut State,
    services: &mut impl Bindings,
    approach: &super::interaction::Approach,
) -> walk::Result<bool> {
    legacy_runtime::arrival(
        state,
        services,
        &super::legacy_interaction::Task::from(approach),
    )
    .map_err(|e| walk::Error::Binding(e.to_string()))
}
