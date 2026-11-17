//! The rest of a player's own state, joined to the canonical player: the
//! Player, Entity and PlayerDetails members that have a native library
//! (details and contact metadata, skills, saved data, timers, Properties,
//! the spell book, the persisted systems behind `save::Host`, the save
//! version and Player's own scalar fields), and the character save file
//! written from, and loaded into, the canonical player.
//!
//! None of these values stores a canonical node handle: the original's
//! `entity`/`player` back-references of Properties, Skills, TimerManager and
//! the persisted managers are implicit in their being fields of this player,
//! so they add no ownership edge and are not traced. Pouch and beast-of-burden
//! containers have no listeners.
//!
//! Operations that run services take the state out of the player for their
//! duration ([`Player::with_profile`], save, load). No player lock is held
//! across a service: a service that reaches back into the same player's
//! profile meanwhile gets [`Error::InUse`] instead of a deadlock or a stand-in.
use super::Player;
use crate::{
    auth::Clock,
    definitions::item::Registry,
    inventory::{
        self,
        container::{Container, Context as ContainerContext, Event, Factory},
        container_listener::Handle as Listener,
        item::Handle as ItemHandle,
    },
    player::{
        attributes::{AttributeValue, GameAttributes},
        core_data::{self, CoreData, Varps},
        details::PlayerDetails,
        persisted::host::{Native, Persisted, Remaining, World},
        properties::Properties,
        save::{self, Env, Level, Loaded, Parser},
        saved_data::SavedData,
        settings::{self, Settings},
        skills::{self, Effect, EntityKind, Skills},
    },
    timers,
    world::{doors::Location, player_walk::State},
};
use rs09_game_core::java_json::Json;
use std::collections::BTreeMap;

/// ServerConstants.CURRENT_SAVEFILE_VERSION.
pub const CURRENT_SAVEFILE_VERSION: i32 = 6;
/// SpellBook.MODERN's interface, SpellBookManager's initial spell book.
pub const MODERN_SPELL_BOOK: i32 = 192;
/// `new byte[255]`.
pub const OP_COUNTS: usize = 255;

