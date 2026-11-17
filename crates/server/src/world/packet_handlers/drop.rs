//! `DropListener`: the "drop", "destroy" and "dissolve" item listener
//! (`handleDropAction`, `drop`), the soft queued script that drops the item
//! and the destroy dialogue's action.
use super::{Base, Container, Error, Listener, Result};
use crate::definitions::property::Property;
use crate::world::interaction_listeners::Kind;

/// The registered options.
pub const OPTIONS: [&str; 3] = ["drop", "destroy", "dissolve"];
/// `Sounds.EYEGLO_COIN_10`.
pub const DROP_COINS_SOUND: i32 = 10;
/// `Sounds.PUT_DOWN_2739`.
pub const DROP_ITEM_SOUND: i32 = 2739;
/// `Sounds.DESTROY_OBJECT_2381`.
pub const DESTROY_ITEM_SOUND: i32 = 2381;
/// `Items.COINS_995`.
pub const COINS_995: i32 = 995;
/// `ItemConfigParser.DESTROY`.
pub const DESTROY: &str = "destroy";

/// The queued script's captures.
#[derive(Clone)]
pub struct DropScript<N> {
    pub player: N,
    pub item: N,
}
/// The destroy dialogue action's captures.
#[derive(Clone)]
pub struct DestroyAction<N> {
    pub item: N,
}

