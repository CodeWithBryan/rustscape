//! `Player.commenceDeath`, `Player.finalizeDeath` and `DeathInterface`.
use super::{
    BONES, COINS, Controller, DEATH_SCREEN, DEATHS, Host, MAGIC, ORIGINAL_LOCATION, Prayer, Result,
    Rights, SAFE_ZONE, barrows, colorize, grave, kept, null,
};
use crate::{
    definitions::item::{Registry, Shared},
    inventory::{
        container::Factory,
        item::{self, Item, definition_alchemy, definition_tradeable},
    },
    player::ironman::Mode,
    world::{
        ground_manager as gm,
        npc_death::{self, Attribute, COMBAT_TIME},
        player_walk::NodeHandle,
    },
};

/// `Player.commenceDeath(killer)`: nothing for a player who is not playing;
/// `Entity.commenceDeath` is empty; Retribution starts when it is on.
pub fn commence_death<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    killer: Option<&NodeHandle>,
) -> Result<()> {
    if !host.playing(player)? {
        return Ok(());
    }
    if host.prayer_active(player, Prayer::Retribution)? {
        host.start_retribution(player, killer)?;
    }
    Ok(())
}

/// `item.getDefinition()`.
fn definition(item: &item::Handle) -> Result<crate::definitions::item::Handle> {
    item.read().definition.clone().ok_or_else(null)
}

/// `Player.finalizeDeath(killer)`.
///
/// Nothing happens for a player who is not playing. Otherwise: the death
/// counter, special energy 100, run energy lowered by 100, the item owner
/// `k` (the killer when it is an active player, else the player), the
/// `dead` attribute for an artificial player killed by a player and an early
/// return for one killed by an NPC, the defeat message, "Oh dear, you are
/// dead!", the death statistic and the death music. Items move only when the
/// zone does not handle the death, the player is not in a safe zone (flag or
/// zone type) and is not an administrator: a hardcore ironman with the
/// `permadeath` attribute gets the news line and the permadeath reset and the
/// method ends; otherwise bones drop at the original location (or the
/// player's) for `k`, the kept items are chosen, every worn item runs its
/// unequip listeners and equipment plugin, and the lost items go into a grave
/// or, when no grave is allowed, onto the ground (see the original order in
/// the body). Equipment and inventory are cleared, the kept items return to
/// the inventory and the familiar is dismissed. In every case the skull,
/// combat time, prayers, `original-loc`, default tabs and logout buttons are
/// reset, `Entity.finalizeDeath` runs, the appearance is synced and the death
/// screen opens unless disabled.
pub fn finalize_death<H: Host>(
    host: &mut H,
    controller: &mut Controller,
    player: &NodeHandle,
    killer: Option<&NodeHandle>,
    factory: &Factory,
    definitions: &Registry,
) -> Result<()> {
    if !host.playing(player)? {
        return Ok(());
    }
    host.increment_death_count()?;
    host.set_special_energy(player, 100)?;
    let energy = host.run_energy(player)?;
    host.update_run_energy(player, energy - 100.0)?;
    let killer_player = killer.filter(|killer| host.is_player(killer)).cloned();
    let mut k = killer_player.clone().unwrap_or_else(|| player.clone());
    if !host.is_active(&k)? {
        k = player.clone();
    }
    if host.artificial(player)? && killer_player.is_some() {
        host.set_attribute(player, "dead", Attribute::Bool(true))?;
    }
    if host.artificial(player)? && killer.is_some_and(|killer| host.is_npc(killer)) {
        return Ok(());
    }
    if let Some(killer) = &killer_player {
        let killer_name = host.get_name(killer)?;
        let name = host.get_name(player)?;
        if killer_name != name {
            let username = host.username(player)?.unwrap_or_else(|| "null".into());
            host.api_message(killer, &format!("You have defeated {username}."))?;
        }
    }
    host.dispatch_message(player, "Oh dear, you are dead!")?;
    host.increment_attribute(player, DEATHS)?;
    host.temp_music(player, 90)?;
    let dangerous = !host.zone_player_death(player, killer)?
        && (!host.safe_zone(player)? && host.zone_type(player)? != SAFE_ZONE)
        && host.rights(player)? != Rights::Administrator;
    if dangerous {
        let mode = host.ironman_mode(player)?.ok_or_else(null)?;
        if mode == Mode::Hardcore {
            let gender = if host.is_male(player)? {
                "man "
            } else {
                "woman "
            };
            if host.attributes_contain(player, "permadeath")? {
                let username = host.username(player)?.unwrap_or_else(|| "null".into());
                let total = host.total_level(player)?;
                host.repository_news(&format!(
                    "Permadeath Hardcore Iron{gender} {username} has fallen. Total Level: {total}"
                ))?;
                host.permadeath(player)?;
                return Ok(());
            }
        }
        drop_items(
            host,
            controller,
            player,
            killer,
            killer_player.as_ref(),
            &k,
            factory,
            definitions,
        )?;
    }
    host.set_skulled(player, false)?;
    host.remove_attribute(player, COMBAT_TIME)?;
    host.prayer_reset(player)?;
    host.remove_attribute(player, "original-loc")?;
    host.open_default_tabs(player)?;
    host.component_visibility(player, 548, 69, false)?;
    host.component_visibility(player, 746, 12, false)?;
    npc_death::entity_finalize_death(host, player)?;
    host.appearance_sync(player)?;
    if !host.death_screen_disabled(player)? {
        host.open_component(player, DEATH_SCREEN)?;
    }
    Ok(())
}

