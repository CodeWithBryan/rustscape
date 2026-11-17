//! `LevelUp` and `LevelUpJingles.getSkillJingle`: the level-up reward
//! (jingle, milestones, graphic, chatbox texts, message, prayer point, the
//! level-99 news, skillcape trim and emote), the milestone varp and the
//! flashing skill icons.
use super::{Error, Result, Value, emotes};

/// `Skills.SKILL_NAME`.
pub const SKILL_NAME: [&str; 24] = [
    "Attack",
    "Defence",
    "Strength",
    "Hitpoints",
    "Ranged",
    "Prayer",
    "Magic",
    "Cooking",
    "Woodcutting",
    "Fletching",
    "Fishing",
    "Firemaking",
    "Crafting",
    "Smithing",
    "Mining",
    "Herblore",
    "Agility",
    "Thieving",
    "Slayer",
    "Farming",
    "Runecrafting",
    "Hunter",
    "Construction",
    "Summoning",
];
/// `Skills.PRAYER`.
const PRAYER: i32 = 5;
/// `SKILLCAPES`.
pub const SKILLCAPES: [i32; 24] = [
    9747, 9753, 9750, 9768, 9756, 9759, 9762, 9801, 9807, 9783, 9798, 9804, 9780, 9795, 9792, 9774,
    9771, 9777, 9786, 9810, 9765, 9948, 9789, 12169,
];
/// `SKILL_ICON`.
pub const SKILL_ICON: [i32; 25] = [
    67108864, 335544320, 134217728, 402653184, 201326592, 469762048, 268435456, 1073741824,
    1207959552, 1275068416, 1006632960, 1140850688, 738197504, 939524096, 872415232, 603979776,
    536870912, 671088640, 1342177280, 1409286144, 805306368, 1543503872, 1476395008, 1610612736,
    1677721600,
];
/// `FLASH_ICONS`.
pub const FLASH_ICONS: [i32; 25] = [
    1, 4, 2, 64, 8, 16, 32, 32768, 131072, 2048, 16384, 65536, 1024, 8192, 4096, 256, 128, 512,
    524288, 1048576, 262144, 4194304, 2097152, 8388608, 16777216,
];
/// `ADVANCE_CONFIGS`.
pub const ADVANCE_CONFIGS: [i32; 24] = [
    9, 40, 17, 49, 25, 57, 33, 641, 659, 664, 121, 649, 89, 114, 107, 72, 64, 80, 673, 680, 99,
    698, 689, 705,
];
/// `CLIENT_ID`.
pub const CLIENT_ID: [i32; 25] = [
    1, 5, 2, 6, 3, 7, 4, 16, 18, 19, 15, 17, 11, 14, 13, 9, 8, 10, 20, 21, 12, 23, 22, 24, 24,
];
/// `SKILL_MILESTONES`.
pub const SKILL_MILESTONES: [i32; 4] = [1, 50, 75, 100];
/// `COMBAT_MILESTONES`.
pub const COMBAT_MILESTONES: [i32; 13] = [3, 5, 10, 15, 25, 75, 90, 100, 110, 120, 126, 130, 138];
/// The interface `levelup` writes to and opens in the chatbox.
pub const LEVEL_UP_INTERFACE: i32 = 740;

