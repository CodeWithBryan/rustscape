//! QuestRepository: the per-player map of quest index to stage and the point
//! total, against the server-wide registry of quests. Quest content (journals,
//! rewards, varps, tab synchronisation) is not ported.
use super::{IntHashMap, java_array, java_object, java_text, parse_int};
use crate::player::save::{Error, Level, Quests as Saved, Result};
use rs09_game_core::java_json::Json;
use std::collections::BTreeMap;

/// One registered Quest: the constructor values the repository reads.
#[derive(Clone, Debug)]
pub struct Quest {
    /// `Quests.toString()`, which is the quest's display name.
    pub name: String,
    pub index: i32,
    pub button: i32,
    pub points: i32,
}
/// QuestRepository.QUESTS: a TreeMap keyed by the `Quests` constant, so its
/// values iterate in that enum's declaration order. Registration is world
/// start-up content; what is registered is the caller's input.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    quests: BTreeMap<i32, Quest>,
}
impl Registry {
    /// `register`: a second quest for the same constant replaces the first.
    pub fn register(&mut self, ordinal: i32, quest: Quest) {
        self.quests.insert(ordinal, quest);
    }
    pub fn get(&self, ordinal: i32) -> Option<&Quest> {
        self.quests.get(&ordinal)
    }
    pub fn values(&self) -> impl Iterator<Item = &Quest> {
        self.quests.values()
    }
    pub fn clear(&mut self) {
        self.quests.clear();
    }
    /// getAvailablePoints, with the original int wrapping.
    pub fn available_points(&self) -> i32 {
        self.values()
            .fold(0i32, |sum, q| sum.wrapping_add(q.points))
    }
}

#[derive(Clone, Debug)]
pub struct Repository {
    pub stages: IntHashMap<i32>,
    pub points: i32,
}
impl Repository {
    /// The constructor puts stage 0 for every quest registered at that moment.
    pub fn new(registry: &Registry) -> Self {
        let mut stages = IntHashMap::default();
        for quest in registry.values() {
            stages.put(quest.index, 0);
        }
        Self { stages, points: 0 }
    }
    /// getStage(Quest): an index the map lacks fails unboxing.
    pub fn stage(&self, quest: &Quest) -> Result<i32> {
        self.stages.get(quest.index).copied().ok_or(Error::Null)
    }
    /// getStage(Quests): 0 for an unregistered quest.
    pub fn stage_of(&self, registry: &Registry, ordinal: i32) -> Result<i32> {
        match registry.get(ordinal) {
            None => Ok(0),
            Some(quest) => self.stage(quest),
        }
    }
    /// setStage: only an increase is stored; anything else logs a warning.
    pub fn set_stage(
        &mut self,
        quest: &Quest,
        stage: i32,
        player: &str,
        log: &mut dyn FnMut(Level, &str),
    ) -> Result<()> {
        let old = self.stage(quest)?;
        if old < stage {
            self.stages.put(quest.index, stage);
        } else {
            log(
                Level::Warn,
                &format!(
                    "Nonmonotonic QuestRepository.setStage call for player \"{player}\", quest \"{}\", old stage {old}, new stage {stage}",
                    quest.name
                ),
            );
        }
        Ok(())
    }
    pub fn set_stage_nonmonotonic(&mut self, quest: &Quest, stage: i32) {
        self.stages.put(quest.index, stage);
    }
    pub fn increment_points(&mut self, value: i32) {
        self.points = self.points.wrapping_add(value);
    }
    pub fn dock_points(&mut self, value: i32) {
        self.points = self.points.wrapping_sub(value);
    }
    /// syncPoints: the total is published only after every quest was read.
    pub fn sync_points(&mut self, registry: &Registry) -> Result<()> {
        let mut points = 0i32;
        for quest in registry.values() {
            if self.stage(quest)? >= 100 {
                points = points.wrapping_add(quest.points);
            }
        }
        self.points = points;
        Ok(())
    }
    /// isComplete(Quests): an unregistered quest logs an error and is not
    /// complete. `name` is that `Quests` constant's toString, its display name.
    pub fn is_complete(
        &self,
        registry: &Registry,
        ordinal: i32,
        name: &str,
        log: &mut dyn FnMut(Level, &str),
    ) -> Result<bool> {
        match registry.get(ordinal) {
            None => {
                log(
                    Level::Err,
                    &format!("Error can't check if quest is complete for {name}"),
                );
                Ok(false)
            }
            Some(quest) => Ok(self.stage(quest)? >= 100),
        }
    }
    /// hasStarted(Quests) checks the argument, not the lookup, for null, so an
    /// unregistered quest fails on the null quest.
    pub fn has_started(&self, registry: &Registry, ordinal: i32) -> Result<bool> {
        Ok(self.stage(registry.get(ordinal).ok_or(Error::Null)?)? > 0)
    }
    pub fn has_completed_all(&self, registry: &Registry) -> bool {
        self.points >= registry.available_points()
    }
    /// What saveQuests reads: the points and the map in its iteration order.
    pub fn saved(&self) -> Saved {
        Saved {
            points: self.points,
            stages: self.stages.iter().map(|(id, stage)| (id, *stage)).collect(),
        }
    }
    /// QuestRepository.parse. Points are stored before the stages are read
    /// and then recomputed by syncPoints, which fails for a registered quest
    /// whose index the map lacks.
    pub fn parse(&mut self, data: &Json, registry: &Registry) -> Result<()> {
        self.points = parse_int(&java_text(data.get("points"))?)?;
        let stages = java_array(data.get("questStages"))?.ok_or(Error::Null)?;
        for quest in stages {
            let quest = java_object(Some(quest))?.ok_or(Error::Null)?;
            let id = parse_int(&java_text(quest.get("questId"))?)?;
            let stage = parse_int(&java_text(quest.get("questStage"))?)?;
            self.stages.put(id, stage);
        }
        self.sync_points(registry)
    }
}
