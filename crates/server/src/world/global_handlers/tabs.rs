//! The tab interface listeners (`content/global/handlers/iface/tabs`).
use super::{
    ALL_OPTIONS, ClanField, ClanRank, Error, Extension, Host, Input, IronmanMode, LogLevel,
    PrayerType, Result, ScriptArg, SpellBook, Task, Toggle, Value,
};
use crate::world::interaction_listeners::Kind as IntType;
use crate::world::interface_listeners::Button;

// ------------------------------------------------------------------ LogoutTabInterface

/// `on(LOGOUT_182)`.
pub fn logout<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 6 {
        if !h.can_logout(p)? {
            return Ok(true);
        }
        if h.in_combat(p)? {
            h.api_message(
                p,
                "You can't log out until 10 seconds after the end of combat.",
            )?;
            return Ok(true);
        }
        if h.is_teleporting(p)? {
            h.api_message(p, "Please finish your teleport before logging out.")?;
            return Ok(true);
        }
        h.disconnect(p)?;
        return Ok(true);
    }
    Ok(true)
}

// ------------------------------------------------------------------ SettingsTabInterface

/// `restrictForIronman(player, restriction) { action }`.
pub fn restrict_for_ironman<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    mode: IronmanMode,
    action: impl FnOnce(&mut H) -> Result<()>,
) -> Result<()> {
    if !h.check_restriction(p, mode)? {
        action(h)?;
    }
    Ok(())
}

/// `on(OPTIONS_261)`.
pub fn settings<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    match b.button {
        3 => h.toggle(p, Toggle::Run)?,
        4 => h.toggle(p, Toggle::ChatEffects)?,
        5 => h.toggle(p, Toggle::SplitPrivateChat)?,
        6 => h.toggle(p, Toggle::MouseButton)?,
        7 => restrict_for_ironman(h, p, IronmanMode::Standard, |h| {
            h.toggle(p, Toggle::AcceptAid)
        })?,
        8 => h.open_single_tab(p, 398)?,
        16 => h.open_interface(p, 742)?,
        18 => h.open_interface(p, 743)?,
        _ => {}
    }
    Ok(true)
}

// ------------------------------------------------------------------ StatsTabInterface

/// `StatsTabInterface.SkillConfig`: button, config, skill.
pub const SKILL_CONFIGS: [(i32, i32, i32); 24] = [
    (125, 1, 0),
    (126, 2, 2),
    (127, 5, 1),
    (128, 3, 4),
    (129, 7, 5),
    (130, 4, 6),
    (131, 12, 20),
    (133, 6, 3),
    (134, 8, 16),
    (135, 9, 15),
    (136, 10, 17),
    (137, 11, 12),
    (138, 19, 9),
    (139, 20, 18),
    (141, 13, 14),
    (142, 14, 13),
    (143, 15, 10),
    (144, 16, 7),
    (145, 17, 11),
    (146, 18, 8),
    (147, 21, 19),
    (132, 22, 22),
    (140, 23, 21),
    (148, 24, 23),
];
/// `StatsTabInterface.ADVANCE_CONFIGS`.
pub const ADVANCE_CONFIGS: [i32; 24] = [
    9, 40, 17, 49, 25, 57, 33, 641, 659, 664, 121, 649, 89, 114, 107, 72, 64, 80, 673, 680, 99,
    698, 689, 705,
];

/// `on(STATS_320)`.
pub fn stats<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let Some(&(_, config, skill)) = SKILL_CONFIGS.iter().find(|c| c.0 == b.button) else {
        return Ok(true);
    };
    let key = format!("levelup:{skill}");
    let level_up = h.api_get_attribute(p, &key)?;
    if Value::bool(level_up, false)? {
        h.api_remove_attributes(p, &[&key])?;
        h.flashing_icons(p, -1)?;
        h.set_varp(p, 1230, ADVANCE_CONFIGS[skill as usize], false)?;
        h.open_interface(p, 741)?;
    } else {
        h.open_interface(p, 499)?;
        h.set_varp(p, 965, config, false)?;
        h.api_set_attribute(p, "skillMenu", Value::Int(config))?;
    }
    Ok(true)
}