/// Every call `LevelUp` and `getSkillJingle` make outside their class, for
/// the owning player.
pub trait Host {
    /// `entity.getAttribute(key, default)`.
    fn get_attribute(&mut self, key: &str, default: Value) -> Result<Value>;
    /// `entity.setAttribute(key, value)`.
    fn set_attribute(&mut self, key: &str, value: Value) -> Result<()>;
    /// `skills.getStaticLevel(slot)` (also `ContentAPI.getStatLevel`).
    fn static_level(&mut self, slot: i32) -> Result<i32>;
    /// `skills.incrementPrayerPoints(amount)`.
    fn increment_prayer_points(&mut self, amount: f64) -> Result<()>;
    /// `skills.getTotalLevel()`.
    fn total_level(&mut self) -> Result<i32>;
    /// `skills.getCombatMilestone()`.
    fn combat_milestone(&mut self) -> Result<i32>;
    /// `skills.setCombatMilestone(value)`.
    fn set_combat_milestone(&mut self, value: i32) -> Result<()>;
    /// `skills.getSkillMilestone()`.
    fn skill_milestone(&mut self) -> Result<i32>;
    /// `skills.setSkillMilestone(value)`.
    fn set_skill_milestone(&mut self, value: i32) -> Result<()>;
    /// `properties.getCurrentCombatLevel()`.
    fn combat_level(&mut self) -> Result<i32>;
    /// `ContentAPI.playJingle(player, id)`: MusicPacket with the secondary flag.
    fn jingle(&mut self, id: i32) -> Result<()>;
    /// `ContentAPI.setVarp(player, id, value, save)`.
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()>;
    /// `packetDispatch.sendGraphic(id)`.
    fn graphic(&mut self, id: i32) -> Result<()>;
    /// `packetDispatch.sendString(text, interface, child)`.
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> Result<()>;
    /// `packetDispatch.sendMessage(text)`.
    fn message(&mut self, text: &str) -> Result<()>;
    /// `player.isArtificial()`.
    fn is_artificial(&mut self) -> bool;
    /// `player.getUsername()`.
    fn username(&mut self) -> String;
    /// `Repository.sendNews(text)` (`style` None) or
    /// `Repository.sendNews(text, icon, color)` (through `ContentAPI.sendNews`).
    fn news(&mut self, text: &str, style: Option<(i32, &str)>) -> Result<()>;
    /// `Skillcape.trim(player)`: content.
    fn skillcape_trim(&mut self) -> Result<()>;
    /// `player.getEmoteManager().unlock(emote)`.
    fn emote_unlock(&mut self, emote: emotes::Emote) -> Result<bool>;
    /// `player.getInterfaceManager().openChatbox(new Component(id))`.
    fn open_chatbox(&mut self, id: i32) -> Result<()>;
}

fn attribute_bool<H: Host>(h: &mut H, key: &str) -> Result<bool> {
    h.get_attribute(key, Value::Bool(false))?.boolean()
}

fn skill_name(slot: i32) -> Result<&'static str> {
    usize::try_from(slot)
        .ok()
        .and_then(|slot| SKILL_NAME.get(slot).copied())
        .ok_or(Error::Index)
}

fn at<const N: usize>(table: &[i32; N], slot: i32) -> Result<i32> {
    usize::try_from(slot)
        .ok()
        .and_then(|slot| table.get(slot).copied())
        .ok_or(Error::Index)
}

/// `LevelUp.levelup(player, slot, amount)`.
pub fn levelup<H: Host>(h: &mut H, slot: i32, amount: i32) -> Result<()> {
    if !attribute_bool(h, "tutorial:complete")? {
        return Ok(());
    }
    let jingle = skill_jingle(h, slot)?;
    h.jingle(jingle)?;
    handle_milestones(h, slot, amount)?;
    h.graphic(199)?;
    let name = skill_name(slot)?;
    h.send_string(
        &format!("<col=00008B>Congratulations, you've just advanced a {name} level!"),
        LEVEL_UP_INTERFACE,
        0,
    )?;
    let level = h.static_level(slot)?;
    h.send_string(
        &format!("Your {name} level is now {level}."),
        LEVEL_UP_INTERFACE,
        1,
    )?;
    let level = h.static_level(slot)?;
    h.message(&format!(
        "You've just advanced a {name} level! You have reached level {level}."
    ))?;
    if slot == PRAYER {
        h.increment_prayer_points(1.0)?;
    }
    if h.static_level(slot)? == 99 && !h.is_artificial() {
        let username = h.username();
        h.news(
            &format!("{username} has just achieved level 99 {name}!"),
            None,
        )?;
        h.skillcape_trim()?;
        h.emote_unlock(emotes::Emote::Skillcape)?;
        if h.total_level()? == 2376 {
            let username = h.username();
            h.news(
                &format!("{username} has just achieved level 99 in all skills!"),
                Some((12, "CC6600")),
            )?;
        }
    }
    h.set_attribute(&format!("/save:levelup:{slot}"), Value::Bool(true))?;
    send_flashing_icons(h, slot)
}

