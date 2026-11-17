//! The autocast members of `WeaponInterface`: `getAutospellId`,
//! `selectAutoSpell` and `canAutocast`, with the class's four autocast
//! spell tables. The weapon interface itself (its attack styles, tabs and
//! packets) is not ported here; `setAttackStyle` is a required service and
//! `current == STAFF` is a host accessor.
use super::{Host, Result, SpellBook, Value, slot};
use rs09_game_core::java_string::equals_ignore_case;

pub const MODERN_SPELL_IDS: [i32; 16] =
    [1, 4, 6, 8, 10, 14, 17, 20, 24, 27, 33, 38, 45, 48, 52, 55];
pub const ANCIENT_SPELL_IDS: [i32; 20] = [
    8, 12, 4, 0, 10, 14, 6, 2, 9, 13, 5, 1, 11, 15, 7, 3, 16, 17, 18, 19,
];
pub const SLAYER_STAFF_SPELL_IDS: [i32; 6] = [22, 31, 45, 48, 52, 55];
pub const VOID_STAFF_SPELL_IDS: [i32; 6] = [22, 42, 45, 48, 52, 55];
/// `WeaponInterface.ATTRIBUTE_AUTOCAST_BUTTON_ID`.
pub const ATTRIBUTE_AUTOCAST_BUTTON_ID: &str = "/save:autocast:buttonid";
/// `WeaponInterface.ATTRIBUTE_AUTOCAST_STYLE`.
pub const ATTRIBUTE_AUTOCAST_STYLE: &str = "/save:autocast:style";
/// `Components.WEAPON_STAFF_SEL_90`.
pub const WEAPON_STAFF_SEL: i32 = 90;

/// `equipment.getNew(3).getName().equalsIgnoreCase(name)`: one `getNew`.
fn weapon_named<H: Host>(h: &mut H, player: &H::Node, name: &str) -> Result<bool> {
    let id = h.equipment_new(player, slot::WEAPON)?;
    Ok(equals_ignore_case(&h.item_name(id), name))
}

/// The table `getAutospellId` and `selectAutoSpell` pick: by book, then the
/// Slayer's staff and the Void knight mace replace the standard table.
fn table<H: Host>(h: &mut H, player: &H::Node, modern: bool) -> Result<&'static [i32]> {
    let mut data: &'static [i32] = if modern {
        &MODERN_SPELL_IDS
    } else {
        &ANCIENT_SPELL_IDS
    };
    if modern && weapon_named(h, player, "Slayer's staff")? {
        data = &SLAYER_STAFF_SPELL_IDS;
    }
    if modern && weapon_named(h, player, "Void knight mace")? {
        data = &VOID_STAFF_SPELL_IDS;
    }
    Ok(data)
}

/// `getAutospellId(spellId)`: the spell's index in the current table, or -1.
pub fn autospell_id<H: Host>(h: &mut H, player: &H::Node, spell_id: i32) -> Result<i32> {
    let modern = *h.spell_book(player) == 192;
    let data = table(h, player, modern)?;
    Ok(data
        .iter()
        .position(|id| *id == spell_id)
        .map_or(-1, |i| i as i32))
}

/// `removeAttributes(player, keys...)`: each key without its `/save:` prefix.
fn remove_attributes<H: Host>(h: &mut H, player: &H::Node, keys: &[&str]) -> Result<()> {
    for key in keys {
        h.remove_attribute(player, &key.replace("/save:", ""))?;
    }
    Ok(())
}