/// `on(SKILL_GUIDE_V2_499)`.
pub fn skill_guide<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let menu = h.api_get_attribute(p, "skillMenu")?;
    let menu = Value::int(menu, -1)?;
    h.set_varbit(p, 3288, menu, false)?;
    h.set_varbit(p, 3289, b.button.wrapping_sub(10), false)?;
    Ok(true)
}

/// `on(LEVEL_UP_INTERFACE_740)`.
pub fn level_up<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 3 {
        h.close_interface(p)?;
    }
    Ok(true)
}

// ------------------------------------------------------------------ EquipmentTabInterface

/// `ITEMS_KEPT_ON_DEATH_102`.
pub const ITEMS_KEPT_ON_DEATH: i32 = 102;
/// `Items.BEER_1917`.
pub const BEER: i32 = 1917;

/// `onOpen(ITEMS_KEPT_ON_DEATH_102)`.
pub fn items_kept_on_death<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    component: i32,
) -> Result<bool> {
    let zone = h.zone_type(p)?;
    let [kept, _] = h.death_containers(p)?;
    let amount = if !h.is_skulled(p)? && h.item_count(&kept)? < 3 {
        if h.prayer_active(p, PrayerType::ProtectItems)? {
            4
        } else {
            3
        }
    } else {
        h.item_count(&kept)?
    };
    let slot0 = h.container_id(&kept, 0)?;
    let slot1 = h.container_id(&kept, 1)?;
    let slot2 = h.container_id(&kept, 2)?;
    let slot3 = h.container_id(&kept, 3)?;
    let skull = i32::from(h.is_skulled(p)?);
    let beast = if h.has_familiar(p)? && h.is_burden_beast(p)? {
        Some(h.beast_container(p)?)
    } else {
        None
    };
    let bob = match beast {
        Some(container) => i32::from(!h.is_empty(&container)?),
        None => 0,
    };
    let args = vec![
        ScriptArg::Int(bob),
        ScriptArg::Int(skull),
        ScriptArg::Int(slot3),
        ScriptArg::Int(slot2),
        ScriptArg::Int(slot1),
        ScriptArg::Int(slot0),
        ScriptArg::Int(amount),
        ScriptArg::Int(zone),
        ScriptArg::Text("You are skulled.".into()),
    ];
    if amount > 4 && zone == 0 {
        h.log(
            LogLevel::Err,
            "Items kept on death interface should not contain more than 4 items when not in a safe zone!",
        )?;
    }
    h.run_script(p, 118, "siiooooii", args)?;
    let count = h.item_count(&kept)?;
    h.iface_settings(p, ALL_OPTIONS, 18, component, 0, count)?;
    let [_, lost] = h.death_containers(p)?;
    let count = h.item_count(&lost)?;
    h.iface_settings(p, ALL_OPTIONS, 21, component, 0, count)?;
    Ok(true)
}

/// `on(WORNITEMS_387)`.
pub fn worn_items<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    match b.button {
        28 => {
            if b.opcode == 81 {
                h.unequip(p, b.slot, b.item)?;
            }
            if b.opcode == 206 {
                operate_item(h, p, b.slot)?;
            }
        }
        52 => h.open_interface(p, ITEMS_KEPT_ON_DEATH)?,
        55 => h.open_interface(p, 667)?,
        _ => {}
    }
    Ok(true)
}

/// `onOpen(EQUIP_SCREEN2_667)`.
pub fn equip_screen_open<H: Host + ?Sized>(h: &mut H, p: &H::Node, component: i32) -> Result<bool> {
    h.iface_settings(p, ALL_OPTIONS, 14, component, 0, 13)?;
    h.equipment_update(p)?;
    h.open_single_tab(p, 670)?;
    Ok(true)
}

