//! The quest tab: `QuestRepository.syncronizeTab`, `forButtonId`,
//! `forIndex` and the base `Quest.getConfig`.
//!
//! The stages and points are the player's persisted
//! [`Repository`](crate::player::persisted::quests::Repository) against the
//! world's [`Registry`]; their other members (setStage, syncPoints,
//! isComplete, ...) are ported there. A quest's `getConfig` is virtual
//! (seven quests override it) and its `updateVarps` is content: both are
//! required [`Host`] services. [`base_config`] is the base class's
//! `getConfig`, which a host answers with for a quest that does not override it.
use super::{Error, Result};
use crate::player::persisted::quests::{Quest, Registry, Repository};

/// Every call `syncronizeTab` makes outside the repository, for the player it
/// is given (every call site passes the repository's own player).
pub trait Host {
    /// `ContentAPI.setVarp(player, id, value, save)`.
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()>;
    /// `ContentAPI.setVarbit(player, id, value, save)`.
    fn varbit(&mut self, id: i32, value: i32, save: bool) -> Result<()>;
    /// `quest.getConfig(player, stage)`, dispatched to the quest's class.
    fn quest_config(&mut self, quest: &Quest, stage: i32) -> Result<Vec<i32>>;
    /// `quest.updateVarps(player)`: content.
    fn update_varps(&mut self, quest: &Quest) -> Result<()>;
}

/// The base `Quest.getConfig(player, stage)` over the quest's constructor
/// `configs`: four values are a varp and its three stage values, five a
/// varbit (after an ignored varp) and its three stage values. Fewer than four
/// throws `IndexOutOfBoundsException`.
pub fn base_config(configs: &[i32], stage: i32) -> Result<Vec<i32>> {
    if configs.len() < 4 {
        return Err(Error::Index);
    }
    let pick = |first: usize| {
        if stage == 0 {
            configs[first]
        } else if stage >= 100 {
            configs[first + 2]
        } else {
            configs[first + 1]
        }
    };
    if configs.len() >= 5 {
        return Ok(vec![configs[0], configs[1], pick(2)]);
    }
    Ok(vec![configs[0], pick(1)])
}

/// `syncronizeTab(player)`: the points varp, then each registered quest in
/// the registry's order: its config as a varbit (three values) or a varp,
/// then its `updateVarps`.
pub fn syncronize_tab<H: Host>(
    h: &mut H,
    repository: &Repository,
    registry: &Registry,
) -> Result<()> {
    h.varp(101, repository.points, false)?;
    for quest in registry.values() {
        let stage = repository.stage(quest)?;
        let config = h.quest_config(quest, stage)?;
        if config.len() == 3 {
            h.varbit(config[1], config[2], false)?;
        } else {
            let id = *config.first().ok_or(Error::Index)?;
            let value = *config.get(1).ok_or(Error::Index)?;
            h.varp(id, value, false)?;
        }
        h.update_varps(quest)?;
    }
    Ok(())
}

/// `forButtonId(buttonId)`: the first quest in the registry's order.
pub fn for_button(registry: &Registry, button: i32) -> Option<&Quest> {
    registry.values().find(|q| q.button == button)
}

/// `forIndex(index)`: the first quest in the registry's order.
pub fn for_index(registry: &Registry, index: i32) -> Option<&Quest> {
    registry.values().find(|q| q.index == index)
}