#[derive(Debug)]
pub enum Error {
    /// No profile state has been bound to this player.
    Unbound,
    /// The profile is taken out by a running operation on this player.
    InUse,
    /// The player or its fields belong to another canonical world, or a
    /// second profile was bound.
    Binding(String),
    Save(save::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<save::Error> for Error {
    fn from(error: save::Error) -> Self {
        Self::Save(error)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// Player's own scalar fields, with their initial values.
#[derive(Clone, Debug, PartialEq)]
pub struct Flags {
    /// `newPlayer = getSkills().getTotalLevel() < 50`, evaluated once at construction.
    pub new_player: bool,
    pub in_wardrobe: bool,
    pub start_location: Option<[i32; 3]>,
    /// `Boolean isAfkLogout`, null until set.
    pub is_afk_logout: Option<bool>,
    pub playing: bool,
    /// Player's own `invisible`, which hides Entity's field of the same name.
    pub invisible: bool,
    /// Entity's `invisible`.
    pub entity_invisible: bool,
    pub artificial: bool,
    pub custom_state: String,
    pub archery_targets: i32,
    pub archery_total: i32,
    pub op_counts: Vec<i8>,
    pub invalid_packet_count: i32,
}
impl Default for Flags {
    fn default() -> Self {
        Self {
            new_player: false,
            in_wardrobe: false,
            start_location: None,
            is_afk_logout: None,
            playing: false,
            invisible: false,
            entity_invisible: false,
            artificial: false,
            custom_state: String::new(),
            archery_targets: 0,
            archery_total: 0,
            op_counts: vec![0; OP_COUNTS],
            invalid_packet_count: 0,
        }
    }
}

/// The canonical player's profile state. Values only: it is never shared.
pub struct Profile {
    /// Node.name, the username the player was constructed with.
    pub name: String,
    /// Player.version.
    pub version: i32,
    /// Player.details: account details, rights, UID info, the session address,
    /// `saveParsed` and the communication (contact) metadata.
    pub details: PlayerDetails,
    /// Entity.skills.
    pub skills: Skills,
    /// Player.savedData.
    pub saved: SavedData,
    /// Entity.timers.
    pub timers: timers::Manager,
    /// Entity.properties, without the autocast spell (see `persisted.spells`).
    pub properties: Properties,
    /// SpellBookManager.spellBook, an interface id.
    pub spell_book: i32,
    /// Quests, music, emotes, diaries, the autocast spell, pouches, POH
    /// storage, house and familiar values.
    pub persisted: Persisted,
    /// FamiliarManager.summoningCombatLevel.
    pub summoning_combat_level: i32,
    pub flags: Flags,
}

/// The world inputs a new player's construction reads.
pub struct Fresh<'a> {
    /// Registered quests, songs and spells.
    pub world: &'a World,
    /// GameWorld.getTicks(), read by Skills' `lastUpdate` initialiser.
    pub ticks: i32,
}

/// The services `new Skills(entity)` reaches: it reads the tick and sets the
/// entity's combat level, both answered here.
struct Construction<'a> {
    properties: &'a mut Properties,
    ticks: i32,
}
impl skills::Context for Construction<'_> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        CURRENT_SAVEFILE_VERSION
    }
    fn tick(&mut self) -> i32 {
        self.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.ticks
    }
    fn combat_level(&self) -> i32 {
        self.properties.combat_level
    }
    fn summoning_combat_level(&self) -> i32 {
        0
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn username(&mut self) -> skills::Result<String> {
        Err(skills::Error::Binding)
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, _: &mut Skills, effect: Effect) -> skills::Result<()> {
        match effect {
            Effect::Combat(level) => {
                self.properties.combat_level = level;
                Ok(())
            }
            _ => Err(skills::Error::Binding),
        }
    }
}

impl Profile {
    /// What `new Player(details)` sets for the members held here, in the
    /// original's initialisation order: Entity's Properties, then Skills
    /// (which sets the combat level to 3), then Player's fields
    /// (`newPlayer`, the pouches, the spell book, quests for every quest
    /// registered now, the other persisted managers, `version`). `details` is
    /// constructed first, by the caller, as the original's caller does.
    pub fn new(details: PlayerDetails, fresh: Fresh<'_>, factory: &Factory) -> Result<Self> {
        let name = crate::auth::storage::lock(&details.account)
            .username
            .clone();
        let mut properties = Properties::default();
        let skills = Skills::new(&mut Construction {
            properties: &mut properties,
            ticks: fresh.ticks,
        })
        .map_err(|e| Error::Save(save::Error::Skills(e)))?;
        let flags = Flags {
            new_player: skills.total_level() < 50,
            ..Flags::default()
        };
        let persisted = Persisted::new(fresh.world, factory)?;
        Ok(Self {
            name,
            version: CURRENT_SAVEFILE_VERSION,
            details,
            skills,
            saved: SavedData::default(),
            timers: timers::Manager::default(),
            properties,
            spell_book: MODERN_SPELL_BOOK,
            persisted,
            summoning_combat_level: 0,
            flags,
        })
    }
}

pub(super) enum Slot {
    Unbound,
    Bound(Box<Profile>),
    InUse,
}

/// Puts the profile back however the operation ended.
struct Taken<'a> {
    player: &'a Player,
    profile: Option<Box<Profile>>,
}
impl Drop for Taken<'_> {
    fn drop(&mut self) {
        if let Some(profile) = self.profile.take() {
            self.player.0.write().profile = Slot::Bound(profile);
        }
    }
}

fn snapshot(a: &GameAttributes) -> GameAttributes {
    GameAttributes {
        values: a.values.clone(),
        null_value: a.null_value.clone(),
        saved: a.saved.clone(),
        expirations: a.expirations.clone(),
        null_expiration: a.null_expiration,
    }
}