/// `on(EQUIP_SCREEN2_667)`.
pub fn equip_screen<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 14 {
        match b.opcode {
            9 => {
                let equipment = h.equipment(p)?;
                examine_slot(h, p, &equipment, b.slot)?;
            }
            155 => h.unequip(p, b.slot, b.item)?,
            196 => operate_item(h, p, b.slot)?,
            _ => {}
        }
    }
    Ok(true)
}

/// `onClose(EQUIP_SCREEN2_667)`.
pub fn equip_screen_close<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.close_tab_interface(p)?;
    Ok(true)
}

/// `onOpen(INVENTORY_WEAR2_670)`.
pub fn equip_inventory_open<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    component: i32,
) -> Result<bool> {
    let inventory = h.inventory(p)?;
    let items = h.to_array(&inventory)?;
    h.generate_items(p, items, &["Equip"], component, 0, 7, 4, Some(93))?;
    Ok(true)
}

/// `on(INVENTORY_WEAR2_670)`.
pub fn equip_inventory<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.opcode == 9 {
        let inventory = h.inventory(p)?;
        examine_slot(h, p, &inventory, b.slot)?;
    }
    if b.opcode == 155 {
        equip_item(h, p, b.slot)?;
    }
    Ok(true)
}

/// `sendMessage(player, container.get(slot).definition.examine)`.
fn examine_slot<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    c: &H::Container,
    slot: i32,
) -> Result<()> {
    let item = h.get(c, slot)?.ok_or(Error::Null)?;
    let text = h.examine(&item)?.ok_or(Error::Null)?;
    h.api_message(p, &text)
}

/// `EquipmentTabInterface.equipItem`.
fn equip_item<H: Host + ?Sized>(h: &mut H, p: &H::Node, slot: i32) -> Result<()> {
    let inventory = h.inventory(p)?;
    let Some(item) = h.get(&inventory, slot)? else {
        return Ok(());
    };
    let options = h.inventory_options(&item)?.ok_or(Error::Null)?;
    let wearable = options
        .iter()
        .any(|o| matches!(o.as_deref(), Some("Equip" | "Wield" | "Wear")));
    let id = h.id(&item)?;
    if wearable || id == BEER {
        h.listeners(id, IntType::Item, "equip", p, &item)?;
    } else {
        h.api_message(p, "You can't wear that.")?;
    }
    Ok(())
}

/// `EquipmentTabInterface.operateItem`.
fn operate_item<H: Host + ?Sized>(h: &mut H, p: &H::Node, slot: i32) -> Result<()> {
    let equipment = h.equipment(p)?;
    let Some(item) = h.get(&equipment, slot)? else {
        return Ok(());
    };
    let id = h.id(&item)?;
    if h.listeners(id, IntType::Item, "operate", p, &item)? {
        return Ok(());
    }
    if let Some(handler) = h.operate_handler(&item)? {
        if h.operate_handle(&handler, p, &item, "operate")? {
            return Ok(());
        }
    }
    h.api_message(p, "You can't operate that.")
}

// ------------------------------------------------------------------ CombatTabInterface

/// `CombatTabInterface.handle(p, component, opcode, button, slot, itemId)`.
pub fn combat_tab<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    match b.button {
        7 | 9 | 24 | 26 | 27 => {
            h.submit(1, &[p], Task::Retaliate { player: p.clone() })?;
            Ok(true)
        }
        11 | 10 | 8 | 85 => {
            h.submit(1, &[p], Task::SpecialBar { player: p.clone() })?;
            Ok(true)
        }
        _ => {
            let Some(weapon) = java_weapon(h, p)? else {
                return Ok(false);
            };
            if h.set_attack_style(&weapon, b.button)? {
                if b.button == 4 || b.button == 5 {
                    h.open_autocast_select(&weapon)?;
                } else if h.has_autocast_spell(p)? {
                    h.select_auto_spell(&weapon, -1, false, true)?;
                }
                return Ok(true);
            }
            Ok(false)
        }
    }
}

