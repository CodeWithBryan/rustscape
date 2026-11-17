//! `AchievementDiaryManager` and `AchievementDiary` over the persisted diary
//! state: the diary tab (component 259) and the diary interface (275), task
//! updates and completion with their messages, the shared `completedLevels`
//! list, the reward checks (Karamja gloves, Varrock armour, mining and
//! smithing doubles) and the reward claiming helpers.
//!
//! The diary fields are the player's
//! [`persisted::diaries::Manager`](crate::player::persisted::diaries::Manager)
//! (joined, not copied); its `parse` is AchievementDiaryManager.parse.
//! `completedLevels` is one list for the whole world (a static of the
//! original), reached through [`Host::completed_levels`]. DiaryType's tables
//! are [`diary_data::TYPES`](super::diary_data::TYPES); a diary type is its
//! index there (the enum's ordinal) and None is a null type.
use super::{Error, Item, Kind, Result, diary_data::TYPES, java_split};
use crate::player::persisted::diaries::{Diary, Manager};
use rs09_game_core::{java_json::Json, java_lower::lowercase_root};

/// `DIARY_COMPONENT`.
pub const DIARY_COMPONENT: i32 = 275;
/// The diary tab.
pub const TAB: i32 = 259;
const RED: &str = "<col=8A0808>";
const BLUE: &str = "<col=08088A>";
const YELLOW: &str = "<col=F7FE2E>";
const GREEN: &str = "<col=3ADF00>";
/// EquipmentContainer.SLOT_HANDS and SLOT_CHEST.
pub const SLOT_HANDS: i32 = 9;
pub const SLOT_CHEST: i32 = 4;
/// The DiaryType ordinals the reward checks name.
pub const KARAMJA: usize = 0;
pub const VARROCK: usize = 1;
/// `Bar` constants in declaration order; checkSmithReward compares ordinals.
pub const BARS: [&str; 9] = [
    "BRONZE", "BLURITE", "IRON", "SILVER", "STEEL", "GOLD", "MITHRIL", "ADAMANT", "RUNITE",
];
pub const STEEL: usize = 4;
pub const MITHRIL: usize = 6;
pub const ADAMANT: usize = 7;

/// Every call the diary classes make outside them.
pub trait Host {
    type Player: Clone;
    /// `player.getAchievementDiaryManager()`'s diaries. The borrow ends
    /// before any service call.
    fn diaries(&mut self, player: &Self::Player) -> Result<&mut Manager>;
    /// `AchievementDiary.completedLevels`, shared by every player.
    fn completed_levels(&mut self) -> &mut Vec<i32>;
    /// `getPacketDispatch().sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Player,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `player.sendMessage(text)`.
    fn message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `player.sendMessages(lines...)`.
    fn messages(&mut self, player: &Self::Player, lines: &[String]) -> Result<()>;
    /// `NPCDefinition.forId(id).getName()`; None is null.
    fn npc_name(&mut self, id: i32) -> Result<Option<String>>;
    /// `getInterfaceManager().openTab(slot, new Component(id))`.
    fn open_tab(&mut self, player: &Self::Player, slot: i32, id: i32) -> Result<()>;
    /// `getInterfaceManager().isOpened()`.
    fn is_opened(&mut self, player: &Self::Player) -> Result<bool>;
    /// `getInterfaceManager().open(new Component(id))`.
    fn open_component(&mut self, player: &Self::Player, id: i32) -> Result<()>;
    /// `getPacketDispatch().sendRunScript(id, types, args...)`.
    fn run_script(
        &mut self,
        player: &Self::Player,
        id: i32,
        types: &str,
        args: &[i32],
    ) -> Result<()>;
    /// `isArtificial()`.
    fn is_artificial(&mut self, player: &Self::Player) -> Result<bool>;
    /// `getAttribute("debug", false)` unboxed (a non-boolean fails the cast).
    fn debug_enabled(&mut self, player: &Self::Player) -> Result<bool>;
    /// `packetDispatch.sendMessage(text)` from `debug`.
    fn debug_message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `containsItem(item)` of the container.
    fn contains(&mut self, player: &Self::Player, kind: Kind, item: Item) -> Result<bool>;
    /// `remove(item)` of the container.
    fn remove(&mut self, player: &Self::Player, kind: Kind, item: Item) -> Result<bool>;
    /// `add(item)` of the container.
    fn add(&mut self, player: &Self::Player, kind: Kind, item: Item) -> Result<bool>;
    /// `freeSlots()` of the container.
    fn free_slots(&mut self, player: &Self::Player, kind: Kind) -> Result<i32>;
    /// `get(slot)` of the container.
    fn get(&mut self, player: &Self::Player, kind: Kind, slot: i32) -> Result<Option<Item>>;
    /// `Item.getName()` of an item with this id; None is null.
    fn item_name(&mut self, id: i32) -> Result<Option<String>>;
}