impl Player {
    /// Binds the profile state once. A second binding fails, as does binding
    /// while an operation has it out.
    pub fn bind_profile(&self, profile: Profile) -> Result<()> {
        let mut fields = self.0.write();
        match fields.profile {
            Slot::Unbound => {
                fields.profile = Slot::Bound(Box::new(profile));
                Ok(())
            }
            Slot::Bound(_) => Err(Error::Binding(
                "the canonical player already has its profile".into(),
            )),
            Slot::InUse => Err(Error::InUse),
        }
    }
    pub fn profile_bound(&self) -> bool {
        !matches!(self.0.read().profile, Slot::Unbound)
    }
    fn take_profile(&self) -> Result<Taken<'_>> {
        let mut fields = self.0.write();
        match std::mem::replace(&mut fields.profile, Slot::InUse) {
            Slot::Bound(profile) => Ok(Taken {
                player: self,
                profile: Some(profile),
            }),
            other => {
                let error = if matches!(other, Slot::Unbound) {
                    Error::Unbound
                } else {
                    Error::InUse
                };
                fields.profile = other;
                Err(error)
            }
        }
    }
    /// Runs `operation` on the profile state. The state is out of the player
    /// meanwhile and no player lock is held.
    pub fn with_profile<T>(&self, operation: impl FnOnce(&mut Profile) -> T) -> Result<T> {
        let mut taken = self.take_profile()?;
        Ok(operation(taken.profile.as_mut().unwrap()))
    }
    fn check_world(&self, state: &State) -> Result<()> {
        let owner = self.owner();
        if !self.in_world(&state.identity())
            || !state.player_fields(&owner).is_ok_and(|p| p.same(self))
        {
            return Err(Error::Binding(
                "the player belongs to another canonical world".into(),
            ));
        }
        Ok(())
    }
}

/// The save file's `core_data` view of the canonical player. The containers
/// are the player's own (Container is a shared handle). The bank tab arrays,
/// the bank selector and the varp and save-flag maps are copied in before the
/// operation and written back after it, also after a failure.
struct Projection {
    varp_keys: Vec<i32>,
    saved: BTreeMap<i32, Option<bool>>,
}
impl Player {
    fn project(&self, location: Option<Location>) -> Result<(CoreData, Projection)> {
        let variables = self.variables();
        let values = variables.values().ok_or(Error::Save(save::Error::Null))?;
        let saved_map = variables.saved().ok_or(Error::Save(save::Error::Null))?;
        let mut varps = Varps::default();
        let keys = values.keys();
        for &key in &keys {
            if let Some(value) = values.get(key) {
                varps.insert(key, value);
            }
        }
        let saved: BTreeMap<i32, Option<bool>> = saved_map.snapshot().into_iter().collect();
        let tabs = |bank: &crate::inventory::bank::Bank| -> Result<[i32; core_data::BANK_TABS]> {
            let slots = bank.tab_start_slots();
            let slots = slots.read();
            <[i32; core_data::BANK_TABS]>::try_from(slots.as_slice())
                .map_err(|_| Error::Save(save::Error::Index))
        };
        let primary = self.bank_primary();
        let secondary = self.bank_secondary();
        let core = CoreData {
            inventory: self.inventory(),
            equipment: self.equipment(),
            primary: core_data::Bank {
                container: primary.container(),
                tabs: tabs(&primary)?,
            },
            secondary: core_data::Bank {
                container: secondary.container(),
                tabs: tabs(&secondary)?,
            },
            use_secondary: self.use_secondary_bank(),
            location,
            varps,
            save_varps: saved.clone(),
        };
        Ok((
            core,
            Projection {
                varp_keys: keys,
                saved,
            },
        ))
    }
    fn publish(&self, core: CoreData, projection: Projection) -> Result<Option<Location>> {
        for (bank, tabs) in [
            (self.bank_primary(), core.primary.tabs),
            (self.bank_secondary(), core.secondary.tabs),
        ] {
            let slots = bank.tab_start_slots();
            let mut slots = slots.write();
            for (slot, value) in slots.iter_mut().zip(tabs) {
                *slot = value;
            }
        }
        self.set_use_secondary_bank(core.use_secondary);
        let variables = self.variables();
        let values = variables.values().ok_or(Error::Save(save::Error::Null))?;
        let saved = variables.saved().ok_or(Error::Save(save::Error::Null))?;
        // Keys already in the map keep their place; new keys are put in the
        // order the projected map iterates them, which keeps the relative put
        // order of keys sharing a bucket.
        for key in core.varps.order() {
            let value = core.varps.get(key).unwrap();
            let known = projection.varp_keys.contains(&key);
            if !known || values.get(key) != Some(value) {
                values.put(key, Some(value));
            }
        }
        for (key, flag) in &core.save_varps {
            if projection.saved.get(key) != Some(flag) {
                saved.put(*key, *flag);
            }
        }
        Ok(core.location)
    }
}