/// `WeaponInterface w = p.getExtension(WeaponInterface.class)` (Java's
/// checkcast fails for anything else).
pub(crate) fn java_weapon<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<Option<H::Weapon>> {
    match h.weapon_interface(p)? {
        None => Ok(None),
        Some(Extension::Weapon(w)) => Ok(Some(w)),
        Some(Extension::StaffConstant) => Err(Error::Cast),
    }
}

/// The first CombatTabInterface pulse: `toggleRetaliating`, finished.
pub fn retaliate_pulse<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.toggle(p, Toggle::Retaliating)?;
    Ok(true)
}

/// The second CombatTabInterface pulse: the special bar and an instant special.
pub fn special_bar_pulse<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    let Some(weapon) = java_weapon(h, p)? else {
        return Ok(true);
    };
    if !h.is_special_bar(&weapon)? {
        return Ok(true);
    }
    h.toggle(p, Toggle::SpecialBar)?;
    if h.special_toggled(p)? {
        let equipment = h.equipment(p)?;
        let id = h.get_new_id(&equipment, 3)?;
        if let Some(handler) = h.special_handler(id)? {
            if h.special_instant(&handler, p)? {
                // handleInstantSpec: handler.swing(p, victim, null)
                let victim = h.combat_victim(p)?;
                h.special_swing(&handler, p, victim.as_ref())?;
            }
        }
    }
    Ok(true)
}

// ------------------------------------------------------------------ MagicTabInterface

/// `SpellListener.NONE`.
pub const SPELL_NONE: i32 = -5;

/// `on(book.interfaceId)` for each spell book.
pub fn magic<H: Host + ?Sized>(h: &mut H, p: &H::Node, book: SpellBook, b: Button) -> Result<bool> {
    let ticks = h.ticks();
    let delay = h.api_get_attribute(p, "magic:delay")?;
    if ticks < Value::int(delay, -1)? {
        return Ok(true);
    }
    let name = rs09_game_core::java_lower::lowercase_root(book.name());
    h.spell_listeners(b.button, SPELL_NONE, &name, p, None)?;
    h.cast_spell(p, book, b.button, p)
}

// ------------------------------------------------------------------ PrayerTabInterface

/// `on(PRAYER_271)`.
pub fn prayer<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let Some(prayer) = h.prayer_type(b.button)? else {
        return Ok(true);
    };
    h.prayer_toggle(p, prayer)
}

// ------------------------------------------------------------------ QuestTabInterface

/// `on(QUESTJOURNAL_V2_274)`.
pub fn quest_journal<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 3 {
        h.diary_open_tab(p)?;
    } else {
        match h.quest_for_button(p, b.button)? {
            Some(quest) => {
                h.open_interface(p, 275)?;
                let stage = h.quest_stage(&quest, p)?;
                h.draw_journal(&quest, p, stage)?;
            }
            None => h.quest_requirements(p, b.button)?,
        }
    }
    Ok(true)
}

/// `on(AREA_TASK_259)`.
pub fn area_tasks<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 8 {
        h.open_tab(p, 2, 274)?;
    } else {
        let kind = h.diary_for_child(b.button)?;
        if let Some(diary) = h.get_diary(p, kind)? {
            h.diary_open(&diary, p)?;
        }
    }
    Ok(true)
}

// ------------------------------------------------------------------ ClanTabInterface

/// `on(CLANJOIN_589)`.
pub fn clan_join<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 9 {
        if h.opened(p)? {
            h.api_message(
                p,
                "Please close the interface you have open before using 'Clan Setup'",
            )?;
        } else {
            h.clan_open_settings(p)?;
        }
    }
    if b.button == 14 {
        h.toggle_lootshare(p)?;
    }
    Ok(true)
}

/// `ClanTabInterface.getRank(opcode)`.
pub fn rank(opcode: i32) -> ClanRank {
    match opcode {
        155 => ClanRank::None,
        196 => ClanRank::Friend,
        124 => ClanRank::Recruit,
        199 => ClanRank::Corporal,
        234 => ClanRank::Sergeant,
        168 => ClanRank::Lieutenant,
        166 => ClanRank::Captain,
        64 => ClanRank::General,
        53 => ClanRank::Owner,
        _ => ClanRank::None,
    }
}

