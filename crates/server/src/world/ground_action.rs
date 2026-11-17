//! PacketProcessor.processGroundItemAction over the native ground manager.
//! Approach/arrival dispatch is a mandatory owning-world service, never an
//! immediate pickup or an ID/tile substitute for the retained selected option.
use super::{
    doors::Location,
    ground_item::{self, Handle},
    ground_manager::{Error, Result},
    interaction::OptionHandle,
    player_walk::NodeHandle,
};
use rs09_protocol::game::actions::ClientAction;

#[derive(Clone)]
pub struct Request {
    pub player: Option<NodeHandle>,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub option: i32,
}
impl Request {
    pub fn from_action(player: NodeHandle, action: &ClientAction) -> Option<Self> {
        match *action {
            ClientAction::GroundItemAction {
                opt_index,
                id,
                x,
                y,
            } => Some(Self {
                player: Some(player),
                id,
                x,
                y,
                option: opt_index,
            }),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    GroundItem,
    Item,
}
/// Only the lookup operations used by the packet body. The owning world binds
/// `lookup` to GroundItemManager.get; this keeps the packet adapter independent
/// of unrelated creation/removal services and permits canonical actor reentry.
pub trait Lookup {
    fn action_location(&mut self, player: &NodeHandle) -> Result<Option<Location>>;
    fn lookup(
        &mut self,
        id: i32,
        location: Option<Location>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Handle>>;
}
pub trait Context: Lookup {
    type Interaction: Clone;
    fn reset_scripts(&mut self, player: &NodeHandle) -> Result<()>;
    fn clear_minimap(&mut self, player: &NodeHandle) -> Result<()>;
    fn interaction(&mut self, item: &Handle) -> Result<Option<Self::Interaction>>;
    fn option(
        &mut self,
        interaction: &Self::Interaction,
        index: i32,
    ) -> Result<Option<OptionHandle>>;
    /// Invoke the native legacy invalid-interaction path with canonical Option.NULL.
    fn invalid(
        &mut self,
        interaction: &Self::Interaction,
        player: &NodeHandle,
        item: &Handle,
    ) -> Result<()>;
    fn plugin_interaction(
        &mut self,
        player: &NodeHandle,
        item: &Handle,
        option: &OptionHandle,
    ) -> Result<bool>;
    fn listeners(
        &mut self,
        id: i32,
        kind: Listener,
        name: Option<String>,
        player: &NodeHandle,
        item: &Handle,
    ) -> Result<bool>;
    fn handle(
        &mut self,
        interaction: &Self::Interaction,
        player: &NodeHandle,
        option: &OptionHandle,
    ) -> Result<()>;
}
pub fn process(ctx: &mut impl Context, request: &Request) -> Result<()> {
    let player = request.player.as_ref().ok_or(Error::Null)?;
    let plane = ctx
        .action_location(player)?
        .ok_or(Error::Null)?
        .coordinates()[2];
    let item = ctx.lookup(
        request.id,
        Some(Location::new(request.x, request.y, plane)),
        Some(player.clone()),
    )?;
    ctx.reset_scripts(player)?;
    let Some(item) = item else {
        return ctx.clear_minimap(player);
    };
    let interaction = ctx.interaction(&item)?.ok_or(Error::Null)?;
    let option = ctx.option(&interaction, request.option)?;
    let Some(option) = option else {
        ctx.invalid(&interaction, player, &item)?;
        return ctx.clear_minimap(player);
    };
    if ctx.plugin_interaction(player, &item, &option)? {
        return Ok(());
    }
    for kind in [Listener::GroundItem, Listener::Item] {
        let id = ground_item::as_item(&item).read().id();
        let name = option.read().name();
        if ctx.listeners(id, kind, name, player, &item)? {
            return Ok(());
        }
    }
    // A plugin/listener can replace this receiver without replacing the selected
    // option. Re-read only the interaction, retaining the original option object.
    let interaction = ctx.interaction(&item)?.ok_or(Error::Null)?;
    ctx.handle(&interaction, player, &option)
}
