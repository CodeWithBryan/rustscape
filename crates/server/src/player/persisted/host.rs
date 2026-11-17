//! A `save::Host` that answers the ported sections from native state.
use super::{
    diaries, emotes, familiars, graves, house, music, poh_storage, pouches, quests, spells,
};
use crate::{
    definitions::item::Registry,
    inventory::container::{Context, Factory},
    player::{
        attributes::{AttributeValue, GameAttributes},
        core_data::CoreData,
        save::{self, Autocast, Diary, Emotes, Familiars, House, Level, Quests, Result},
        settings::{self, Settings},
        skills::{self, Effect, EntityKind, Skills},
    },
};
use rs09_game_core::java_json::Json;

/// Server-wide state the persisted sections are read against or change:
/// registered quests, the song table, the spell books' spells, and
/// AchievementDiary's static `completedLevels` list, which every player's
/// diary parse appends to. All of it is world start-up input.
#[derive(Clone, Debug, Default)]
pub struct World {
    pub quests: quests::Registry,
    pub songs: music::Songs,
    pub spells: spells::Registry,
    pub completed_levels: Vec<i32>,
}

/// One character's persisted state for the systems behind `save::Host`.
pub struct Persisted {
    pub quests: quests::Repository,
    pub music: music::Player,
    pub emotes: emotes::Manager,
    pub diaries: diaries::Manager,
    pub spells: spells::State,
    pub pouches: pouches::Manager,
    pub poh: poh_storage::State,
    pub house: House,
    pub familiars: familiars::Manager,
}
impl Persisted {
    /// What a new Player constructs. The quest map takes the quests registered
    /// at that moment.
    pub fn new(world: &World, factory: &Factory) -> Result<Self> {
        Ok(Self {
            quests: quests::Repository::new(&world.quests),
            music: music::Player::default(),
            emotes: emotes::Manager::default(),
            diaries: diaries::Manager::default(),
            spells: spells::State::default(),
            pouches: pouches::Manager::new(factory)?,
            poh: poh_storage::State::default(),
            house: house::fresh(),
            familiars: familiars::Manager::default(),
        })
    }
}

/// The services still outside native state. Each is required: an
/// implementation must not report success for work it did not do.
pub trait Remaining: Context + skills::Context + settings::Context {
    fn log(&mut self, level: Level, message: &str);
    /// Properties.isRetaliating.
    fn retaliating(&mut self) -> Result<bool>;
    /// Entity.dispatch(AttributeSetEvent(player, key, value)) after an attribute write.
    fn attribute_set(&mut self, key: &str, value: &AttributeValue) -> Result<()>;
    /// The registered PersistPlayer content hooks.
    fn content_hooks(&mut self) -> usize;
    fn save_content(&mut self, hook: usize, root: &mut Json) -> Result<()>;
    fn parse_content(&mut self, hook: usize, root: &Json) -> Result<()>;
    /// FamiliarManager.parse.
    fn parse_familiars(&mut self, data: &Json) -> Result<()>;
    /// HouseManager.parse.
    fn parse_house(&mut self, data: &Json) -> Result<()>;
}

pub struct Native<'a, R> {
    pub state: &'a mut Persisted,
    pub world: &'a mut World,
    pub factory: &'a Factory,
    pub definitions: &'a Registry,
    pub rest: &'a mut R,
}
impl<R: Remaining> save::Host for Native<'_, R> {
    fn log(&mut self, level: Level, message: &str) {
        self.rest.log(level, message);
    }
    fn retaliating(&mut self) -> Result<bool> {
        Remaining::retaliating(self.rest)
    }
    fn quests(&mut self) -> Result<Quests> {
        Ok(self.state.quests.saved())
    }
    fn autocast(&mut self) -> Result<Option<Autocast>> {
        Ok(self.state.spells.autocast)
    }
    fn unlocked_music(&mut self) -> Result<Vec<i32>> {
        Ok(self.state.music.saved())
    }
    fn familiars(&mut self) -> Result<Familiars> {
        Ok(self.state.familiars.saved())
    }
    fn house(&mut self) -> Result<House> {
        Ok(self.state.house.clone())
    }
    fn diaries(&mut self) -> Result<Vec<Diary>> {
        Ok(self.state.diaries.saved())
    }
    fn emotes(&mut self) -> Result<Emotes> {
        Ok(self.state.emotes.saved())
    }
    fn save_pouches(&mut self, root: &mut Json) -> Result<()> {
        self.state.pouches.save(root)
    }
    fn poh_storage(&mut self) -> Result<Json> {
        Ok(self.state.poh.to_json())
    }
    fn content_hooks(&mut self) -> usize {
        self.rest.content_hooks()
    }
    fn save_content(&mut self, hook: usize, root: &mut Json) -> Result<()> {
        self.rest.save_content(hook, root)
    }
    fn parse_content(&mut self, hook: usize, root: &Json) -> Result<()> {
        self.rest.parse_content(hook, root)
    }
    fn parse_quests(&mut self, data: &Json) -> Result<()> {
        self.state.quests.parse(data, &self.world.quests)
    }
    fn update_grave_type(&mut self, attributes: &mut GameAttributes, ordinal: i32) -> Result<()> {
        let value = graves::update(attributes, ordinal);
        self.rest.attribute_set(graves::ATTRIBUTE, &value)
    }
    fn set_autocast(&mut self, book: i32, spell: i32) -> Result<()> {
        self.state.spells.set(&self.world.spells, book, spell)
    }
    fn unlock_music(&mut self, id: i32) -> Result<()> {
        self.state.music.put(&self.world.songs, id)
    }
    fn parse_familiars(&mut self, data: &Json) -> Result<()> {
        self.rest.parse_familiars(data)
    }
    fn parse_house(&mut self, data: &Json) -> Result<()> {
        self.rest.parse_house(data)
    }
    fn emote_unlocked(&mut self, ordinal: i32) -> Result<bool> {
        Ok(self.state.emotes.is_unlocked(ordinal))
    }
    fn unlock_emote(&mut self, ordinal: i32) -> Result<()> {
        self.state.emotes.emotes.push(ordinal);
        Ok(())
    }
    fn parse_diaries(&mut self, data: &Json) -> Result<()> {
        let Json::Array(values) = data else {
            return Err(save::Error::Type);
        };
        self.state
            .diaries
            .parse(values, &mut self.world.completed_levels)
    }
    fn reset_diary_rewards(&mut self, core: &mut CoreData, definitions: &Registry) -> Result<()> {
        self.state
            .diaries
            .reset_rewards(core, definitions, &mut *self.rest)
    }
    fn parse_pouches(&mut self, data: &Json) -> Result<()> {
        let Json::Array(values) = data else {
            return Err(save::Error::Type);
        };
        self.state
            .pouches
            .parse(values, self.factory, self.definitions)
    }
    fn parse_poh_storage(&mut self, data: &Json) -> Result<()> {
        self.state.poh.read_json(data)
    }
}