fn at<T: Copy>(values: &[T], index: i32) -> Result<T> {
    usize::try_from(index)
        .ok()
        .and_then(|i| values.get(i).copied())
        .ok_or(Error::Index)
}
fn kind(kind: Option<usize>) -> Result<usize> {
    kind.ok_or(Error::Null)
}

// ---------------------------------------------------------------- DiaryType

/// `getAchievements(level)`.
pub fn achievements(kind: usize, level: i32) -> Result<&'static [&'static str]> {
    at(TYPES[kind].achievements, level)
}
/// `getRewards(level)`.
pub fn rewards(kind: usize, level: i32) -> Result<&'static [Item]> {
    at(TYPES[kind].rewards, level)
}
/// `getNpc(level)`.
pub fn npc(kind: usize, level: i32) -> Result<i32> {
    at(TYPES[kind].npcs, level)
}
/// `DiaryType.forChild(child)`: a type whose child is at most three below.
pub fn for_child(child: i32) -> Option<usize> {
    TYPES
        .iter()
        .position(|t| (0..4).any(|i| child == t.child.wrapping_add(i)))
}
/// `DiaryType.hasRewardEquipment(player)`: a held reward that is not a lamp.
pub fn has_reward_equipment<H: Host>(h: &mut H, player: &H::Player, kind: usize) -> Result<bool> {
    for tier in TYPES[kind].rewards {
        for &item in *tier {
            let name = h.item_name(item.0)?.ok_or(Error::Null)?;
            if !lowercase_root(&name).contains("lamp") && has_item(h, player, item)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// `Player.hasItem(item)`: inventory, bank, then equipment.
pub fn has_item<H: Host>(h: &mut H, player: &H::Player, item: Item) -> Result<bool> {
    Ok(h.contains(player, Kind::Inventory, item)?
        || h.contains(player, Kind::Bank, item)?
        || h.contains(player, Kind::Equipment, item)?)
}

// ---------------------------------------------------------------- AchievementDiary state

/// `isStarted(level)`.
pub fn started(diary: &Diary, level: i32) -> Result<bool> {
    at(&diary.started, level)
}
/// `isStarted()`: any of the level names' levels.
pub fn is_started(diary: &Diary) -> Result<bool> {
    for j in 0..TYPES[diary.kind].level_names.len() as i32 {
        if started(diary, j)? {
            return Ok(true);
        }
    }
    Ok(false)
}
/// `isComplete(level, index)`.
pub fn task_complete(diary: &Diary, level: i32, index: i32) -> Result<bool> {
    let row = usize::try_from(level)
        .ok()
        .and_then(|l| diary.completed.get(l))
        .ok_or(Error::Index)?;
    at(row, index)
}
/// `isComplete(level)`: every task of the level.
pub fn level_complete(diary: &Diary, level: i32) -> Result<bool> {
    for i in 0..achievements(diary.kind, level)?.len() as i32 {
        if !task_complete(diary, level, i)? {
            return Ok(false);
        }
    }
    Ok(true)
}
/// `isComplete(level, cumulative)`: with every lower level too.
pub fn cumulative_complete(diary: &Diary, level: i32, cumulative: bool) -> Result<bool> {
    if level_complete(diary, level)? {
        Ok(!cumulative || level <= 0 || cumulative_complete(diary, level - 1, true)?)
    } else {
        Ok(false)
    }
}
/// `isComplete()`: every task of every level.
pub fn is_complete(diary: &Diary) -> Result<bool> {
    for i in 0..diary.completed.len() as i32 {
        for x in 0..achievements(diary.kind, i)?.len() as i32 {
            if !task_complete(diary, i, x)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
/// `getLevel()`: the highest complete level, or -1.
pub fn completed_level(diary: &Diary) -> Result<i32> {
    for level in [2, 1, 0] {
        if level_complete(diary, level)? {
            return Ok(level);
        }
    }
    Ok(-1)
}
/// `getReward()`: the highest rewarded level, or -1.
pub fn reward_level(diary: &Diary) -> Result<i32> {
    for level in [2, 1, 0] {
        if rewarded(diary, level)? {
            return Ok(level);
        }
    }
    Ok(-1)
}
/// `getLevel(level)`: the level's name.
pub fn level_name(diary: &Diary, level: i32) -> Result<&'static str> {
    at(TYPES[diary.kind].level_names, level)
}
/// `getStatus(level)`.
pub fn status(diary: &Diary, level: i32) -> Result<&'static str> {
    Ok(if !started(diary, level)? {
        RED
    } else if level_complete(diary, level)? {
        GREEN
    } else {
        YELLOW
    })
}
/// `isLevelRewarded(level)`.
pub fn rewarded(diary: &Diary, level: i32) -> Result<bool> {
    at(&diary.rewarded, level)
}
/// `setLevelStarted(level)`.
pub fn set_level_started(diary: &mut Diary, level: i32) -> Result<()> {
    let level = usize::try_from(level).map_err(|_| Error::Index)?;
    *diary.started.get_mut(level).ok_or(Error::Index)? = true;
    Ok(())
}
/// `setCompleted(level, index)`.
pub fn set_completed(diary: &mut Diary, level: i32, index: i32) -> Result<()> {
    set_task(diary, level, index, true)
}
/// `setLevelRewarded(level)`.
pub fn set_level_rewarded(diary: &mut Diary, level: i32) -> Result<()> {
    let level = usize::try_from(level).map_err(|_| Error::Index)?;
    *diary.rewarded.get_mut(level).ok_or(Error::Index)? = true;
    Ok(())
}
fn set_task(diary: &mut Diary, level: i32, index: i32, value: bool) -> Result<()> {
    let row = usize::try_from(level)
        .ok()
        .and_then(|l| diary.completed.get_mut(l))
        .ok_or(Error::Index)?;
    let index = usize::try_from(index).map_err(|_| Error::Index)?;
    *row.get_mut(index).ok_or(Error::Index)? = value;
    Ok(())
}

/// The diary of `owner` of this type (`getDiary`), cloned for reading.
fn diary<H: Host>(h: &mut H, owner: &H::Player, kind: usize) -> Result<Diary> {
    Ok(diary_mut(h, owner, kind)?.clone())
}
fn diary_mut<'a, H: Host>(h: &'a mut H, owner: &H::Player, kind: usize) -> Result<&'a mut Diary> {
    h.diaries(owner)?
        .diaries
        .iter_mut()
        .find(|d| d.kind == kind)
        .ok_or(Error::Null)
}

// ---------------------------------------------------------------- AchievementDiary

/// The private `sendString`: `<blue>` and `<red>` become their colours.
fn send<H: Host>(h: &mut H, player: &H::Player, text: &str, child: i32) -> Result<()> {
    let text = text.replace("<blue>", BLUE).replace("<red>", RED);
    h.send_string(player, &text, DIARY_COMPONENT, child)
}

/// `open(player)`: 311 lines cleared, the title, the status line, the start
/// info for a diary not started, each level's status line and its tasks
/// (struck through when done, split on `<br><br>`, each followed by `*`),
/// the interface when nothing is open, and the scroll bar script.
pub fn open<H: Host>(h: &mut H, owner: &H::Player, player: &H::Player, kind: usize) -> Result<()> {
    for i in 0..311 {
        h.send_string(player, "", DIARY_COMPONENT, i)?;
    }
    let t = &TYPES[kind];
    send(
        h,
        player,
        &format!("<red>Achievement Diary - {}", t.name),
        2,
    )?;
    let mut child = 12;
    let d = diary(h, owner, kind)?;
    let colour = if is_complete(&d)? {
        GREEN
    } else if is_started(&d)? {
        YELLOW
    } else {
        "<red>"
    };
    send(h, player, &format!("{colour}{} Area Tasks", t.name), child)?;
    child += 1;
    if !t.info.is_empty() && !is_started(&diary(h, owner, kind)?)? {
        send(h, player, t.info, child)?;
        child += 1;
        child += java_split(t.info, "<br><br>").len() as i32;
    }
    child += 1;
    for level in 0..t.achievements.len() as i32 {
        let d = diary(h, owner, kind)?;
        let line = format!("{}{}", status(&d, level)?, level_name(&d, level)?);
        send(h, player, &line, child)?;
        child += 2;
        for i in 0..achievements(kind, level)?.len() as i32 {
            let complete = task_complete(&diary(h, owner, kind)?, level, i)?;
            let line = achievements(kind, level)?[i as usize];
            if line.contains("<br><br>") {
                for part in java_split(line, "<br><br>") {
                    let text = if complete {
                        format!("<str><str>{part}")
                    } else {
                        part.to_owned()
                    };
                    send(h, player, &text, child)?;
                    child += 1;
                }
            } else {
                let text = if complete {
                    format!("<str><str>{line}")
                } else {
                    line.to_owned()
                };
                send(h, player, &text, child)?;
                child += 1;
            }
            send(h, player, "*", child)?;
            child += 1;
        }
        child += 1;
    }
    if !h.is_opened(player)? {
        h.open_component(player, DIARY_COMPONENT)?;
    }
    h.run_script(player, 1207, "ii", &[1, child - 10])
}

/// `drawStatus(player)`: for a started diary, its name and the three level
/// lines on the diary tab.
pub fn draw_status<H: Host>(
    h: &mut H,
    owner: &H::Player,
    player: &H::Player,
    kind: usize,
) -> Result<()> {
    let t = &TYPES[kind];
    if is_started(&diary(h, owner, kind)?)? {
        let colour = if is_complete(&diary(h, owner, kind)?)? {
            GREEN
        } else {
            YELLOW
        };
        h.send_string(player, &format!("{colour}{}", t.name), TAB, t.child)?;
        for i in 0..3 {
            let d = diary(h, owner, kind)?;
            let colour = if level_complete(&d, i)? {
                GREEN
            } else if started(&d, i)? {
                YELLOW
            } else {
                "<col=FF0000>"
            };
            let text = format!("{colour}{}", level_name(&d, i)?);
            h.send_string(player, &text, TAB, t.child + (i + 1))?;
        }
    }
    Ok(())
}

/// `updateTask(player, level, index, complete)`.
#[allow(clippy::too_many_arguments)]
pub fn update_task<H: Host>(
    h: &mut H,
    owner: &H::Player,
    player: &H::Player,
    kind: usize,
    level: i32,
    index: i32,
    complete: bool,
) -> Result<()> {
    let name = TYPES[kind].name;
    {
        let d = diary_mut(h, owner, kind)?;
        if !started(d, level)? {
            set_level_started(d, level)?;
        }
    }
    if !complete {
        h.message(
            player,
            &format!("Well done! A {name} task has been updated."),
        )?;
    } else {
        set_task(diary_mut(h, owner, kind)?, level, index, true)?;
        let temp = if TYPES[kind].constant == "LUMBRIDGE" {
            level - 1
        } else {
            level
        };
        let grade = match temp {
            -1 => "a beginner",
            0 => "an easy",
            1 => "a medium",
            _ => "a hard",
        };
        h.messages(
            player,
            &[
                format!("Well done! You have completed {grade} task in the {name} area. Your"),
                "Achievement Diary has been updated.".into(),
            ],
        )?;
    }
    let d = diary(h, owner, kind)?;
    if level_complete(&d, level)? {
        let first = format!(
            "Congratulations! You have completed all of the {} tasks in the {name} area.",
            lowercase_root(level_name(&d, level)?)
        );
        let npc = npc(kind, level)?;
        let npc_name = h.npc_name(npc)?.unwrap_or_else(|| "null".into());
        h.messages(
            player,
            &[first, format!("Speak to {npc_name} to claim your reward.")],
        )?;
    }
    draw_status(h, owner, player, kind)
}

/// `finishTask(player, level, index)`: an unfinished task updated as
/// complete; the level joins the shared list when all 25 slots are set,
/// otherwise its first entry leaves it.
#[allow(clippy::too_many_arguments)]
pub fn finish_task_diary<H: Host>(
    h: &mut H,
    owner: &H::Player,
    player: &H::Player,
    kind: usize,
    level: i32,
    index: i32,
) -> Result<()> {
    if !task_complete(&diary(h, owner, kind)?, level, index)? {
        update_task(h, owner, player, kind, level, index, true)?;
        let d = diary(h, owner, kind)?;
        let row = usize::try_from(level)
            .ok()
            .and_then(|l| d.completed.get(l))
            .ok_or(Error::Index)?;
        let complete = row.iter().all(|t| *t);
        let list = h.completed_levels();
        if complete {
            list.push(level);
        } else if let Some(position) = list.iter().position(|l| *l == level) {
            list.remove(position);
        }
    }
    Ok(())
}

/// `resetTask(player, level, index)`.
pub fn reset_task<H: Host>(
    h: &mut H,
    owner: &H::Player,
    player: &H::Player,
    kind: usize,
    level: i32,
    index: i32,
) -> Result<()> {
    {
        let d = diary_mut(h, owner, kind)?;
        set_task(d, level, index, false)?;
        if !started(d, level)? {
            let at = usize::try_from(level).map_err(|_| Error::Index)?;
            d.started[at] = false;
        }
        if !level_complete(d, level)? {
            let at = usize::try_from(level).map_err(|_| Error::Index)?;
            *d.rewarded.get_mut(at).ok_or(Error::Index)? = false;
        }
    }
    draw_status(h, owner, player, kind)
}

/// `removeRewardsFor(player, type, level)`: the level's main reward taken
/// from the inventory, else the bank, else the equipment; a debug line.
pub fn remove_rewards_for<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    let reward = rewards(kind, level)?[0];
    let removed = h.remove(player, Kind::Inventory, reward)?
        || h.remove(player, Kind::Bank, reward)?
        || h.remove(player, Kind::Equipment, reward)?;
    if removed && h.debug_enabled(player)? {
        h.debug_message(player, "Removed previous reward")?;
    }
    Ok(removed)
}

/// `addRewardsFor(player, type, level)`: every reward of the level when the
/// inventory has the room; a failed add takes all of them out again.
pub fn add_rewards_for<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    let rewards = rewards(kind, level)?;
    let free = h.free_slots(player, Kind::Inventory)?;
    if free < rewards.len() as i32 {
        return Ok(false);
    }
    let mut all = true;
    for &reward in rewards {
        all &= h.add(player, Kind::Inventory, reward)?;
    }
    if !all {
        for &item in rewards {
            h.remove(player, Kind::Inventory, item)?;
        }
    }
    Ok(all)
}

