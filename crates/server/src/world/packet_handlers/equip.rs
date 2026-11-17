//! `EquipHandler`: the "equip", "wield" and "wear" item listener
//! (`handleEquip`), `unequip` and the registration. The equip itself is
//! `EquipmentContainer.add(item, slot, true, true)` (requirements,
//! two-handed and shield swaps, ammunition stacking, the full-inventory
//! refusal, the equipment listener and the bonus refresh), which is
//! `inventory::equipment::add_at` behind [`Context::equipment_add`].
use super::{Base, Container, Error, Listener, Result, config_int, config_plugin};
use crate::definitions::plugin;
use crate::world::interaction_listeners::Kind;

/// The registered options.
pub const OPTIONS: [&str; 3] = ["equip", "wield", "wear"];
/// `EquipmentContainer.SLOT_WEAPON`.
pub const SLOT_WEAPON: i32 = 3;
/// `ItemConfigParser.EQUIP_SLOT`.
pub const EQUIP_SLOT: &str = "equipment_slot";
/// `ItemConfigParser.EQUIP_AUDIO`.
pub const EQUIP_AUDIO: &str = "equip_audio";
/// `Sounds.EQUIP_FUN_2238`.
pub const EQUIP_FUN_2238: i32 = 2238;
/// The brawling gloves' ids.
pub const BRAWLING_GLOVES: std::ops::RangeInclusive<i32> = 13845..=13857;

/// What an equipment plugin's `fireEvent` answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fired {
    Null,
    Bool(bool),
    /// Any other object.
    Other,
}