// The parser's skill and settings services are not part of these systems;
// they pass through unchanged.
impl<R: Remaining> skills::Context for Native<'_, R> {
    fn kind(&self) -> EntityKind {
        self.rest.kind()
    }
    fn version(&self) -> i32 {
        self.rest.version()
    }
    fn tick(&mut self) -> i32 {
        self.rest.tick()
    }
    fn world_tick(&mut self) -> i32 {
        self.rest.world_tick()
    }
    fn combat_level(&self) -> i32 {
        self.rest.combat_level()
    }
    fn summoning_combat_level(&self) -> i32 {
        self.rest.summoning_combat_level()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        self.rest.npc_combat_level()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        self.rest.damage_spong_active()
    }
    fn assisted_level(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.rest.assisted_level(skill)
    }
    fn translate_experience(
        &mut self,
        state: &mut Skills,
        skill: i32,
        amount: f64,
        multiplier: f64,
    ) -> skills::Result<bool> {
        self.rest
            .translate_experience(state, skill, amount, multiplier)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        self.rest.brawling_present()
    }
    fn brawling_for_skill(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.rest.brawling_for_skill(skill)
    }
    fn equipment_contains(&mut self, id: i32) -> skills::Result<bool> {
        self.rest.equipment_contains(id)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        self.rest.brawling_bonus()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        self.rest.tutorial_complete()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        self.rest.artificial()
    }
    fn username(&mut self) -> skills::Result<String> {
        self.rest.username()
    }
    fn permadeath_present(&self) -> bool {
        self.rest.permadeath_present()
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.rest.effect(state, effect)
    }
}
impl<R: Remaining> settings::Context for Native<'_, R> {
    fn varp(&mut self, state: &Settings, id: i32, value: i32) -> settings::Result<()> {
        self.rest.varp(state, id, value)
    }
    fn session(&mut self, state: &Settings) -> settings::Result<Option<usize>> {
        self.rest.session(state)
    }
    fn send(
        &mut self,
        state: &Settings,
        session: Option<usize>,
        frame: rs09_protocol::game::Frame,
    ) -> settings::Result<()> {
        self.rest.send(state, session, frame)
    }
    fn dispatcher(&mut self, state: &Settings) -> settings::Result<Option<usize>> {
        self.rest.dispatcher(state)
    }
    fn run_energy(&mut self, state: &Settings, dispatcher: Option<usize>) -> settings::Result<()> {
        self.rest.run_energy(state, dispatcher)
    }
    fn message(
        &mut self,
        state: &Settings,
        dispatcher: Option<usize>,
        text: &str,
    ) -> settings::Result<()> {
        self.rest.message(state, dispatcher, text)
    }
    fn string(
        &mut self,
        state: &Settings,
        dispatcher: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        self.rest.string(state, dispatcher, text, interface, child)
    }
    fn properties(&mut self, state: &Settings) -> settings::Result<Option<usize>> {
        self.rest.properties(state)
    }
    fn retaliating(
        &mut self,
        state: &Settings,
        properties: Option<usize>,
    ) -> settings::Result<bool> {
        settings::Context::retaliating(self.rest, state, properties)
    }
    fn set_retaliating(
        &mut self,
        state: &Settings,
        properties: Option<usize>,
        value: bool,
    ) -> settings::Result<()> {
        self.rest.set_retaliating(state, properties, value)
    }
    fn ironman(&mut self, state: &Settings) -> settings::Result<bool> {
        self.rest.ironman(state)
    }
    fn container(
        &mut self,
        state: &Settings,
        kind: settings::ContainerKind,
    ) -> settings::Result<usize> {
        self.rest.container(state, kind)
    }
    fn item(
        &mut self,
        state: &Settings,
        container: usize,
        slot: i32,
    ) -> settings::Result<Option<usize>> {
        self.rest.item(state, container, slot)
    }
    fn weight(&mut self, state: &Settings, item: usize) -> settings::Result<f64> {
        self.rest.weight(state, item)
    }
}