pub trait Context: Base {
    /// `getUsedOption(player)`.
    fn used_option(&mut self, player: &Self::Node) -> Result<String>;
    /// `Pets.forId(id) != null`.
    fn is_pet(&mut self, id: i32) -> Result<bool>;
    /// `player.familiarManager.summon(item, pet, deposit)`.
    fn summon(
        &mut self,
        player: &Self::Node,
        item: &Self::Node,
        pet: bool,
        deposit: bool,
    ) -> Result<()>;
    /// `GraveController.hasGraveAt(player.location)`.
    fn has_grave_at(&mut self, location: Option<[i32; 3]>) -> Result<bool>;
    /// `closeAllInterfaces(player)`.
    fn close_all_interfaces(&mut self, player: &Self::Node) -> Result<()>;
    /// `queueScript(player, strength = SOFT) { ... }`: the world runs
    /// [`script`] with these captures.
    fn queue_soft_script(
        &mut self,
        player: &Self::Node,
        script: DropScript<Self::Node>,
    ) -> Result<()>;
    /// `stopExecuting(player)`.
    fn stop_executing(&mut self, player: &Self::Node) -> Result<bool>;
    /// `PlayerMonitor.log(player, LogType.DUPE_ALERT, text)`.
    fn dupe_alert(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.name`.
    fn player_name(&mut self, player: &Self::Node) -> Result<Option<String>>;
    /// `item.amount`.
    fn amount(&mut self, item: &Self::Node) -> Result<i32>;
    /// `item.name`.
    fn name(&mut self, item: &Self::Node) -> Result<Option<String>>;
    /// `item.dropItem`.
    fn drop_item(&mut self, item: &Self::Node) -> Result<Self::Node>;
    /// `GroundItemManager.create(item, location, player)`.
    fn ground_create(
        &mut self,
        item: &Self::Node,
        location: Option<[i32; 3]>,
        player: &Self::Node,
    ) -> Result<()>;
    /// `getWorldTicks()`.
    fn world_ticks(&mut self) -> i32;
    /// `setAttribute(player, key, value)`.
    fn set_attribute(&mut self, player: &Self::Node, key: &str, value: i32) -> Result<()>;
    /// `PlayerParser.save(player)`.
    fn save(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.dialogueInterpreter.sendDestroyItem(id, name)`.
    fn send_destroy_item(&mut self, player: &Self::Node, id: i32, name: Option<&str>)
    -> Result<()>;
    /// `addDialogueAction(player) { player, button -> ... }`.
    fn add_dialogue_action(
        &mut self,
        player: &Self::Node,
        action: DestroyAction<Self::Node>,
    ) -> Result<()>;
    /// `removeItem(player, item)` (the inventory).
    fn remove_item(&mut self, player: &Self::Node, item: &Self::Node) -> Result<bool>;
}

/// `defineListeners()`.
pub fn define<C: Base>(ctx: &mut C) -> Result<()> {
    ctx.on(Kind::Item, &OPTIONS, Listener::Drop)
}

/// `item.definition.handlers.getOrDefault(DESTROY, false) as Boolean`.
fn destroy_flag<C: Base>(ctx: &mut C, item: &C::Node) -> Result<bool> {
    let definition = ctx.item_definition_of(item)?;
    let d = definition.read();
    match d.handlers.get(DESTROY) {
        None => Ok(false),
        Some(Property::Bool(value)) => Ok(*value),
        Some(Property::Null) => Err(Error::Null),
        Some(_) => Err(Error::Cast),
    }
}

/// `handleDropAction(player, node)`: what the listener answers.
pub fn handle_drop_action<C: Context>(
    ctx: &mut C,
    player: &C::Node,
    node: &C::Node,
) -> Result<bool> {
    let option = ctx.used_option(player)?;
    if !ctx.node_kind(node).is_item() {
        return Ok(false);
    }
    let item = node.clone();
    if option == "drop" {
        let id = ctx.id(&item)?;
        if ctx.is_pet(id)? {
            ctx.summon(player, &item, true, true)?;
            return Ok(true);
        }
        let location = ctx.location(player)?;
        if ctx.has_grave_at(location)? {
            ctx.api_message(player, "You cannot drop items on top of graves!")?;
            return Ok(false);
        }
        if ctx.has_equipment_lock(player)? {
            return Ok(false);
        }
        ctx.close_all_interfaces(player)?;
        ctx.queue_soft_script(
            player,
            DropScript {
                player: player.clone(),
                item,
            },
        )?;
    } else if option == "destroy" || option == "dissolve" || destroy_flag(ctx, &item)? {
        let id = ctx.id(&item)?;
        let name = ctx.name(&item)?;
        ctx.send_destroy_item(player, id, name.as_deref())?;
        ctx.add_dialogue_action(player, DestroyAction { item })?;
    }
    Ok(true)
}

/// `DropListener.drop(player, item)`.
pub fn drop<C: Context>(ctx: &mut C, player: &C::Node, item: &C::Node) -> Result<bool> {
    handle_drop_action(ctx, player, item)
}

/// The queued script's body (any stage): what it answers.
pub fn script<C: Context>(ctx: &mut C, s: &DropScript<C::Node>) -> Result<bool> {
    let player = &s.player;
    let item = &s.item;
    let slot = ctx.index(item)?;
    let current = ctx.get(player, Container::Inventory, slot)?;
    if current.as_ref() != Some(item) {
        return ctx.stop_executing(player);
    }
    let slot = ctx.index(item)?;
    let replaced = ctx.replace(player, Container::Inventory, None, slot, true)?;
    if replaced.as_ref() != Some(item) {
        let name = super::or_null(ctx.player_name(player)?);
        let amount = ctx.amount(item)?;
        let id = ctx.id(item)?;
        ctx.dupe_alert(
            player,
            &format!(
                "Potential exploit attempt when player {name} tried to drop {amount}x {id}. \
                 The item has been lost and will need to be refunded to the player, but how did \
                 they get this to happen?"
            ),
        )?;
        return ctx.stop_executing(player);
    }
    let dropped = ctx.drop_item(item)?;
    let sound = if ctx.id(&dropped)? == COINS_995 {
        DROP_COINS_SOUND
    } else {
        DROP_ITEM_SOUND
    };
    ctx.audio(player, sound)?;
    let location = ctx.location(player)?;
    ctx.ground_create(&dropped, location, player)?;
    let id = ctx.id(&dropped)?;
    let ticks = ctx.world_ticks().wrapping_add(2);
    ctx.set_attribute(player, &format!("droppedItem:{id}"), ticks)?;
    ctx.save(player)?;
    ctx.stop_executing(player)
}

/// The destroy dialogue action's body.
pub fn destroy_action<C: Context>(
    ctx: &mut C,
    action: &DestroyAction<C::Node>,
    player: &C::Node,
    button: i32,
) -> Result<()> {
    if button == 3 && ctx.remove_item(player, &action.item)? {
        ctx.audio(player, DESTROY_ITEM_SOUND)?;
    }
    Ok(())
}