/// The services around one canonical save or load.
pub struct Environment<'a, T, R> {
    pub clock: &'a Clock,
    /// Timer content: resolution, save and parse of each timer type.
    pub timers: &'a mut T,
    /// Every remaining service. `retaliating`, the combat levels the skills
    /// set and read, the settings' Properties and ironman queries are
    /// answered from the canonical player and never asked of it.
    pub services: &'a mut R,
    /// Registered quests, songs, spells and the diaries' static list.
    pub world: &'a mut World,
    /// ServerConstants.PLAYER_ATTRIBUTE_PATH.
    pub attribute_path: &'a str,
}

/// Answers from the canonical player what it owns; forwards the rest.
struct Answers<'a, R> {
    properties: &'a mut Properties,
    summoning: &'a mut i32,
    ironman: crate::player::ironman::Manager,
    rest: &'a mut R,
}
impl<R: Remaining> Remaining for Answers<'_, R> {
    fn log(&mut self, level: Level, message: &str) {
        self.rest.log(level, message);
    }
    /// Properties.isRetaliating.
    fn retaliating(&mut self) -> save::Result<bool> {
        Ok(self.properties.retaliating)
    }
    fn attribute_set(&mut self, key: &str, value: &AttributeValue) -> save::Result<()> {
        self.rest.attribute_set(key, value)
    }
    fn content_hooks(&mut self) -> usize {
        self.rest.content_hooks()
    }
    fn save_content(&mut self, hook: usize, root: &mut Json) -> save::Result<()> {
        self.rest.save_content(hook, root)
    }
    fn parse_content(&mut self, hook: usize, root: &Json) -> save::Result<()> {
        self.rest.parse_content(hook, root)
    }
    fn parse_familiars(&mut self, data: &Json) -> save::Result<()> {
        self.rest.parse_familiars(data)
    }
    fn parse_house(&mut self, data: &Json) -> save::Result<()> {
        self.rest.parse_house(data)
    }
}
impl<R: Remaining> ContainerContext for Answers<'_, R> {
    fn update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
        definitions: &Registry,
    ) -> inventory::Result<()> {
        self.rest.update(listener, container, event, definitions)
    }
    fn refresh(
        &mut self,
        listener: Listener,
        container: &mut Container,
        definitions: &Registry,
    ) -> inventory::Result<()> {
        self.rest.refresh(listener, container, definitions)
    }
    fn ground(
        &mut self,
        container: &mut Container,
        item: Option<ItemHandle>,
        definitions: &Registry,
    ) -> inventory::Result<()> {
        self.rest.ground(container, item, definitions)
    }
}
impl<R: Remaining> skills::Context for Answers<'_, R> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
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
    /// Properties.getCombatLevel.
    fn combat_level(&self) -> i32 {
        self.properties.combat_level
    }
    /// FamiliarManager.getSummoningCombatLevel.
    fn summoning_combat_level(&self) -> i32 {
        *self.summoning
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
    /// Properties.setCombatLevel and FamiliarManager.setSummoningCombatLevel
    /// are field writes on this player; every other effect is a service.
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        match effect {
            Effect::Combat(level) => {
                self.properties.combat_level = level;
                Ok(())
            }
            Effect::SummoningCombat(level) => {
                *self.summoning = level;
                Ok(())
            }
            other => self.rest.effect(state, other),
        }
    }
}
impl<R: Remaining> settings::Context for Answers<'_, R> {
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
    /// `player.getProperties()`: this player's own Properties.
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        Ok(self.properties.retaliating)
    }
    fn set_retaliating(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        value: bool,
    ) -> settings::Result<()> {
        self.properties.retaliating = value;
        Ok(())
    }
    /// `player.getIronmanManager().isIronman()`.
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Ok(self.ironman.is_ironman())
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