/// `LevelUp.handleMilestones(player, slot, amount)`: `amount` is not read.
pub fn handle_milestones<H: Host>(h: &mut H, slot: i32, _amount: i32) -> Result<()> {
    let mut value = at(&ADVANCE_CONFIGS, slot)?;
    for (i, milestone) in COMBAT_MILESTONES.iter().enumerate() {
        if h.combat_level()? < *milestone {
            if i as i32 > h.combat_milestone()? {
                value |= 0x2;
                h.set_combat_milestone(i as i32)?;
            }
            break;
        }
    }
    let total = h.total_level()?;
    for (i, milestone) in SKILL_MILESTONES.iter().enumerate() {
        if total < *milestone {
            if i as i32 > h.skill_milestone()? {
                value |= 0x4;
                h.set_skill_milestone(i as i32)?;
            }
            break;
        }
    }
    value |= h.combat_milestone()?.wrapping_shl(23);
    value |= h.skill_milestone()?.wrapping_shl(27);
    h.varp(1230, value, false)
}

/// `LevelUp.sendFlashingIcons(player, slot)`: the flashing icons of every
/// `levelup:<skill>` attribute, and for a slot above -1 that skill's icon
/// and the level-up chatbox.
pub fn send_flashing_icons<H: Host>(h: &mut H, slot: i32) -> Result<()> {
    let mut value = 0;
    for (i, icon) in FLASH_ICONS.iter().enumerate().take(SKILL_NAME.len()) {
        if attribute_bool(h, &format!("levelup:{i}"))? {
            value |= icon;
        }
    }
    if slot > -1 {
        value |= at(&SKILL_ICON, slot)?;
        h.open_chatbox(LEVEL_UP_INTERFACE)?;
    }
    h.varp(1179, value, false)
}

const ATTACK: &[i32] = &[5, 10, 15, 20, 30, 40, 50, 60, 70, 75, 78, 99];
const MINING: &[i32] = &[
    6, 10, 15, 20, 21, 30, 31, 35, 40, 41, 45, 46, 50, 55, 60, 70, 80, 85, 90, 99,
];
const SMITHING: &[i32] = &[
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 46, 48, 49, 50, 51, 52, 53,
    54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 68, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
    80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99,
];
const DEFENCE: &[i32] = &[
    5, 10, 20, 25, 30, 40, 45, 50, 55, 60, 65, 70, 75, 78, 80, 99,
];
const HERBLORE: &[i32] = &[
    3, 5, 8, 11, 12, 19, 20, 22, 25, 26, 30, 31, 34, 35, 36, 38, 39, 40, 44, 45, 48, 50, 52, 53,
    54, 55, 57, 59, 60, 63, 65, 66, 67, 68, 69, 70, 72, 73, 75, 76, 78, 80, 81, 82, 99,
];
const FISHING: &[i32] = &[
    5, 10, 15, 16, 20, 23, 25, 28, 30, 33, 35, 38, 40, 46, 48, 50, 53, 55, 58, 62, 65, 68, 70, 76,
    79, 81, 90, 96, 99,
];
const RANGED: &[i32] = &[
    5, 10, 16, 19, 20, 21, 25, 26, 28, 31, 30, 33, 35, 36, 37, 39, 40, 42, 45, 46, 50, 55, 60, 61,
    65, 70, 78, 80, 99,
];
const THIEVING: &[i32] = &[
    2, 5, 10, 13, 15, 20, 22, 25, 27, 28, 30, 32, 35, 36, 38, 40, 42, 43, 44, 45, 47, 49, 50, 52,
    53, 55, 59, 63, 65, 70, 72, 75, 78, 80, 82, 85, 99,
];
const COOKING: &[i32] = &[
    4, 5, 6, 7, 8, 10, 11, 12, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 28, 29, 30, 31, 32,
    33, 34, 35, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 53, 54, 55, 57, 58, 59,
    60, 62, 64, 65, 67, 68, 70, 80, 82, 85, 91, 95, 99,
];
const PRAYER_LEVELS: &[i32] = &[
    2, 4, 7, 8, 9, 10, 13, 16, 19, 22, 25, 26, 28, 31, 34, 35, 37, 40, 43, 44, 45, 46, 49, 52, 60,
    70, 85, 90, 99,
];
const CRAFTING: &[i32] = &[
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53,
    54, 55, 56, 57, 58, 60, 62, 63, 64, 66, 67, 68, 70, 71, 73, 74, 75, 77, 79, 80, 82, 84, 85, 87,
    90, 99,
];
const FIREMAKING: &[i32] = &[
    4, 5, 11, 12, 15, 20, 21, 25, 26, 30, 33, 35, 40, 42, 43, 45, 47, 48, 49, 50, 52, 53, 55, 58,
    59, 60, 62, 63, 65, 68, 70, 72, 75, 76, 78, 79, 80, 83, 85, 87, 89, 92, 95, 99,
];
const MAGIC: &[i32] = &[
    4, 5, 7, 9, 11, 13, 14, 15, 17, 19, 20, 21, 23, 24, 25, 27, 29, 30, 31, 32, 33, 35, 37, 39, 40,
    43, 45, 47, 49, 50, 51, 52, 53, 54, 55, 56, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
    71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 90, 91, 92, 93, 94, 95,
    96, 97, 99,
];
const FLETCHING: &[i32] = &[
    5, 7, 9, 10, 11, 15, 18, 20, 22, 24, 25, 26, 30, 32, 33, 35, 37, 38, 39, 40, 41, 42, 43, 45,
    46, 48, 49, 50, 51, 52, 53, 54, 55, 55, 56, 58, 59, 60, 61, 62, 63, 65, 67, 69, 70, 71, 73, 75,
    77, 80, 81, 85, 90, 95, 99,
];
const WOODCUTTING: &[i32] = &[
    6, 10, 12, 15, 20, 21, 27, 30, 31, 35, 37, 40, 41, 42, 44, 45, 50, 54, 56, 57, 58, 60, 61, 75,
    80, 82, 99,
];
const RUNECRAFTING: &[i32] = &[
    2, 5, 6, 9, 10, 11, 13, 14, 15, 19, 20, 22, 23, 26, 27, 28, 33, 35, 38, 40, 42, 44, 46, 52, 54,
    55, 56, 57, 59, 65, 66, 70, 74, 76, 77, 78, 82, 84, 91, 92, 95, 98, 99,
];
const SLAYER: &[i32] = &[
    5, 7, 10, 15, 17, 20, 22, 25, 30, 32, 33, 35, 37, 39, 40, 42, 45, 47, 50, 51, 52, 55, 56, 57,
    58, 59, 60, 63, 65, 68, 70, 72, 75, 80, 83, 85, 90, 99,
];
const FARMING: &[i32] = &[
    2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
    30, 31, 32, 33, 36, 38, 39, 42, 44, 45, 47, 48, 50, 51, 52, 53, 55, 56, 57, 59, 60, 62, 63, 67,
    68, 70, 72, 73, 75, 79, 83, 85, 99,
];

