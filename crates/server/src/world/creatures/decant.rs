//! `DecantListener`: Bob Barter's (any NPC's) "decant", combining potion
//! doses and kegs by hand, pouring a keg into a beer glass, and barbarian
//! decanting of a four-dose potion into an empty vial; with its
//! `DecantingDialogue`.
use super::{Host, Potion};
use crate::world::combat_swing::{Error, Result};
use crate::world::consumables::{self as consumables, Consumable, Kind};
use crate::world::dragonfire::{Container, ItemValue};

pub const VIAL: i32 = 229;
pub const BEER_GLASS: i32 = 1919;
/// `BarbarianTraining.attributeHerblore`.
pub const ATTRIBUTE_HERBLORE: &str = "/save:barbtraining:herblore";
/// The liquid sound.
pub const LIQUID_SOUND: i32 = 2401;
/// `core.tools.END_DIALOGUE`.
pub const END_DIALOGUE: i32 = 1_000_000;

/// The companion's `decantables`: every potion id, then every keg id.
pub fn decantables<H: Host>(h: &H) -> Vec<i32> {
    let mut ids = h.potion_ids();
    ids.extend(h.keg_ids());
    ids
}

/// `defineListeners()`: the catch-all NPC option, then the three use-with
/// listeners in their order.
pub fn define_listeners<H: Host>(h: &mut H) -> Result<()> {
    h.on_npc(None, &["decant"])?;
    let decantables = decantables(h);
    h.on_use_with(&decantables, &decantables)?;
    let kegs = h.keg_ids();
    h.on_use_with(&[BEER_GLASS], &kegs)?;
    let potions = h.potion_ids();
    h.on_use_with(&potions, &[VIAL])
}

/// `Potion.getDose(item)`: by the item's place in the ids, otherwise the
/// digits of its name.
pub fn dose<H: Host>(h: &H, potion: &Potion, item: &H::Item) -> Result<i32> {
    let consumable = Consumable {
        kind: Kind::Potion,
        ids: potion.ids.clone(),
        effect: None,
        animation: None,
        messages: vec![],
    };
    let value = consumables::Item::new(h.item_id(item), h.item_amount(item));
    consumable
        .dose(&value, &h.item_name(item))
        .map_err(|e| Error::Binding(format!("Potion.getDose: {e:?}")))
}

fn value<H: Host>(h: &H, item: &H::Item) -> ItemValue {
    (h.item_id(item), h.item_amount(item), h.item_charge(item))
}

/// The "decant" handler: every potion, then every keg, of the inventory
/// decanted (a refused removal answers false), then the dialogue.
pub fn decant<H: Host>(h: &mut H, player: &H::Entity, npc: Option<&H::Entity>) -> Result<bool> {
    let (remove, add) = h.decant_container(player)?;
    for item in remove {
        if !h.api_remove_item(player, item)? {
            return Ok(false);
        }
    }
    for (id, amount, _) in add {
        h.api_add_item(player, id, amount)?;
    }
    let (remove, add) = h.decant_calquat(player)?;
    for item in remove {
        if !h.api_remove_item(player, item)? {
            return Ok(false);
        }
    }
    for (id, amount, _) in add {
        h.api_add_item(player, id, amount)?;
    }
    let npc = npc.ok_or_else(|| Error::Binding("ClassCastException: (NPC) node".into()))?;
    h.open_decanting_dialogue(player, npc)?;
    Ok(true)
}

