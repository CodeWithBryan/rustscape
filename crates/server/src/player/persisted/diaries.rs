//! AchievementDiaryManager and AchievementDiary as persisted: per diary the
//! started and rewarded flags of its three levels and a 25-task bitset per
//! level. Task updates, messages, the diary interface and reward claiming are
//! not ported; resetRewards, which the parser runs for a file without diaries,
//! is.
use super::{java_array, java_bool, java_object};
use crate::{
    definitions::item::Registry,
    inventory::{
        container::{Container, Context},
        item::Item,
    },
    player::{
        core_data::CoreData,
        save::{Diary as Saved, Error, Result},
    },
};
use rs09_game_core::{
    java_json::{Json, key_order},
    java_string::equals_ignore_case,
};

pub const LEVELS: usize = 3;
pub const TASKS: usize = 25;
/// One DiaryType constant: its name, display name and the reward items of
/// each level (id, amount, charge), as the pinned enum declares them.
pub struct Type {
    pub name: &'static str,
    pub display: &'static str,
    pub levels: usize,
    pub rewards: [[(i32, i32, i32); 2]; 3],
}
const fn rewards(ids: [[i32; 2]; 3]) -> [[(i32, i32, i32); 2]; 3] {
    let mut out = [[(0, 1, 1000); 2]; 3];
    let mut level = 0;
    while level < 3 {
        out[level][0].0 = ids[level][0];
        out[level][1].0 = ids[level][1];
        level += 1;
    }
    out
}
pub const TYPES: [Type; 6] = [
    Type {
        name: "KARAMJA",
        display: "Karamja",
        levels: 3,
        rewards: rewards([[11136, 11137], [11138, 11139], [11140, 11141]]),
    },
    Type {
        name: "VARROCK",
        display: "Varrock",
        levels: 3,
        rewards: rewards([[11756, 11753], [11757, 11754], [11758, 11755]]),
    },
    Type {
        name: "LUMBRIDGE",
        display: "Lumbridge",
        levels: 3,
        rewards: rewards([[13560, 11185], [13561, 11186], [13562, 11187]]),
    },
    Type {
        name: "FALADOR",
        display: "Falador",
        levels: 3,
        rewards: rewards([[14577, 14580], [14578, 14581], [14579, 14582]]),
    },
    Type {
        name: "FREMENNIK",
        display: "Fremennik",
        levels: 3,
        rewards: rewards([[14571, 14574], [14572, 14575], [14573, 14576]]),
    },
    Type {
        name: "SEERS_VILLAGE",
        display: "Seers' Village",
        levels: 3,
        rewards: rewards([[14631, 14633], [14631, 14634], [14631, 14635]]),
    },
];

#[derive(Clone, Debug)]
pub struct Diary {
    pub kind: usize,
    pub started: [bool; LEVELS],
    pub rewarded: [bool; LEVELS],
    /// `new boolean[type.getAchievements().length][25]`.
    pub completed: Vec<[bool; TASKS]>,
}
impl Diary {
    pub fn new(kind: usize) -> Self {
        Self {
            kind,
            started: [false; LEVELS],
            rewarded: [false; LEVELS],
            completed: vec![[false; TASKS]; TYPES[kind].levels],
        }
    }
    /// AchievementDiary.parse. `completed_levels` is the class's static list,
    /// shared by every player: each stored task appends its level.
    pub fn parse(&mut self, data: Option<&Json>, completed_levels: &mut Vec<i32>) -> Result<()> {
        let data = data.ok_or(Error::Null)?;
        let started = java_array(data.get("startedLevels"))?.ok_or(Error::Null)?;
        for (i, value) in started.iter().enumerate() {
            // The cast precedes the store's bounds check.
            let value = java_bool(Some(value))?;
            *self.started.get_mut(i).ok_or(Error::Index)? = value;
        }
        let completed = java_array(data.get("completedLevels"))?.ok_or(Error::Null)?;
        for (i, level) in completed.iter().enumerate() {
            let level = java_array(Some(level))?.ok_or(Error::Null)?;
            for (j, value) in level.iter().enumerate() {
                // `taskCompleted[i]` is evaluated before the value, `[j]` after it.
                if i >= self.completed.len() {
                    return Err(Error::Index);
                }
                let value = java_bool(Some(value))?;
                *self.completed[i].get_mut(j).ok_or(Error::Index)? = value;
                completed_levels.push(i as i32);
            }
        }
        let rewarded = java_array(data.get("rewardedLevels"))?.ok_or(Error::Null)?;
        for (i, value) in rewarded.iter().enumerate() {
            let value = java_bool(Some(value))?;
            *self.rewarded.get_mut(i).ok_or(Error::Index)? = value;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Manager {
    pub diaries: Vec<Diary>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            diaries: (0..TYPES.len()).map(Diary::new).collect(),
        }
    }
}
impl Manager {
    /// What saveAchievementData reads, in array order under the constant name.
    pub fn saved(&self) -> Vec<Saved> {
        self.diaries
            .iter()
            .map(|d| Saved {
                name: TYPES[d.kind].name.to_owned(),
                started: d.started.to_vec(),
                completed: d.completed.iter().map(|level| level.to_vec()).collect(),
                rewarded: d.rewarded.to_vec(),
            })
            .collect()
    }
    /// AchievementDiaryManager.parse. Each element's first key in HashMap order
    /// names the diary, with `_` read as `' `; the diary's object is then
    /// fetched under that name with `' ` turned back into `_`.
    pub fn parse(&mut self, data: &[Json], completed_levels: &mut Vec<i32>) -> Result<()> {
        for diary in data {
            let diary = java_object(Some(diary))?.ok_or(Error::Null)?;
            let Json::Object(entries) = diary else {
                unreachable!()
            };
            let first = *key_order(entries).first().ok_or(Error::Index)?;
            let name = entries[first].0.replace('_', "' ");
            for index in 0..self.diaries.len() {
                if equals_ignore_case(TYPES[self.diaries[index].kind].display, &name) {
                    let value = java_object(diary.get(&name.replace("' ", "_")))?;
                    self.diaries[index].parse(value, completed_levels)?;
                }
            }
        }
        Ok(())
    }
    /// resetRewards: every reward of every level is removed from the
    /// inventory, the active bank and the equipment where each contains it.
    pub fn reset_rewards(
        &self,
        core: &mut CoreData,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        for diary in &self.diaries {
            for level in &TYPES[diary.kind].rewards {
                for &(id, amount, charge) in level {
                    let item = crate::definitions::item::Shared::new(Item::new(
                        id,
                        amount,
                        charge,
                        definitions,
                    ));
                    let remove = |container: &mut Container, ctx: &mut _| -> Result<()> {
                        if container.contains(id, amount) {
                            container
                                .remove(Some(&item), None, true, definitions, ctx)
                                .map_err(Error::Inventory)?;
                        }
                        Ok(())
                    };
                    remove(&mut core.inventory, ctx)?;
                    remove(&mut core.bank_mut().container, ctx)?;
                    remove(&mut core.equipment, ctx)?;
                }
            }
        }
        Ok(())
    }
}