/// What a canonical load did besides changing the player.
#[derive(Debug)]
pub struct LoadOutcome {
    /// `parseData`'s own result; `None` when `Parser::parse` found no file.
    pub result: save::Result<Option<Loaded>>,
    /// Entity.location after the load. The location is the actor's: placing
    /// the player there belongs to login, which is not ported.
    pub location: Option<Location>,
}

/// An operation's own result, the text a save built and the location a load left.
type Ran = (
    save::Result<Option<Option<Loaded>>>,
    Option<String>,
    Option<Location>,
);

enum Operation<'p> {
    Save,
    Text(&'p mut Parser, &'p str),
    File(&'p mut Parser, &'p str),
    Hooks(&'p Parser),
}

impl Player {
    /// One save-file operation over the canonical player's state.
    fn run_file_operation<T, R>(
        &self,
        state: &State,
        location: Option<Location>,
        operation: Operation<'_>,
        env: &mut Environment<'_, T, R>,
    ) -> Result<Ran>
    where
        T: timers::Context<CoreData>,
        R: Remaining,
    {
        self.check_world(state)?;
        // Only parsing changes the attributes and the projected core_data.
        let loading = matches!(operation, Operation::Text(..) | Operation::File(..));
        let mut taken = self.take_profile()?;
        let profile = taken.profile.as_mut().unwrap();
        let mut attributes = snapshot(&self.0.read().attributes);
        let (mut core, projection) = self.project(location)?;
        let definitions = self.definitions();
        let factory = Factory {
            null_item: self.core().null_item(),
        };
        let settings = self.settings();
        let appearance = self.appearance();
        let bank_pin = self.bank_pin();
        let ironman = self.ironman();
        let Profile {
            name,
            version,
            details,
            skills,
            saved,
            timers: manager,
            properties,
            spell_book,
            persisted,
            summoning_combat_level,
            ..
        } = &mut **profile;
        let mut answers = Answers {
            properties,
            summoning: summoning_combat_level,
            ironman: ironman.clone(),
            rest: &mut *env.services,
        };
        let mut host = Native {
            state: persisted,
            world: &mut *env.world,
            factory: &factory,
            definitions: &definitions,
            rest: &mut answers,
        };
        let mut save_env = Env {
            definitions: &definitions,
            clock: env.clock,
            timers: &mut *env.timers,
            host: &mut host,
            attribute_path: env.attribute_path,
        };
        let mut view = save::Profile {
            name,
            version,
            save_parsed: &mut details.save_parsed,
            core: &mut core,
            timers: manager,
            attributes: &mut attributes,
            skills,
            settings: &settings,
            saved,
            appearance: &appearance,
            bank_pin: &bank_pin,
            ironman: &ironman,
            spell_book,
        };
        let mut text = None;
        let result = match operation {
            Operation::Save => save::save_text(&mut view, &mut save_env).map(|t| {
                text = Some(t);
                None
            }),
            Operation::Text(parser, input) => parser
                .parse_text(&mut view, &mut save_env, input)
                .map(|loaded| Some(Some(loaded))),
            Operation::File(parser, path) => parser.parse(&mut view, &mut save_env, path).map(Some),
            Operation::Hooks(parser) => parser
                .run_content_hooks(&mut view, &mut save_env)
                .map(|()| None),
        };
        let location = if loading {
            self.0.write().attributes = attributes;
            self.publish(core, projection)?
        } else {
            None
        };
        drop(taken);
        Ok((result, text, location))
    }
    /// `PlayerSaver(player).populate().toJSONString()`: the text the original
    /// writes on Java 11 and later. `location` is Entity.location, which lives
    /// with the canonical actor. Attributes are read from a copy taken when
    /// the save starts.
    pub fn save_text<T, R>(
        &self,
        state: &State,
        location: Option<&Location>,
        env: &mut Environment<'_, T, R>,
    ) -> Result<String>
    where
        T: timers::Context<CoreData>,
        R: Remaining,
    {
        let (result, text, _) =
            self.run_file_operation(state, location.cloned(), Operation::Save, env)?;
        result?;
        Ok(text.unwrap())
    }
    /// `PlayerSaver.save` for Java 11 and later: nothing unless
    /// `details.saveParsed`, then the file `<save_path><name>.json`.
    pub fn save_file<T, R>(
        &self,
        state: &State,
        location: Option<&Location>,
        env: &mut Environment<'_, T, R>,
        save_path: &str,
    ) -> Result<save::SaveOutcome>
    where
        T: timers::Context<CoreData>,
        R: Remaining,
    {
        let (parsed, name) = self.with_profile(|p| (p.details.save_parsed, p.name.clone()))?;
        if !parsed {
            return Ok(save::SaveOutcome::NotParsed);
        }
        let json = self.save_text(state, location, env)?;
        let path = format!("{save_path}{name}.json");
        let written = (|| {
            if !std::path::Path::new(&path).exists() {
                std::fs::create_dir_all(save_path)?;
            }
            std::fs::write(&path, json.as_bytes())
        })();
        Ok(match written {
            Ok(()) => save::SaveOutcome::Written,
            Err(error) => save::SaveOutcome::Failed(error.kind()),
        })
    }
    /// `PlayerSaveParser.parse`'s steps after the reader exists, into this
    /// player. A failed section keeps the earlier sections' changes, as the
    /// original does. `location` is Entity.location before the load.
    pub fn load_text<T, R>(
        &self,
        state: &State,
        location: Option<&Location>,
        parser: &mut Parser,
        text: &str,
        env: &mut Environment<'_, T, R>,
    ) -> Result<LoadOutcome>
    where
        T: timers::Context<CoreData>,
        R: Remaining,
    {
        let (result, _, location) =
            self.run_file_operation(state, location.cloned(), Operation::Text(parser, text), env)?;
        Ok(LoadOutcome {
            result: result.map(Option::flatten),
            location,
        })
    }
    /// `PlayerSaveParser.parse`: reads `<save_path><name>.json`.
    pub fn load_file<T, R>(
        &self,
        state: &State,
        location: Option<&Location>,
        parser: &mut Parser,
        save_path: &str,
        env: &mut Environment<'_, T, R>,
    ) -> Result<LoadOutcome>
    where
        T: timers::Context<CoreData>,
        R: Remaining,
    {
        let (result, _, location) = self.run_file_operation(
            state,
            location.cloned(),
            Operation::File(parser, save_path),
            env,
        )?;
        Ok(LoadOutcome {
            result: result.map(Option::flatten),
            location,
        })
    }
    /// `PlayerSaveParser.runContentHooks`, which sets `details.saveParsed`.
    pub fn run_content_hooks<T, R>(
        &self,
        state: &State,
        location: Option<&Location>,
        parser: &Parser,
        env: &mut Environment<'_, T, R>,
    ) -> Result<save::Result<()>>
    where
        T: timers::Context<CoreData>,
        R: Remaining,
    {
        let (result, _, _) =
            self.run_file_operation(state, location.cloned(), Operation::Hooks(parser), env)?;
        Ok(result.map(|_| ()))
    }
}