/// `DecantingDialogue.handle(componentID, buttonID)`: `when (stage++)`.
pub fn dialogue<H: Host>(h: &mut H, player: &H::Entity, stage: &mut i32) -> Result<()> {
    let current = *stage;
    *stage = stage.wrapping_add(1);
    match current {
        0 => h.dialogue_npc(player, "There you go!"),
        1 => {
            h.dialogue_player(player, "Thanks!")?;
            *stage = END_DIALOGUE;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The decantables use-with handler. `None` is a node that is not an item.
pub fn combine<H: Host>(
    h: &mut H,
    player: &H::Entity,
    used: Option<&H::Item>,
    with: Option<&H::Item>,
) -> Result<bool> {
    let (Some(used), Some(with)) = (used, with) else {
        return Ok(false);
    };
    if h.is_keg(h.item_id(used))? || h.is_keg(h.item_id(with))? {
        return h.combine_kegs(player, used, with);
    }
    let Some(potion_used) = h.potion(h.item_id(used)) else {
        return Ok(false);
    };
    let Some(potion_with) = h.potion(h.item_id(with)) else {
        return Ok(false);
    };
    if potion_used.identity != potion_with.identity {
        return Ok(false);
    }
    let used_dose = dose(h, &potion_used, used)?;
    let with_dose = dose(h, &potion_with, with)?;
    if used_dose == 4 || with_dose == 4 {
        return Ok(false);
    }
    let total = used_dose.wrapping_add(with_dose);
    let full = total / 4;
    let leftover = total % 4;
    let at = |ids: &[i32], index: i32| -> Result<i32> {
        usize::try_from(index)
            .ok()
            .and_then(|i| ids.get(i))
            .copied()
            .ok_or(Error::Index)
    };
    if full != 0 {
        let first = *potion_with.ids.first().ok_or(Error::Index)?;
        let slot = h.item_slot(with);
        let current = value(h, with);
        h.replace_slot(
            player,
            slot,
            (first, 1, 1000),
            Some(current),
            Container::Inventory,
        )?;
    }
    if leftover != 0 && full == 0 {
        let size = potion_used.ids.len() as i32;
        let id = at(&potion_used.ids, size.wrapping_sub(total))?;
        let slot = h.item_slot(with);
        let current = value(h, with);
        h.replace_slot(
            player,
            slot,
            (id, 1, 1000),
            Some(current),
            Container::Inventory,
        )?;
    } else if leftover != 0 {
        let size = potion_used.ids.len() as i32;
        let id = at(&potion_used.ids, size.wrapping_sub(leftover))?;
        let slot = h.item_slot(used);
        let current = value(h, used);
        h.replace_slot(
            player,
            slot,
            (id, 1, 1000),
            Some(current),
            Container::Inventory,
        )?;
    }
    if leftover == 0 || full == 0 {
        let slot = h.item_slot(used);
        let current = value(h, used);
        h.replace_slot(
            player,
            slot,
            (VIAL, 1, 1000),
            Some(current),
            Container::Inventory,
        )?;
    }
    let amount = if total >= 4 {
        "four"
    } else if total == 3 {
        "three"
    } else {
        "two"
    };
    h.api_message(
        player,
        &format!("You have combined the liquid into {amount} doses."),
    )?;
    h.play_audio(player, LIQUID_SOUND, 0)?;
    Ok(true)
}

/// The beer glass and keg handler.
pub fn pour<H: Host>(
    h: &mut H,
    player: &H::Entity,
    used: Option<&H::Item>,
    with: Option<&H::Item>,
) -> Result<bool> {
    let (Some(used), Some(with)) = (used, with) else {
        return Ok(false);
    };
    let (glass, keg) = if h.item_id(used) == BEER_GLASS {
        (used, with)
    } else {
        (with, used)
    };
    h.pour_into_glass(player, glass, keg)
}

/// Barbarian decanting: a four-dose potion and an empty vial become two
/// two-dose potions once the herblore training has begun.
pub fn barbarian<H: Host>(
    h: &mut H,
    player: &H::Entity,
    used: Option<&H::Item>,
    with: Option<&H::Item>,
) -> Result<bool> {
    if h.attribute_int(player, ATTRIBUTE_HERBLORE, 0)? == 0 {
        return Ok(false);
    }
    let (Some(used), Some(with)) = (used, with) else {
        return Ok(false);
    };
    let potion_item = if h.item_id(used) == VIAL { with } else { used };
    let Some(potion) = h.potion(h.item_id(potion_item)) else {
        return Ok(false);
    };
    if dose(h, &potion, potion_item)? != 4 {
        return Ok(false);
    }
    let two = *potion.ids.get(2).ok_or(Error::Index)?;
    let slot = h.item_slot(with);
    h.replace_slot(player, slot, (two, 1, 1000), None, Container::Inventory)?;
    let slot = h.item_slot(used);
    h.replace_slot(player, slot, (two, 1, 1000), None, Container::Inventory)?;
    h.play_audio(player, LIQUID_SOUND, 0)?;
    Ok(true)
}