/// `flagRewarded(player, type, level)`.
pub fn flag_rewarded<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    if level > 0 {
        remove_rewards_for(h, player, kind, level - 1)?;
    }
    if add_rewards_for(h, player, kind, level)? {
        set_level_rewarded(diary_mut(h, player, kind)?, level)?;
    } else {
        h.message(
            player,
            "You do not have enough space in your inventory to claim these rewards.",
        )?;
        return Ok(false);
    }
    Ok(true)
}

/// `canReplaceReward(player, type, level)`.
pub fn can_replace_reward<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    let reward = rewards(kind, level)?[0];
    let claimed = has_completed_level(h, player, kind, level)?
        && has_claimed_level_rewards(h, player, kind, level)?
        && !has_item(h, player, reward)?;
    Ok(if level == 2 {
        claimed
    } else {
        claimed && !has_claimed_level_rewards(h, player, kind, level + 1)?
    })
}

/// `grantReplacement(player, type, level)`.
pub fn grant_replacement<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    let reward = rewards(kind, level)?[0];
    Ok(can_replace_reward(h, player, kind, level)? && h.add(player, Kind::Inventory, reward)?)
}

/// `hasCompletedLevel(player, type, level)`.
pub fn has_completed_level<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    if level > TYPES[kind].level_names.len() as i32 - 1 {
        return Ok(false);
    }
    cumulative_complete(&diary(h, player, kind)?, level, true)
}

