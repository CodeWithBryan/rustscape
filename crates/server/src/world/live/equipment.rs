//! `EquipmentContainer.add(item, slot, fire, fromInventory)` and
//! `remove(item)` on a live player's canonical equipment container
//! (`inventory::equipment`), with the live equipment context
//! ([`super::inner::EquipmentContext`]): requirements
//! (`ItemDefinition.hasRequirement`), the container listeners (the
//! EquipmentListener's packet, bonuses, weapon tab and appearance; the
//! InventoryListener of the inventory) and the equip listeners of the
//! world's `InteractionListeners`.
use super::{P, host::Live, inner::EquipmentContext};
use crate::inventory::{self, equipment, item::Handle};

/// `player.equipment.add(item, slot, fire, fromInventory)`.
pub fn add(
    live: &mut Live,
    p: &P,
    item: &Handle,
    slot: i32,
    fire: bool,
    from_inventory: bool,
) -> inventory::Result<bool> {
    let mut container = live.inner.canonical(p).player.equipment();
    let definitions = live.inner.items.clone();
    let listeners = Some(live.item_actions.listeners.clone());
    let mut ctx = EquipmentContext {
        inner: &mut live.inner,
        p: p.clone(),
        listeners,
    };
    let result = equipment::add_at(
        &mut container,
        Some(item),
        slot,
        fire,
        from_inventory,
        &definitions,
        &mut ctx,
    );
    live.inner.drain(p);
    result
}

/// `player.equipment.remove(item)`.
pub fn remove(live: &mut Live, p: &P, item: &Handle) -> inventory::Result<bool> {
    let mut container = live.inner.canonical(p).player.equipment();
    let definitions = live.inner.items.clone();
    let listeners = Some(live.item_actions.listeners.clone());
    let mut ctx = EquipmentContext {
        inner: &mut live.inner,
        p: p.clone(),
        listeners,
    };
    let result = container.remove(Some(item), None, true, &definitions, &mut ctx);
    live.inner.drain(p);
    result
}