/// `on(CLANSETUP_590)`.
pub fn clan_setup<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let name = h.name(p)?;
    let clan = h.clan_get(name.as_deref(), true)?;
    let field = match b.button {
        22 => {
            if b.opcode == 155 {
                let body = Task::ClanPrefix {
                    player: p.clone(),
                    clan: clan.clone(),
                };
                h.input_dialogue(p, false, "Enter clan prefix:", body)?;
            }
            if b.opcode == 196 {
                let c = clan.as_ref().ok_or(Error::Null)?;
                h.clan_set_name(c, "Chat disabled")?;
                h.comm_set_clan_name(p, "")?;
                if h.world_communicator_enabled()? {
                    let name = h.comm_clan_name(p)?;
                    h.ms_clan_rename(p, name.as_deref())?;
                }
                h.clan_update_settings(c, p)?;
                h.clan_delete(c)?;
            }
            None
        }
        23 => Some((ClanField::Join, rank(b.opcode), 0)),
        24 => Some((ClanField::Message, rank(b.opcode), 1)),
        25 => Some((ClanField::Kick, rank(b.opcode), 2)),
        26 => Some((
            ClanField::Loot,
            if b.opcode == 155 {
                ClanRank::Administrator
            } else {
                rank(b.opcode)
            },
            3,
        )),
        33 => {
            h.api_message(p, "CoinShare is not available.")?;
            None
        }
        _ => None,
    };
    if let Some((field, rank, kind)) = field {
        let c = clan.as_ref().ok_or(Error::Null)?;
        h.clan_set_rank(c, field, Some(rank))?;
        let current = h.clan_rank(c, field)?;
        h.comm_set_rank(p, field, current)?;
        let current = h.clan_rank(c, field)?;
        h.ms_clan_setting(p, kind, current)?;
    }
    let c = clan.as_ref().ok_or(Error::Null)?;
    h.clan_update_settings(c, p)?;
    h.clan_update(c)?;
    Ok(true)
}

/// ClanTabInterface's prefix input callback.
pub fn clan_prefix<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    clan: &Option<H::Clan>,
    value: &Input,
) -> Result<()> {
    let name = crate::world::social::names::format_display_name(&value.to_java_string());
    if h.world_communicator_enabled()? {
        h.ms_clan_rename(p, Some(&name))?;
    }
    let c = clan.as_ref().ok_or(Error::Null)?;
    if h.clan_name(c)?.as_deref() == Some("Chat disabled") {
        h.api_message(p, "Your clan channel has now been enabled!")?;
        let username = h.username(p)?;
        h.api_message(
            p,
            &format!("Join your channel by clicking 'Join Chat' and typing: {username}"),
        )?;
    }
    h.clan_set_name(c, &name)?;
    h.comm_set_clan_name(p, &name)?;
    h.clan_update_settings(c, p)?;
    h.clan_update(c)?;
    Ok(())
}

// ------------------------------------------------------------------ EmoteTabInterface, MusicTabInterface

/// `on(EMOTES_464)`.
pub fn emotes<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    h.emote(p, b.button)?;
    Ok(true)
}

/// `on(MUSIC_V3_187)`.
pub fn music<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.opcode == 155 {
        if b.button == 11 {
            h.toggle_looping(p)?;
            return Ok(true);
        }
        if b.button == 1 {
            if h.unlocked(p, b.slot)?.is_some() {
                // player.musicPlayer.play(player.musicPlayer.unlocked[slot])
                let song = h.unlocked(p, b.slot)?;
                h.play(p, song)?;
                return Ok(true);
            }
            if h.is_admin(p)? {
                for song in h.songs()? {
                    if song.index == b.slot {
                        h.unlock_song(p, song.id)?;
                    }
                }
            } else {
                h.api_message(p, "You have not unlocked this piece of music yet!")?;
            }
            return Ok(true);
        }
    }
    Ok(true)
}