/// `hasClaimedLevelRewards(player, type, level)`.
pub fn has_claimed_level_rewards<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    rewarded(&diary(h, player, kind)?, level)
}

/// `canClaimLevelRewards(player, type, level)`.
pub fn can_claim_level_rewards<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: usize,
    level: i32,
) -> Result<bool> {
    if level == 2 {
        Ok(has_completed_level(h, player, kind, level)?
            && !has_claimed_level_rewards(h, player, kind, level)?)
    } else {
        Ok(!has_claimed_level_rewards(h, player, kind, level + 1)?
            && has_completed_level(h, player, kind, level)?
            && !has_claimed_level_rewards(h, player, kind, level)?)
    }
}

// ---------------------------------------------------------------- AchievementDiaryManager

/// `parse(data)`: the persisted parser, with the shared completed list.
pub fn parse<H: Host>(h: &mut H, owner: &H::Player, data: &[Json]) -> Result<()> {
    let mut completed = std::mem::take(h.completed_levels());
    let result = h
        .diaries(owner)
        .and_then(|m| m.parse(data, &mut completed).map_err(Error::from));
    *h.completed_levels() = completed;
    result
}

/// `openTab()`: the tab, then each diary's status.
pub fn open_tab<H: Host>(h: &mut H, owner: &H::Player) -> Result<()> {
    h.open_tab(owner, 2, TAB)?;
    let kinds: Vec<usize> = h.diaries(owner)?.diaries.iter().map(|d| d.kind).collect();
    for kind in kinds {
        draw_status(h, owner, owner, kind)?;
    }
    Ok(())
}

