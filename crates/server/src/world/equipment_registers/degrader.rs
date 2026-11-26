//! `EquipmentDegrader` and the three registers that fill its static lists:
//! `BarrowsEquipmentRegister` and `PVPEquipmentRegister` (`@Initializable`
//! plugins) and `CrystalEquipmentRegister` (a start-up listener, so it
//! registers after both plugins).
//!
//! The companion's `itemList`, `setList` and `itemCharges` are process
//! state: a world keeps one [`Registry`]. `itemCharges` is only read with
//! `getOrElse` and overwritten, so its iteration order is never observed.
//! Each player's `degrader` keeps the player of the last check in its `p`
//! field; every member reads it right after the check set it, so it is the
//! check's argument here.
use super::{Host, barrows};
use crate::world::combat_swing::{Error, Result};
use std::collections::HashMap;

/// `EquipmentDegrader.StaticDegrader`'s lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    /// `itemList`: every registered id, repeats kept.
    pub item_list: Vec<i32>,
    /// `setList`: every registered set, in registration order.
    pub set_list: Vec<Vec<i32>>,
    /// `itemCharges`: the last registration of an id wins.
    pub item_charges: HashMap<i32, i32>,
}

impl Registry {
    /// `register(charges, item)`.
    pub fn register(&mut self, charges: i32, item: i32) {
        self.item_list.push(item);
        self.item_charges.insert(item, charges);
    }

    /// `registerSet(charges, items)`: each item is registered while the set's
    /// list is built.
    pub fn register_set(&mut self, charges: i32, items: &[i32]) {
        for &item in items {
            self.register(charges, item);
        }
        self.set_list.push(items.to_vec());
    }

    /// `Int.canDegrade()`.
    pub fn can_degrade(&self, id: i32) -> bool {
        self.item_list.contains(&id)
    }

    /// `itemCharges.getOrElse(id) { 1000 }`.
    pub fn charges(&self, id: i32) -> i32 {
        self.item_charges.get(&id).copied().unwrap_or(1000)
    }

    /// `getDegradableSet(item)`: the first set holding the id.
    pub fn set_of(&self, id: i32) -> Option<&Vec<i32>> {
        self.set_list.iter().find(|set| set.contains(&id))
    }

    /// The three registers in the original's order: the two plugins
    /// (`loadPluginsFrom`), then the start-up listener.
    pub fn startup() -> Self {
        let mut registry = Self::default();
        barrows_register(&mut registry);
        pvp_register(&mut registry);
        crystal_register(&mut registry);
        registry
    }
}

/// `BarrowsEquipmentRegister.newInstance`: each piece's states, 22,500
/// charges per tier.
pub fn barrows_register(registry: &mut Registry) {
    for set in barrows::all_equipment_sets() {
        registry.register_set(barrows::TICKS_PER_TIER, &set);
    }
}

/// `PVPEquipmentRegister.TICKS`: an hour of combat.
pub const PVP_TICKS: i32 = 6000;

/// `PVPEquipmentRegister`'s sets in its `newInstance` order.
pub const PVP_SETS: [[i32; 2]; 15] = [
    [13887, 13889],
    [13893, 13895],
    [13899, 13901],
    [13905, 13907],
    [13884, 13886],
    [13902, 13904],
    [13896, 13898],
    [13890, 13892],
    [13861, 13863],
    [13864, 13866],
    [13867, 13869],
    [13858, 13860],
    [13870, 13872],
    [13873, 13875],
    [13876, 13878],
];

/// `PVPEquipmentRegister.newInstance`.
pub fn pvp_register(registry: &mut Registry) {
    for set in PVP_SETS {
        registry.register_set(PVP_TICKS, &set);
    }
}

/// `CrystalEquipmentRegister.shield`.
pub const CRYSTAL_SHIELD: [i32; 12] = [
    4224, 4225, 4226, 4227, 4228, 4229, 4230, 4231, 4232, 4233, 4234, 4207,
];
/// `CrystalEquipmentRegister.bow`.
pub const CRYSTAL_BOW: [i32; 12] = [
    4212, 4214, 4215, 4216, 4217, 4218, 4219, 4220, 4221, 4222, 4223, 4207,
];

/// `CrystalEquipmentRegister.startup`: 250 charges per state.
pub fn crystal_register(registry: &mut Registry) {
    registry.register_set(250, &CRYSTAL_SHIELD);
    registry.register_set(250, &CRYSTAL_BOW);
}