/// The dangerous-death block of `finalizeDeath` after the permadeath check.
#[allow(clippy::too_many_arguments)]
fn drop_items<H: Host>(
    host: &mut H,
    controller: &mut Controller,
    player: &NodeHandle,
    killer: Option<&NodeHandle>,
    killer_player: Option<&NodeHandle>,
    k: &NodeHandle,
    factory: &Factory,
    definitions: &Registry,
) -> Result<()> {
    let manager = host.manager();
    let field = host.location_field(player)?;
    let bones_at = match host.attribute_location(player, ORIGINAL_LOCATION)? {
        Some(location) => location,
        None => field.clone(),
    };
    let bones = Shared::new(Item::new(BONES, 1, 1000, definitions));
    gm::create_at(host, &manager, Some(bones), bones_at, Some(k.clone()))?;
    let [kept, lost] = kept::get_containers(host, player, factory, definitions)?;
    let mut equipment = host.equipment(player)?;
    let worn = equipment.items();
    let length = worn.read().len();
    for index in 0..length {
        let Some(item) = worn.read()[index].clone() else {
            continue;
        };
        let id = item.read().id();
        host.run_equip_listeners(id, player, &item, false)?;
        let definition = definition(&item)?;
        if let Some(plugin) = host.equipment_plugin(&definition)? {
            host.fire_unequip(plugin, player, &item)?;
        }
    }
    let lost_items: Vec<item::Handle> = lost.items().read().iter().flatten().cloned().collect();
    if grave::allow_generate(host, player)? {
        let kind = grave::grave_type(host, player)?;
        let created = grave::produce_grave(host, kind)?;
        let location = host.location_field(player)?;
        grave::initialize(
            host,
            controller,
            &created,
            player,
            location,
            &lost_items,
            definitions,
        )?;
    } else {
        let mut lost_text = String::new();
        let mut coins = 0i32;
        let slots = lost.items();
        let length = slots.read().len();
        for index in 0..length {
            let Some(original) = slots.read()[index].clone() else {
                continue;
            };
            let mut stay_private = false;
            let (id, amount) = {
                let item = original.read();
                (item.id(), item.amount())
            };
            if killer_player.is_some() {
                let name = host.item_name(id)?.unwrap_or_else(|| "null".into());
                lost_text.push_str(&format!("{name}({amount}), "));
            }
            if grave::should_crumble(host, id)? {
                continue;
            }
            if grave::should_release(host, id)? {
                continue;
            }
            if !barrows::is_barrows(id) && !definition_tradeable(&definition(&original)?)? {
                if let Some(killer) = killer_player {
                    let mut value = definition_alchemy(&definition(&original)?, true, definitions)?;
                    if host.stat_level(killer, MAGIC)? < 55 {
                        value /= 2;
                    }
                    coins = coins.wrapping_add(0.max(value.wrapping_sub(250)));
                    continue;
                }
                stay_private = true;
            }
            let item = if barrows::is_barrows(id) {
                if barrows::is_broken(id) {
                    Some(original)
                } else {
                    let broken = barrows::broken_id(id).ok_or_else(null)?;
                    Some(Shared::new(Item::new(broken, amount, 1000, definitions)))
                }
            } else {
                Some(grave::check_transform(host, &original)?)
            };
            let owner = killer_player.unwrap_or(player).clone();
            let location = host.location_field(player)?;
            let ground = gm::create_at(host, &manager, item, location, Some(owner))?;
            ground.write().remain_private = stay_private;
        }
        if coins > 0 {
            let location = host.location_field(player)?;
            let coins = Shared::new(Item::new(COINS, coins, 1000, definitions));
            let owner = killer.filter(|_| killer_player.is_some()).cloned();
            gm::create_at(host, &manager, Some(coins), location, owner)?;
        }
        if let Some(killer) = killer_player {
            let name = host.name_field(player)?.unwrap_or_else(|| "null".into());
            host.monitor_pk(killer, &format!("Killed {name}, who dropped: {lost_text}"))?;
        }
        host.send_message(
            player,
            &colorize("%RDue to the circumstances of your death, you do not have a grave."),
        )?;
    }
    equipment.clear(true, definitions, host)?;
    host.slayer_flags(player)?;
    let mut inventory = host.inventory(player)?;
    inventory.clear(true, definitions, host)?;
    inventory.add_many(&kept.items(), definitions, host)?;
    host.dismiss_familiar(player)?;
    Ok(())
}

/// `DeathInterface`'s button handler on the death screen: button 1 turns the
/// screen off for good and closes it. Every button is handled.
pub fn death_interface_button<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    button: i32,
) -> Result<bool> {
    if button == 1 {
        host.set_death_screen_disabled(player, true)?;
        host.close_interface(player)?;
    }
    Ok(true)
}