/// The manager's `updateTask(player, type, level, index, complete)`.
#[allow(clippy::too_many_arguments)]
pub fn manager_update_task<H: Host>(
    h: &mut H,
    owner: &H::Player,
    player: &H::Player,
    kind: Option<usize>,
    level: i32,
    index: i32,
    complete: bool,
) -> Result<()> {
    update_task(h, owner, player, self::kind(kind)?, level, index, complete)
}

/// The manager's `finishTask(player, type, level, index)`: not for an
/// artificial player.
pub fn finish_task<H: Host>(
    h: &mut H,
    owner: &H::Player,
    player: &H::Player,
    kind: Option<usize>,
    level: i32,
    index: i32,
) -> Result<()> {
    if !h.is_artificial(player)? {
        finish_task_diary(h, owner, player, self::kind(kind)?, level, index)?;
    }
    Ok(())
}

/// `hasCompletedTask(type, level, index)`.
pub fn has_completed_task<H: Host>(
    h: &mut H,
    owner: &H::Player,
    kind: Option<usize>,
    level: i32,
    index: i32,
) -> Result<bool> {
    task_complete(&diary(h, owner, self::kind(kind)?)?, level, index)
}

/// `setStarted(type, level)`.
pub fn set_started<H: Host>(
    h: &mut H,
    owner: &H::Player,
    kind: Option<usize>,
    level: i32,
) -> Result<()> {
    set_level_started(diary_mut(h, owner, self::kind(kind)?)?, level)
}

