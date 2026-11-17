//! `InteractPlugin.handleItemOption` and the body of the pulse it submits
//! (`InteractPlugin$1`).
use super::{Base, Container, Error, NOTHING, Result, or_null};
use crate::world::{interaction::OptionHandle, interaction_listeners::Kind};

/// The anonymous pulse's class: `this.getClass().getName()` in its catch.
pub const CLASS: &str = "core.game.interaction.InteractPlugin$1";

/// The submitted pulse's captures: the player, the InteractPlugin's final
/// node, the option and the container (captured but unused by the body).
#[derive(Clone)]
pub struct Task<N> {
    pub player: N,
    pub node: N,
    pub option: OptionHandle,
    pub container: Container,
}

pub trait Context: Base {
    /// The InteractPlugin's final `node` field.
    fn owner(&mut self, interaction: &Self::Interaction) -> Result<Self::Node>;
    /// `player.getPulseManager().clear(PulseType.STANDARD)`.
    fn clear_pulses(&mut self, player: &Self::Node) -> Result<()>;
    /// `GameWorld.getPulser().submit(new Pulse(1, player) { ... })`: the
    /// world runs [`pulse`] with these captures when it is due.
    fn submit(&mut self, task: Task<Self::Node>) -> Result<()>;
    /// `player.getZoneMonitor().interact(node, option)`.
    fn zone_interact(
        &mut self,
        player: &Self::Node,
        node: &Self::Node,
        option: &OptionHandle,
    ) -> Result<bool>;
    /// `option.getHandler().handle(player, node, name)`.
    fn handler_handle(
        &mut self,
        handler: usize,
        player: &Self::Node,
        node: &Self::Node,
        option: &str,
    ) -> Result<bool>;
    /// `option.getHandler().getClass().getSimpleName()`.
    fn handler_name(&mut self, handler: usize) -> Result<String>;
    /// `log(origin, level, message)`.
    fn log(&mut self, origin: &str, level: &str, message: &str) -> Result<()>;
}

/// `interaction.handleItemOption(player, option, container)`.
pub fn handle_item_option<C: Context>(
    ctx: &mut C,
    interaction: &C::Interaction,
    player: &C::Node,
    option: &OptionHandle,
    container: Container,
) -> Result<()> {
    if ctx.interaction_locked(player)? {
        return Ok(());
    }
    ctx.clear_pulses(player)?;
    let node = ctx.owner(interaction)?;
    ctx.submit(Task {
        player: player.clone(),
        node,
        option: option.clone(),
        container,
    })
}

/// The submitted pulse's `pulse()`: always finished; an exception is printed
/// and logged.
pub fn pulse<C: Context>(ctx: &mut C, task: &Task<C::Node>) -> Result<bool> {
    fn body<C: Context>(ctx: &mut C, t: &Task<C::Node>) -> Result<()> {
        if ctx.interaction_locked(&t.player)? || ctx.zone_interact(&t.player, &t.node, &t.option)? {
            return Ok(());
        }
        let id = ctx.id(&t.node)?;
        let name = t.option.read().name();
        if ctx.listeners(id, Kind::Item, name.as_deref(), &t.player, &t.node)? {
            return Ok(());
        }
        let handler = t.option.read().handler;
        let handled = match handler {
            None => false,
            Some(handler) => {
                let name = t.option.read().name().ok_or(Error::Null)?;
                let lower = ctx.lowercase_default(&name);
                ctx.handler_handle(handler, &t.player, &t.node, &lower)?
            }
        };
        if !handled {
            ctx.packet_message(&t.player, NOTHING)?;
        }
        if let Some(handler) = t.option.read().handler {
            let name = ctx.handler_name(handler)?;
            ctx.debug(&t.player, &format!("Using item handler {name}"))?;
        }
        Ok(())
    }
    if let Err(error) = body(ctx, task) {
        ctx.log(
            CLASS,
            "ERR",
            &format!("{CLASS}{}", or_null(error.message())),
        )?;
    }
    Ok(true)
}
