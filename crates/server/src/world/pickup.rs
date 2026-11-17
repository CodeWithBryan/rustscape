//! Original PickupHandler.take/canTake and PickupPlugin.handle control flow.
//! Player containers, content restrictions, plugins, audio/events and attributes
//! are explicit owning-world bindings. No pickup bypasses these services.
use super::{
    doors::Location,
    ground_item::{self as ground, Handle},
    ground_manager::{self as manager, Error, Manager, Result},
    player_walk::NodeHandle,
};
use crate::inventory::item;

pub const PICKUP_SOUND: i32 = 2582;
/// Concrete native content payload stored in the typed option registry.
#[derive(Clone, Copy, Default)]
pub struct Program;
impl crate::definitions::option_handler::NativeOptionHandler for Program {}
impl Program {
    pub fn handle(
        &self,
        ctx: &mut impl Context,
        manager: &Manager,
        player: Option<&NodeHandle>,
        item: Option<&Handle>,
    ) -> Result<bool> {
        plugin_handle(ctx, manager, player, item)
    }
    pub fn destination(&self) -> Option<Location> {
        plugin_destination()
    }
}
pub fn native_handler() -> crate::definitions::option_handler::Handle {
    crate::definitions::option_handler::Handler::new(
        "content.global.handlers.item.PickupPlugin",
        Program,
    )
}
impl Registration for crate::definitions::option_handler::DefaultHandle {
    type Handler = crate::definitions::option_handler::Handle;
    fn set_option_handler(&mut self, name: &str, handler: Self::Handler) -> Result<()> {
        crate::definitions::option_handler::set_option_handler(
            self,
            Some(name.into()),
            Some(handler),
        );
        Ok(())
    }
}
pub trait Registration {
    type Handler: Clone;
    fn set_option_handler(&mut self, name: &str, handler: Self::Handler) -> Result<()>;
}
pub fn register<C: Registration>(ctx: &mut C, handler: C::Handler) -> Result<C::Handler> {
    ctx.set_option_handler("take", handler.clone())?;
    Ok(handler)
}
pub trait Context: manager::Context {
    /// Retain the actual receiver evaluated before a container operation.
    type Inventory;
    fn inventory(&mut self, player: &NodeHandle) -> Result<Self::Inventory>;
    fn has_space(&mut self, inventory: Self::Inventory, item: &item::Handle) -> Result<bool>;
    fn add(&mut self, inventory: Self::Inventory, item: &item::Handle) -> Result<bool>;
    fn drop_deadline(&mut self, player: &NodeHandle, key: &str) -> Result<i32>;
    /// This tests the GroundSpawn subtype, not the virtual isAutoSpawn result.
    fn is_spawn(&mut self, item: &Handle) -> Result<bool>;
    fn ironman_restriction(&mut self, player: &NodeHandle) -> Result<bool>;
    fn god_cape(&mut self, item: &Handle) -> Result<bool>;
    fn has_god_cape(&mut self, player: &NodeHandle) -> Result<bool>;
    fn rune_pouch(&mut self, item: &Handle) -> Result<bool>;
    fn has_item(&mut self, player: &NodeHandle, item: &Handle) -> Result<bool>;
    fn plugin_pickup(
        &mut self,
        plugin: Self::Plugin,
        player: &NodeHandle,
        item: &Handle,
        kind: i32,
    ) -> Result<bool>;
    fn packet_message(&mut self, player: &NodeHandle, text: &str) -> Result<()>;
    fn message(&mut self, player: &NodeHandle, text: &str) -> Result<()>;
    fn messages(&mut self, player: &NodeHandle, text: &[&str]) -> Result<()>;
    fn guild_dialogue(&mut self, player: &NodeHandle) -> Result<()>;
    fn item_name(&mut self, id: i32) -> Result<String>;
    fn player_name(&mut self, player: &NodeHandle) -> Result<Option<String>>;
    fn drop_trade_log(&mut self, dropper: Option<NodeHandle>, text: &str) -> Result<()>;
    fn teleport_permitted(&mut self, location: Option<Location>) -> Result<bool>;
    fn animate(&mut self, player: &NodeHandle, id: i32) -> Result<()>;
    fn audio(&mut self, player: &NodeHandle, id: i32) -> Result<()>;
    fn dispatch(&mut self, player: &NodeHandle, id: i32) -> Result<()>;
    fn pickup_attribute(&mut self, player: &NodeHandle) -> Result<bool>;
    fn remove_pickup_attribute(&mut self, player: &NodeHandle) -> Result<()>;
}
fn id(item: &Handle) -> i32 {
    item.read().item.read().id()
}
fn plugin<C: Context>(ctx: &mut C, item: &Handle) -> Result<Option<C::Plugin>> {
    let definition = item.read().item.read().definition.clone();
    match definition {
        Some(d) => ctx.plugin(&d),
        None => Ok(None),
    }
}
pub fn can_take<C: Context>(
    ctx: &mut C,
    player: Option<&NodeHandle>,
    item: Option<&Handle>,
    kind: i32,
) -> Result<bool> {
    let player = player.ok_or(Error::Null)?;
    let item = item.ok_or(Error::Null)?;
    let has_dropper = item.read().dropper.is_some();
    if has_dropper && !ground::dropped_by(ctx, item, player)? && ctx.ironman_restriction(player)? {
        return Ok(false);
    }
    if id(item) == 8858 || id(item) == 8859 {
        ctx.guild_dialogue(player)?;
        return Ok(false);
    }
    if ctx.god_cape(item)? && ctx.has_god_cape(player)? {
        ctx.messages(
            player,
            &[
                "You may only possess one sacred cape at a time.",
                "The conflicting powers of the capes drive them apart.",
            ],
        )?;
        return Ok(false);
    }
    if ctx.rune_pouch(item)? && ctx.has_item(player, item)? {
        ctx.message(
            player,
            "A mystical force prevents you from picking up the pouch.",
        )?;
        return Ok(false);
    }
    // hasItemPlugin and getPlugin are separate calls. Reentry may replace or
    // clear the plugin in between; retain the second actual receiver.
    if plugin(ctx, item)?.is_some() {
        let plugin = plugin(ctx, item)?.ok_or(Error::Null)?;
        return ctx.plugin_pickup(plugin, player, item, kind);
    }
    Ok(true)
}
pub fn take<C: Context>(
    ctx: &mut C,
    manager: &Manager,
    player: Option<&NodeHandle>,
    item: Option<&Handle>,
) -> Result<bool> {
    let player = player.ok_or(Error::Null)?;
    let item = item.ok_or(Error::Null)?;
    if item.read().location.is_none() {
        ctx.packet_message(player, "Invalid ground item!")?;
        return Ok(true);
    }
    if !manager
        .get_items()
        .read()
        .iter()
        .flatten()
        .any(|h| h.same(item))
    {
        ctx.packet_message(player, "Too late!")?;
        return Ok(true);
    }
    let deadline = ctx.drop_deadline(player, &format!("droppedItem:{}", id(item)))?;
    if deadline > ctx.ticks()? {
        return Ok(true);
    }
    if !ctx.is_spawn(item)? {
        let private = item.read().remain_private;
        if private && !ground::dropped_by(ctx, item, player)? {
            ctx.message(player, "You can't take that item!")?;
            return Ok(true);
        }
    }
    let (id, amount, charge) = {
        let g = item.read();
        let i = g.item.read();
        (i.id(), i.amount(), i.charge())
    };
    let add = ctx.copy_item(id, amount, charge)?;
    let inventory = ctx.inventory(player)?;
    if !ctx.has_space(inventory, &add)? {
        ctx.packet_message(
            player,
            "You don't have enough inventory space to hold that item.",
        )?;
        return Ok(true);
    }
    if !can_take(ctx, Some(player), Some(item), 0)? {
        return Ok(true);
    }
    if ground::is_active(ctx, item)? {
        let inventory = ctx.inventory(player)?;
        if ctx.add(inventory, &add)? {
            let has_dropper = item.read().dropper.is_some();
            if has_dropper {
                let dropper = item.read().dropper.clone().ok_or(Error::Null)?;
                let dropper_uid = ctx.uid(&dropper)?;
                let player_uid = ctx.uid(player)?;
                if dropper_uid != player_uid {
                    let dropper = item.read().dropper.clone();
                    let name = ctx.item_name(self::id(item))?;
                    let amount = item.read().item.read().amount();
                    let player_name = ctx.player_name(player)?.unwrap_or_else(|| "null".into());
                    ctx.drop_trade_log(
                        dropper,
                        &format!("{name} x{amount} picked up by {player_name}."),
                    )?;
                }
            }
            let location = item.read().location.clone();
            if !ctx.teleport_permitted(location)? {
                ctx.animate(player, 535)?;
            }
            manager::destroy(ctx, manager, Some(item.clone()))?;
            ctx.audio(player, PICKUP_SOUND)?;
            ctx.dispatch(player, self::id(item))?;
        }
    }
    Ok(true)
}
/// Java PickupPlugin does not remove the attribute in a finally block. A take
/// exception leaves it untouched, and an existing key short-circuits even when
/// its value is false/null. Its getDestination override always returns null.
pub fn plugin_handle<C: Context>(
    ctx: &mut C,
    manager: &Manager,
    player: Option<&NodeHandle>,
    item: Option<&Handle>,
) -> Result<bool> {
    let player = player.ok_or(Error::Null)?;
    if ctx.pickup_attribute(player)? {
        return Ok(false);
    }
    let result = take(ctx, manager, Some(player), item)?;
    ctx.remove_pickup_attribute(player)?;
    Ok(result)
}
pub fn plugin_destination() -> Option<Location> {
    None
}