/// The manager's `setCompleted(type, level, index)`.
pub fn manager_set_completed<H: Host>(
    h: &mut H,
    owner: &H::Player,
    kind: Option<usize>,
    level: i32,
    index: i32,
) -> Result<()> {
    set_completed(diary_mut(h, owner, self::kind(kind)?)?, level, index)
}

/// `hasGlove()`: Karamja gloves 1-3 in the hands slot.
pub fn has_glove<H: Host>(h: &mut H, owner: &H::Player) -> Result<bool> {
    let glove = h.get(owner, Kind::Equipment, SLOT_HANDS)?;
    Ok(glove.is_some_and(|(id, _)| (0..3).any(|l| TYPES[KARAMJA].rewards[l][0].0 == id)))
}

/// `hasArmour()`: Varrock armour 1-3 in the chest slot.
pub fn has_armour<H: Host>(h: &mut H, owner: &H::Player) -> Result<bool> {
    let plate = h.get(owner, Kind::Equipment, SLOT_CHEST)?;
    Ok(plate.is_some_and(|(id, _)| (0..3).any(|l| TYPES[VARROCK].rewards[l][0].0 == id)))
}

fn worn_level<H: Host>(h: &mut H, owner: &H::Player, kind: usize) -> Result<i32> {
    for (i, level) in TYPES[kind].rewards.iter().take(3).enumerate() {
        if h.contains(owner, Kind::Equipment, level[0])? {
            return Ok(i as i32);
        }
    }
    Ok(-1)
}