pub trait Context: Base {
    /// `node.getName()`.
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `node.getAmount()` of an item.
    fn amount(&mut self, item: &Self::Node) -> Result<i32>;
    /// `InteractionListeners.run(id, player, node, isEquip)`.
    fn equip_listeners(
        &mut self,
        id: i32,
        player: &Self::Node,
        node: &Self::Node,
        equip: bool,
    ) -> Result<bool>;
    /// `plugin.fireEvent(name, player, item)`.
    fn fire_event(
        &mut self,
        plugin: &plugin::Handle,
        name: &str,
        player: &Self::Node,
        item: &Self::Node,
    ) -> Result<Fired>;
    /// `player.equipment.add(item, slot, fire, fromInventory)`.
    fn equipment_add(
        &mut self,
        player: &Self::Node,
        item: &Self::Node,
        slot: i32,
        fire: bool,
        from_inventory: bool,
    ) -> Result<bool>;
    /// `player.equipment.remove(item)`.
    fn equipment_remove(&mut self, player: &Self::Node, item: &Self::Node) -> Result<bool>;
    /// `player.inventory.add(item)` (its result is discarded).
    fn inventory_add(&mut self, player: &Self::Node, item: &Self::Node) -> Result<()>;
    /// `player.inventory.getMaximumAdd(item)`.
    fn maximum_add(&mut self, player: &Self::Node, item: &Self::Node) -> Result<i32>;
    /// `BrawlingGlovesManager.getInstance(player).registerGlove(id)`.
    fn register_glove(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `player.dialogueInterpreter.close()` (its result is discarded).
    fn dialogue_close(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.properties.autocastSpell != null`.
    fn has_autocast(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.properties.autocastSpell = null`.
    fn clear_autocast(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.getExtension(WeaponInterface.class) != null`.
    fn has_weapon_interface(&mut self, player: &Self::Node) -> Result<bool>;
    /// `wif.selectAutoSpell(button, adjust, forget)`.
    fn select_auto_spell(
        &mut self,
        player: &Self::Node,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> Result<()>;
    /// `wif.openAutocastSelect()`.
    fn open_autocast_select(&mut self, player: &Self::Node) -> Result<()>;
    /// `SlayerEquipmentFlags.isSlayerEq(id)`.
    fn is_slayer_equipment(&mut self, id: i32) -> Result<bool>;
    /// `SlayerEquipmentFlags.updateFlags(player)`.
    fn update_slayer_flags(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.dispatch(ItemEquipEvent(id, slot))`.
    fn dispatch_equip(&mut self, player: &Self::Node, id: i32, slot: i32) -> Result<()>;
    /// `player.dispatch(ItemUnequipEvent(id, slot))`.
    fn dispatch_unequip(&mut self, player: &Self::Node, id: i32, slot: i32) -> Result<()>;
    /// `player.packetDispatch.sendString(text, iface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()>;
}

/// `defineListeners()`.
pub fn define<C: Base>(ctx: &mut C) -> Result<()> {
    ctx.on(Kind::Item, &OPTIONS, Listener::Equip)
}

/// `node.asItem()`.
fn as_item<C: Base>(ctx: &mut C, node: &C::Node) -> Result<C::Node> {
    if ctx.node_kind(node).is_item() {
        Ok(node.clone())
    } else {
        Err(Error::Cast)
    }
}

/// The equipment lock check both methods make: true when it refused.
fn locked<C: Context>(ctx: &mut C, player: &C::Node) -> Result<bool> {
    if ctx.has_equipment_lock(player)? && ctx.equipment_locked(player)? {
        if let Some(message) = ctx.equipment_lock_message(player)? {
            // `lock.message` is read again for the send.
            let message = ctx.equipment_lock_message(player)?.unwrap_or(message);
            ctx.packet_message(player, &message)?;
        }
        return Ok(true);
    }
    Ok(false)
}

/// The listener's body: `handleEquip(player, node)`; the listener answers
/// true whatever happens.
pub fn handle_equip<C: Context>(ctx: &mut C, player: &C::Node, node: &C::Node) -> Result<()> {
    let item = as_item(ctx, node)?;
    let definition = ctx.item_definition_of(&item)?;
    let slot = config_int(&definition, EQUIP_SLOT, -1)?;
    let current = ctx.get(player, Container::Equipment, slot)?;
    if current.as_ref() == Some(&item) {
        return Ok(());
    }
    let name = ctx.name(&item)?.ok_or(Error::Null)?;
    if ctx.lowercase_default(&name).contains("goblin mail") {
        return Ok(());
    }
    if let Some(current) = &current {
        let id = ctx.id(current)?;
        if !ctx.equip_listeners(id, player, current, false)? {
            return Ok(());
        }
    }
    let definition = ctx.item_definition_of(&item)?;
    if let Some(plugin) = config_plugin(&definition, "equipment")? {
        if ctx.fire_event(&plugin, "equip", player, &item)? != Fired::Bool(true) {
            return Ok(());
        }
    }
    let id = ctx.id(node)?;
    if !ctx.equip_listeners(id, player, &item, true)? {
        return Ok(());
    }
    if locked(ctx, player)? {
        return Ok(());
    }
    let index = ctx.index(&item)?;
    if !ctx.equipment_add(player, &item, index, true, true)? {
        return Ok(());
    }
    if BRAWLING_GLOVES.contains(&ctx.id(&item)?) {
        let id = ctx.id(&item)?;
        ctx.debug(player, &format!("Registering gloves... ID: {id}"))?;
        let id = ctx.id(&item)?;
        ctx.register_glove(player, id)?;
    }
    ctx.dialogue_close(player)?;
    let definition = ctx.item_definition_of(&item)?;
    let audio = config_int(&definition, EQUIP_AUDIO, EQUIP_FUN_2238)?;
    ctx.audio(player, audio)?;
    if ctx.has_autocast(player)? && slot == SLOT_WEAPON {
        ctx.clear_autocast(player)?;
        // `wif` is a platform type: a missing extension fails at the call.
        if !ctx.has_weapon_interface(player)? {
            return Err(Error::Null);
        }
        ctx.select_auto_spell(player, -1, true, true)?;
        ctx.open_autocast_select(player)?;
    }
    let id = ctx.id(&item)?;
    if ctx.is_slayer_equipment(id)? {
        ctx.update_slayer_flags(player)?;
    }
    let id = ctx.id(&item)?;
    let index = ctx.index(&item)?;
    ctx.dispatch_equip(player, id, index)
}

/// `EquipHandler.unequip(player, slot, itemId)`.
pub fn unequip<C: Context>(ctx: &mut C, player: &C::Node, slot: i32, item_id: i32) -> Result<()> {
    if !(0..=13).contains(&slot) {
        return Ok(());
    }
    let Some(item) = ctx.get(player, Container::Equipment, slot)? else {
        return Ok(());
    };
    if locked(ctx, player)? {
        return Ok(());
    }
    if slot == SLOT_WEAPON {
        ctx.send_string(player, "", 92, 0)?;
    }
    let maximum = ctx.maximum_add(player, &item)?;
    if maximum < ctx.amount(&item)? {
        return ctx.packet_message(player, "Not enough free space in your inventory.");
    }
    let definition = ctx.item_definition_of(&item)?;
    if let Some(plugin) = config_plugin(&definition, "equipment")? {
        // `as Boolean`: null and other types fail the cast.
        match ctx.fire_event(&plugin, "unequip", player, &item)? {
            Fired::Bool(true) => {}
            Fired::Bool(false) => return Ok(()),
            Fired::Null => return Err(Error::Null),
            Fired::Other => return Err(Error::Cast),
        }
    }
    if !ctx.equip_listeners(item_id, player, &item, false)? {
        return Ok(());
    }
    if ctx.equipment_remove(player, &item)? {
        let definition = ctx.item_definition_of(&item)?;
        let audio = config_int(&definition, EQUIP_AUDIO, EQUIP_FUN_2238)?;
        ctx.audio(player, audio)?;
        ctx.dialogue_close(player)?;
        ctx.inventory_add(player, &item)?;
        ctx.dispatch_unequip(player, item_id, slot)?;
    }
    let id = ctx.id(&item)?;
    if ctx.is_slayer_equipment(id)? {
        ctx.update_slayer_flags(player)?;
    }
    Ok(())
}