/// `selectAutoSpell(buttonId, adjustAttackStyle, forgetAutocast)`.
pub fn select_auto_spell<H: Host>(
    h: &mut H,
    player: &H::Node,
    button: i32,
    adjust_attack_style: bool,
    forget_autocast: bool,
) -> Result<()> {
    let standard_child = 83;
    let defensive_child = 183;
    let modern = *h.spell_book(player) == 192;
    let data = table(h, player, modern)?;
    let current = h.autocast(player)?;
    if button >= data.len() as i32 {
        return Ok(());
    }
    let config_start = if modern { 45 } else { 13 };
    if let Some(current) = &current {
        for (index, id) in data.iter().enumerate() {
            if *id == h.spell(current).spell_id {
                let child = config_start + 2 * index as i32;
                h.interface_config(player, WEAPON_STAFF_SEL, child, true)?;
                h.interface_config(player, WEAPON_STAFF_SEL, 100 + child, true)?;
            }
        }
    }
    if forget_autocast {
        remove_attributes(
            h,
            player,
            &[ATTRIBUTE_AUTOCAST_BUTTON_ID, ATTRIBUTE_AUTOCAST_STYLE],
        )?;
    }
    if button < 0 {
        h.set_autocast(player, None)?;
        h.component_visibility(player, WEAPON_STAFF_SEL, defensive_child, false)?;
        h.component_visibility(player, WEAPON_STAFF_SEL, standard_child, false)?;
        if adjust_attack_style && current.is_some() {
            h.set_attack_style(player, 3)?;
            h.update_style(player)?;
        }
        return Ok(());
    }
    let defensive = h.attack_style_index(player)? == 3;
    h.component_visibility(player, WEAPON_STAFF_SEL, defensive_child, defensive)?;
    h.component_visibility(player, WEAPON_STAFF_SEL, standard_child, !defensive)?;
    let book = if modern {
        SpellBook::Modern
    } else {
        SpellBook::Ancient
    };
    let spell = h.registered(book, data[button as usize]);
    h.set_autocast(player, spell.as_ref())?;
    h.set_attribute(player, ATTRIBUTE_AUTOCAST_BUTTON_ID, Value::Int(button))?;
    let index = h.attack_style_index(player)?;
    h.set_attribute(player, ATTRIBUTE_AUTOCAST_STYLE, Value::Int(index))?;
    let mut config = config_start + 2 * button;
    let special = (modern && weapon_named(h, player, "Slayer's staff")?)
        || (modern && weapon_named(h, player, "Void knight mace")?);
    if special {
        let slayer = weapon_named(h, player, "Slayer's staff")?;
        config = match button {
            0 => 77,
            1 if slayer => 79,
            1 => 81,
            2 => 69,
            3 => 71,
            4 => 73,
            5 => 75,
            _ => config,
        };
        if config >= 85 || config <= 65 {
            return Ok(());
        }
        h.interface_config(
            player,
            WEAPON_STAFF_SEL,
            if defensive { 100 } else { 0 } + config,
            false,
        )?;
    } else {
        config += if defensive { 100 } else { 0 };
        config = match button {
            16 => {
                if defensive {
                    265
                } else {
                    253
                }
            }
            17 => {
                if defensive {
                    268
                } else {
                    256
                }
            }
            18 => {
                if defensive {
                    271
                } else {
                    259
                }
            }
            19 => {
                if defensive {
                    274
                } else {
                    262
                }
            }
            _ => config,
        };
        h.interface_config(player, WEAPON_STAFF_SEL, config, false)?;
    }
    Ok(())
}

/// `canAutocast(message)`.
pub fn can_autocast<H: Host>(h: &mut H, player: &H::Node, message: bool) -> Result<bool> {
    if !h.staff_interface(player) {
        return Ok(false);
    }
    if *h.spell_book(player) == SpellBook::Lunar.interface_id() {
        if message {
            h.packet_message(player, "You can't autocast Lunar magic.")?;
        }
        return Ok(false);
    }
    let first = h.equipment_new(player, slot::WEAPON)?;
    let ancient_staff = h.item_name(first).contains("ncient staff") || {
        let second = h.equipment_new(player, slot::WEAPON)?;
        h.item_name(second).contains("uriel's staff")
    };
    let book = *h.spell_book(player);
    if (book == 192 && ancient_staff) || (book == 193 && !ancient_staff) {
        if message {
            h.packet_message(
                player,
                "You can only autocast ancient magicks with an Ancient or Zuriel's staff.",
            )?;
        }
        return Ok(false);
    }
    Ok(true)
}