/// `getKaramjaGlove()`: the first glove level the equipment contains.
pub fn karamja_glove<H: Host>(h: &mut H, owner: &H::Player) -> Result<i32> {
    if !has_glove(h, owner)? {
        return Ok(-1);
    }
    worn_level(h, owner, KARAMJA)
}

/// `getArmour()`: the first armour level the equipment contains.
pub fn armour<H: Host>(h: &mut H, owner: &H::Player) -> Result<i32> {
    if !has_armour(h, owner)? {
        return Ok(-1);
    }
    worn_level(h, owner, VARROCK)
}

/// `checkMiningReward(reward)`: coal always, otherwise ores up to the
/// armour's level.
pub fn check_mining_reward<H: Host>(h: &mut H, owner: &H::Player, reward: i32) -> Result<bool> {
    let level = armour(h, owner)?;
    if level == -1 {
        return Ok(false);
    }
    if reward == 453 {
        return Ok(true);
    }
    Ok(level == 0 && reward <= 442 || level == 1 && reward <= 447 || level == 2 && reward <= 449)
}

/// `checkSmithReward(bar)`: bars up to steel, mithril or adamant with the
/// armour's level; `bar` is the Bar ordinal, None a null bar.
pub fn check_smith_reward<H: Host>(
    h: &mut H,
    owner: &H::Player,
    bar: Option<usize>,
) -> Result<bool> {
    let level = armour(h, owner)?;
    if level == -1 {
        return Ok(false);
    }
    let ordinal = || bar.ok_or(Error::Null);
    Ok(level == 0 && ordinal()? <= STEEL
        || level == 1 && ordinal()? <= MITHRIL
        || level == 2 && ordinal()? <= ADAMANT)
}

/// The manager's `isComplete(type)`.
pub fn diary_complete<H: Host>(h: &mut H, owner: &H::Player, kind: Option<usize>) -> Result<bool> {
    let kind = self::kind(kind)?;
    let d = h
        .diaries(owner)?
        .diaries
        .get(kind)
        .cloned()
        .ok_or(Error::Index)?;
    is_complete(&d)
}

/// `resetRewards()`: every reward of every diary taken from the inventory,
/// the bank and the equipment where each contains it.
pub fn reset_rewards<H: Host>(h: &mut H, owner: &H::Player) -> Result<()> {
    let kinds: Vec<usize> = h.diaries(owner)?.diaries.iter().map(|d| d.kind).collect();
    for kind in kinds {
        for axis in TYPES[kind].rewards {
            for &item in *axis {
                for container in [Kind::Inventory, Kind::Bank, Kind::Equipment] {
                    if h.contains(owner, container, item)? {
                        h.remove(owner, container, item)?;
                    }
                }
            }
        }
    }
    Ok(())
}