/// `LevelUpJinglesKt.getSkillJingle(player, slot)`: one `getStatLevel` read,
/// then the skill's unlock table.
pub fn skill_jingle<H: Host>(h: &mut H, slot: i32) -> Result<i32> {
    let level = h.static_level(slot)?;
    let pick = |table: &[i32], unlock: i32, plain: i32| {
        if table.contains(&level) {
            unlock
        } else {
            plain
        }
    };
    Ok(match slot {
        0 => pick(ATTACK, 30, 29),
        1 => pick(DEFENCE, 38, 37),
        2 => {
            if level < 50 {
                65
            } else {
                66
            }
        }
        3 => {
            if level < 50 {
                47
            } else {
                48
            }
        }
        4 => pick(RANGED, 58, 57),
        5 => pick(PRAYER_LEVELS, 56, 55),
        6 => pick(MAGIC, 52, 51),
        7 => pick(COOKING, 34, 33),
        8 => pick(WOODCUTTING, 70, 69),
        9 => pick(FLETCHING, 44, 43),
        10 => pick(FISHING, 42, 41),
        11 => pick(FIREMAKING, 40, 39),
        12 => pick(CRAFTING, 36, 35),
        13 => pick(SMITHING, 64, 63),
        14 => pick(MINING, 54, 53),
        15 => pick(HERBLORE, 46, 45),
        16 => 28,
        17 => pick(THIEVING, 68, 67),
        18 => pick(SLAYER, 62, 61),
        19 => pick(FARMING, 11, 10),
        20 => pick(RUNECRAFTING, 60, 59),
        21 => {
            if level % 2 == 0 {
                49
            } else {
                50
            }
        }
        22 => {
            if level % 10 == 0 {
                31
            } else {
                32
            }
        }
        23 => {
            if level < 50 {
                300
            } else {
                301
            }
        }
        _ => 40,
    })
}