/// `checkArmourDegrades(player)`: every slot from 0 to 12 but the weapon's.
pub fn check_armour_degrades<H: Host>(
    h: &mut H,
    registry: &Registry,
    player: Option<&H::Entity>,
) -> Result<()> {
    let Some(p) = player else {
        return Ok(());
    };
    for slot in 0..=12 {
        if slot == 3 {
            continue;
        }
        check_slot(h, registry, p, slot)?;
    }
    Ok(())
}

/// `checkWeaponDegrades(player)`.
pub fn check_weapon_degrades<H: Host>(
    h: &mut H,
    registry: &Registry,
    player: Option<&H::Entity>,
) -> Result<()> {
    match player {
        Some(p) => check_slot(h, registry, p, 3),
        None => Ok(()),
    }
}

fn check_slot<H: Host>(h: &mut H, registry: &Registry, p: &H::Entity, slot: i32) -> Result<()> {
    if let Some(item) = h.equipment(p, slot)?
        && registry.can_degrade(h.item_id(&item))
    {
        degrade(h, registry, p, &item, slot)?;
    }
    Ok(())
}

/// `Item.degrade(slot)`. The item's id and charge are the packed `idHash`,
/// read again after every charge write.
pub fn degrade<H: Host>(
    h: &mut H,
    registry: &Registry,
    p: &H::Entity,
    item: &H::Item,
    slot: i32,
) -> Result<()> {
    let set = registry.set_of(h.item_id(item)).cloned();
    let charges = registry.charges(h.item_id(item));
    if h.item_charge(item) > charges {
        h.set_item_charge(item, charges);
    }
    let charge = h.item_charge(item).wrapping_sub(1);
    h.set_item_charge(item, charge);
    let first = set
        .as_ref()
        .is_some_and(|s| s.first() == Some(&h.item_id(item)));
    if first && !h.item_name(item).contains("100") {
        h.set_item_charge(item, 0);
    }
    if h.item_charge(item) > 0 {
        return Ok(());
    }
    let charges = registry.charges(h.item_id(item));
    let set = set.ok_or(Error::Null)?;
    if set.len() > 2 {
        h.equipment_remove(p, item)?;
        let text = format!("Your {} has degraded.", h.item_name(item));
        h.player_send_message(p, &text)?;
        if is_next_last(&set, h.item_id(item)) {
            let next = next_of(&set, h.item_id(item));
            h.add_item_or_drop(p, next, 1)?;
            let text = format!("Your {} has broken.", h.item_name(item));
            h.player_send_message(p, &text)?;
        } else {
            let next = next_of(&set, h.item_id(item));
            h.equipment_add(p, (next, 1, charges), slot, false, false)?;
            h.equipment_refresh(p)?;
        }
    } else if set.len() == 2 {
        if is_last(&set, h.item_id(item)) {
            h.equipment_remove(p, item)?;
            let text = format!("Your {} degrades into dust.", h.item_name(item));
            h.player_send_message(p, &text)?;
        } else {
            h.equipment_remove(p, item)?;
            let text = format!("Your {} has degraded.", h.item_name(item));
            h.player_send_message(p, &text)?;
            let next = next_of(&set, h.item_id(item));
            h.equipment_add(p, (next, 1, charges), slot, false, false)?;
            h.equipment_refresh(p)?;
        }
    }
    Ok(())
}

fn index_of(set: &[i32], id: i32) -> i32 {
    set.iter().position(|x| *x == id).map_or(-1, |i| i as i32)
}

/// ApiExtensions `Collection.isLast(element)`.
pub fn is_last(set: &[i32], id: i32) -> bool {
    index_of(set, id) == set.len() as i32 - 1
}

/// ApiExtensions `Collection.getNext(element)`: the element after the
/// first match, or the element itself when it is last or absent at the end
/// (an absent element's index -1 answers the first element).
pub fn next_of(set: &[i32], id: i32) -> i32 {
    let idx = index_of(set, id);
    if idx < set.len() as i32 - 1 {
        set[(idx + 1) as usize]
    } else {
        id
    }
}

/// ApiExtensions `Collection.isNextLast(element)`.
pub fn is_next_last(set: &[i32], id: i32) -> bool {
    is_last(set, next_of(set, id))
}
