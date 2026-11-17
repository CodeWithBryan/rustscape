//! The original character save file: PlayerSaver.kt and PlayerSaveParser.kt
//! joined to the native profile libraries. The other systems a save reaches
//! (quests, house, familiars, music, diaries, emotes, autocast spells, graves,
//! pouches, POH storage, content hooks) are reached only through [`Host`];
//! nothing here models them. `player::persisted` holds the persisted state of
//! most of them and its `host::Native` answers those `Host` sections; the rest
//! stay required services. The playable alpha still uses its own snapshot
//! format, and no world login or logout path calls this module yet.
mod order;
pub mod parser;
use super::{
    appearance::{self, Appearance},
    attributes::{self, GameAttributes},
    bank_pin, core_data,
    core_data::CoreData,
    ironman,
    saved_data::{self, SavedData},
    settings::{self, Settings},
    skills::{self, Skills},
};
use crate::{
    auth::Clock,
    definitions::item::Registry,
    inventory::{self, container::Container},
    timers,
};
pub use parser::{Loaded, Parser};
use rs09_game_core::{java_double, java_json, java_json::Json};

/// Sizes of the pinned enums the parser indexes with saved ordinals.
pub const GRAVE_TYPES: i32 = 13;
pub const EMOTES: i32 = 45;
pub const SPELL_BOOKS: i32 = 3;
/// SpellBook interface ids, in ordinal order: modern, ancient, lunar.
pub const SPELL_BOOK_INTERFACES: [i32; 3] = [192, 193, 430];

#[derive(Debug)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    /// JSONParser rejected the text, or a native text limit applied.
    Json(java_json::Error),
    Io(std::io::ErrorKind),
    /// A [`Host`] service failed.
    Host(String),
    Core(core_data::Error),
    Attributes(attributes::Error),
    Skills(skills::Error),
    Settings(settings::Error),
    Saved(saved_data::Error),
    Appearance(appearance::Error),
    BankPin(bank_pin::Error),
    /// A container of the persisted systems (rune pouches, diary rewards).
    Inventory(inventory::Error),
    /// Java's NegativeArraySizeException.
    NegativeSize,
}
pub type Result<T> = std::result::Result<T, Error>;
/// The original exception family, or the native-only rejection, behind an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Null,
    Type,
    Number,
    Index,
    ConcurrentModification,
    Binding,
    Parse,
    Argument,
    Io,
    /// Native text limits with no original counterpart.
    NativeLimit,
    NegativeArraySize,
}
impl Error {
    pub fn category(&self) -> Category {
        use Category as C;
        let inventory = |error: &inventory::Error| match error {
            inventory::Error::Null => C::Null,
            inventory::Error::Type => C::Type,
            inventory::Error::Number => C::Number,
            inventory::Error::Bounds | inventory::Error::ListBounds => C::Index,
            inventory::Error::ConcurrentModification => C::ConcurrentModification,
            inventory::Error::Binding(_) => C::Binding,
            inventory::Error::CyclicNote => C::NativeLimit,
        };
        match self {
            Self::Null => C::Null,
            Self::Type => C::Type,
            Self::Number => C::Number,
            Self::Index => C::Index,
            Self::Json(error) => match error.kind {
                java_json::ErrorKind::Character | java_json::ErrorKind::Token => C::Parse,
                java_json::ErrorKind::Number => C::Number,
                java_json::ErrorKind::Unicode | java_json::ErrorKind::Limit => C::NativeLimit,
            },
            Self::Io(_) => C::Io,
            Self::Host(_) => C::Binding,
            Self::Core(error) => match error {
                core_data::Error::Null => C::Null,
                core_data::Error::Type => C::Type,
                core_data::Error::Number => C::Number,
                core_data::Error::Index => C::Index,
                core_data::Error::Inventory(error) => inventory(error),
                core_data::Error::Timer(error) => match error {
                    timers::Error::Null => C::Null,
                    timers::Error::Type => C::Type,
                    timers::Error::Number => C::Number,
                    timers::Error::Index => C::Index,
                    timers::Error::ConcurrentModification => C::ConcurrentModification,
                    timers::Error::Binding(_) => C::Binding,
                },
            },
            Self::Attributes(error) => match error {
                attributes::Error::Null => C::Null,
                attributes::Error::Type => C::Type,
                attributes::Error::Number => C::Number,
                attributes::Error::Index => C::Index,
                attributes::Error::Base64 => C::Argument,
                attributes::Error::Xml | attributes::Error::Io => C::Io,
            },
            Self::Skills(error) => match error {
                skills::Error::Null => C::Null,
                skills::Error::Type => C::Type,
                skills::Error::Number => C::Number,
                skills::Error::Index => C::Index,
                skills::Error::Binding => C::Binding,
            },
            Self::Settings(error) => match error {
                settings::Error::Null => C::Null,
                settings::Error::Type => C::Type,
                settings::Error::Number => C::Number,
                settings::Error::Index => C::Index,
                settings::Error::Binding => C::Binding,
            },
            Self::Saved(error) => match error {
                saved_data::Error::Null => C::Null,
                saved_data::Error::Type => C::Type,
                saved_data::Error::Number => C::Number,
                saved_data::Error::Index => C::Index,
            },
            Self::Appearance(error) => match error {
                appearance::Error::Null => C::Null,
                appearance::Error::Type => C::Type,
                appearance::Error::Number => C::Number,
                appearance::Error::Index => C::Index,
                appearance::Error::Inventory(error) => inventory(error),
                appearance::Error::Binding(_) => C::Binding,
            },
            Self::BankPin(error) => match error {
                bank_pin::Error::Null => C::Null,
                bank_pin::Error::Type => C::Type,
                bank_pin::Error::Number => C::Number,
                bank_pin::Error::Index => C::Index,
                bank_pin::Error::Binding(_) => C::Binding,
            },
            Self::Inventory(error) => inventory(error),
            Self::NegativeSize => C::NegativeArraySize,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
macro_rules! from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Error {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from!(java_json::Error, Json);
from!(core_data::Error, Core);
from!(attributes::Error, Attributes);
from!(skills::Error, Skills);
from!(settings::Error, Settings);
from!(saved_data::Error, Saved);
from!(appearance::Error, Appearance);
from!(bank_pin::Error, BankPin);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Warn,
    /// Used only by the Java-below-11 branch of the saver, which is not ported.
    Err,
}
/// QuestRepository.points and its quest map entries, in that map's own iteration order.
#[derive(Clone, Debug, Default)]
pub struct Quests {
    pub points: i32,
    pub stages: Vec<(i32, i32)>,
}
#[derive(Clone, Copy, Debug)]
pub struct Autocast {
    pub book: i32,
    pub spell_id: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct PetDetail {
    pub hunger: f64,
    pub growth: f64,
}
pub enum ActiveFamiliar {
    None,
    Pet {
        item_id: i32,
    },
    Familiar {
        original_id: i32,
        ticks: i32,
        special_points: i32,
        /// Present for a beast of burden, including one carrying nothing.
        burden: Option<Container>,
        lifepoints: i32,
    },
}
/// FamiliarManager.petDetails entries in that map's iteration order, and the current familiar.
pub struct Familiars {
    pub pets: Vec<(i32, Vec<PetDetail>)>,
    pub active: ActiveFamiliar,
}
#[derive(Clone, Debug)]
pub struct Servant {
    pub kind: i32,
    pub uses: i32,
    pub item: Option<(i32, i32)>,
    pub greet: bool,
}
#[derive(Clone, Debug)]
pub struct Room {
    pub properties: i32,
    pub rotation: i32,
    /// Every hotspot's decoration index, built or not.
    pub hotspots: Vec<i32>,
}
/// HouseManager values by ordinal. `rooms` keeps the original plane, x, y nesting.
#[derive(Clone, Debug, Default)]
pub struct House {
    pub location: i32,
    pub style: i32,
    pub crest: i32,
    pub servant: Option<Servant>,
    pub rooms: Vec<Vec<Vec<Option<Room>>>>,
}
/// One AchievementDiary. `name` is the DiaryType constant name, not its display name.
#[derive(Clone, Debug)]
pub struct Diary {
    pub name: String,
    pub started: Vec<bool>,
    pub completed: Vec<Vec<bool>>,
    pub rewarded: Vec<bool>,
}
#[derive(Clone, Debug, Default)]
pub struct Emotes {
    pub save_required: bool,
    pub ordinals: Vec<i32>,
}
/// Every system the two original files reach that has no native port. Each
/// method is a required service: an implementation must not report success
/// for work it did not do. Saving reads plain values; parsing hands over the
/// exact JSON the original method would have received.
pub trait Host {
    fn log(&mut self, level: Level, message: &str);
    /// Properties.isRetaliating.
    fn retaliating(&mut self) -> Result<bool>;
    fn quests(&mut self) -> Result<Quests>;
    /// Properties.autocastSpell: its book ordinal and spell id.
    fn autocast(&mut self) -> Result<Option<Autocast>>;
    /// MusicPlayer.unlocked values' ids in that map's iteration order.
    fn unlocked_music(&mut self) -> Result<Vec<i32>>;
    fn familiars(&mut self) -> Result<Familiars>;
    fn house(&mut self) -> Result<House>;
    fn diaries(&mut self) -> Result<Vec<Diary>>;
    fn emotes(&mut self) -> Result<Emotes>;
    /// PouchManager.save(root): the manager writes its own keys.
    fn save_pouches(&mut self, root: &mut Json) -> Result<()>;
    /// StorageState.toJson().
    fn poh_storage(&mut self) -> Result<Json>;
    /// The number of registered PersistPlayer content hooks.
    fn content_hooks(&mut self) -> usize;
    fn save_content(&mut self, hook: usize, root: &mut Json) -> Result<()>;
    fn parse_content(&mut self, hook: usize, root: &Json) -> Result<()>;
    fn parse_quests(&mut self, data: &Json) -> Result<()>;
    /// GraveController.updateGraveType with GraveType.values()[ordinal]. The
    /// original keeps the type as a saved attribute of the player.
    fn update_grave_type(&mut self, attributes: &mut GameAttributes, ordinal: i32) -> Result<()>;
    /// `SpellBook.values()[book].getSpell(spell) as CombatSpell`, then the
    /// assignment. A missing spell is [`Error::Null`], another spell kind [`Error::Type`].
    fn set_autocast(&mut self, book: i32, spell: i32) -> Result<()>;
    /// `MusicEntry.forId(id)` then `unlocked.put(entry.index, entry)`. An
    /// unknown id is [`Error::Null`].
    fn unlock_music(&mut self, id: i32) -> Result<()>;
    fn parse_familiars(&mut self, data: &Json) -> Result<()>;
    fn parse_house(&mut self, data: &Json) -> Result<()>;
    /// `emoteManager.emotes.contains(Emotes.values()[ordinal])`.
    fn emote_unlocked(&mut self, ordinal: i32) -> Result<bool>;
    fn unlock_emote(&mut self, ordinal: i32) -> Result<()>;
    fn parse_diaries(&mut self, data: &Json) -> Result<()>;
    /// AchievementDiaryManager.resetRewards, which removes reward items from
    /// the player's containers.
    fn reset_diary_rewards(&mut self, core: &mut CoreData, definitions: &Registry) -> Result<()>;
    fn parse_pouches(&mut self, data: &Json) -> Result<()>;
    fn parse_poh_storage(&mut self, data: &Json) -> Result<()>;
}
/// The profile fields the two files read and write, borrowed from wherever
/// the character keeps them.
pub struct Profile<'a> {
    /// Player.name: the save file stem and the legacy attribute file stem.
    pub name: &'a str,
    pub version: &'a mut i32,
    /// PlayerDetails.saveParsed.
    pub save_parsed: &'a mut bool,
    pub core: &'a mut CoreData,
    pub timers: &'a mut timers::Manager,
    pub attributes: &'a mut GameAttributes,
    pub skills: &'a mut Skills,
    pub settings: &'a Settings,
    pub saved: &'a mut SavedData,
    pub appearance: &'a Appearance,
    pub bank_pin: &'a bank_pin::Manager,
    pub ironman: &'a ironman::Manager,
    /// SpellBookManager.spellBook, an interface id.
    pub spell_book: &'a mut i32,
}
/// Services and configuration around one save or load.
pub struct Env<'a, T, H> {
    pub definitions: &'a Registry,
    /// System.currentTimeMillis.
    pub clock: &'a Clock,
    pub timers: &'a mut T,
    pub host: &'a mut H,
    /// ServerConstants.PLAYER_ATTRIBUTE_PATH, for the legacy attribute file.
    pub attribute_path: &'a str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveOutcome {
    /// PlayerDetails.saveParsed was false; nothing was built or written.
    NotParsed,
    Written,
    /// The original catches the IOException and returns normally.
    Failed(std::io::ErrorKind),
}

fn put(root: &mut Json, key: &str, value: Json) -> Result<()> {
    root.put(key, value).map_err(|_| Error::Type)?;
    Ok(())
}
fn text(value: impl ToString) -> Json {
    Json::Text(value.to_string())
}

pub fn save_version(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    put(root, "version", Json::Int(i64::from(*profile.version)))
}
pub fn save_core_data<T: timers::Context<CoreData>, H>(
    profile: &mut Profile<'_>,
    root: &mut Json,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    Ok(profile.core.save(root, profile.timers, env.timers)?)
}
pub fn save_skills(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    let mut map = serde_json::Map::new();
    profile.skills.save_profile(&mut map);
    put(
        root,
        "skills",
        order::rows(&map["skills"], order::SKILL_ROW),
    )?;
    put(root, "totalEXP", order::plain(&map["totalEXP"]))?;
    put(root, "exp_multiplier", order::plain(&map["exp_multiplier"]))?;
    if let Some(milestone) = map.get("milestone") {
        put(
            root,
            "milestone",
            order::object(milestone, order::MILESTONE, &|_, v| order::plain(v)),
        )?;
    }
    Ok(())
}
pub fn save_settings<T, H: Host>(
    profile: &Profile<'_>,
    root: &mut Json,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let f = profile.settings.fields.read().clone();
    let mut settings = Json::object();
    put(&mut settings, "brightness", text(f.brightness))?;
    put(&mut settings, "musicVolume", text(f.music_volume))?;
    put(
        &mut settings,
        "soundEffectVolume",
        text(f.sound_effect_volume),
    )?;
    put(&mut settings, "areaSoundVolume", text(f.area_sound_volume))?;
    put(
        &mut settings,
        "publicChatSetting",
        text(f.public_chat_setting),
    )?;
    put(
        &mut settings,
        "privateChatSetting",
        text(f.private_chat_setting),
    )?;
    put(&mut settings, "clanChatSetting", text(f.clan_chat_setting))?;
    put(&mut settings, "tradeSetting", text(f.trade_setting))?;
    put(&mut settings, "assistSetting", text(f.assist_setting))?;
    put(
        &mut settings,
        "runEnergy",
        Json::Text(java_double::format(f.run_energy)),
    )?;
    put(&mut settings, "specialEnergy", text(f.special_energy))?;
    put(&mut settings, "attackStyle", text(f.attack_style_index))?;
    put(
        &mut settings,
        "singleMouse",
        Json::Bool(f.single_mouse_button),
    )?;
    put(
        &mut settings,
        "disableChatEffects",
        Json::Bool(f.disable_chat_effects),
    )?;
    put(
        &mut settings,
        "splitPrivate",
        Json::Bool(f.split_private_chat),
    )?;
    // Settings.isAcceptAid reports false for every ironman, whatever the stored flag is.
    put(
        &mut settings,
        "acceptAid",
        Json::Bool(!profile.ironman.is_ironman() && f.accept_aid),
    )?;
    put(&mut settings, "runToggled", Json::Bool(f.run_toggled))?;
    put(
        &mut settings,
        "retaliation",
        Json::Bool(env.host.retaliating()?),
    )?;
    put(root, "settings", settings)
}
pub fn save_quests<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let view = env.host.quests()?;
    let mut quests = Json::object();
    put(&mut quests, "points", text(view.points))?;
    let mut stages = Vec::with_capacity(view.stages.len());
    for (id, stage) in view.stages {
        let mut quest = Json::object();
        put(&mut quest, "questId", text(id))?;
        put(&mut quest, "questStage", text(stage))?;
        stages.push(quest);
    }
    put(&mut quests, "questStages", Json::Array(stages))?;
    put(root, "quests", quests)
}
pub fn save_appearance(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    let mut appearance = Json::object();
    let gender = profile.appearance.gender().ok_or(Error::Null)?;
    put(&mut appearance, "gender", text(gender.byte()))?;
    let parts = profile.appearance.parts();
    let mut cache = Vec::with_capacity(7);
    for index in 0..7 {
        let part = parts.read()[index].clone().ok_or(Error::Null)?;
        let (look, color) = {
            let part = part.read();
            (part.look, part.color)
        };
        let mut body = Json::object();
        put(&mut body, "look", text(look))?;
        put(&mut body, "color", text(color))?;
        cache.push(body);
    }
    put(&mut appearance, "appearance_cache", Json::Array(cache))?;
    put(root, "appearance", appearance)
}
pub fn save_spellbook(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    put(root, "spellbook", text(*profile.spell_book))
}
pub fn save_activity_data(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    let data = profile.saved.activity.save()?;
    put(
        root,
        "activityData",
        order::object(&data, order::ACTIVITY, &|_, v| order::plain(v)),
    )
}
pub fn save_quest_data(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    let data = profile.saved.quest.save()?;
    put(
        root,
        "questData",
        order::object(&data, order::QUEST, &|key, value| {
            if key == "desertTreasureNode" {
                order::rows(value, order::ITEM)
            } else {
                order::plain(value)
            }
        }),
    )
}
pub fn save_global_data(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    let data = profile.saved.global.save()?;
    put(
        root,
        "globalData",
        order::object(&data, order::GLOBAL, &|_, v| order::plain(v)),
    )
}
/// Each section is published before the next one is built.
pub fn save_saved_data(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    save_activity_data(profile, root)?;
    save_quest_data(profile, root)?;
    save_global_data(profile, root)
}
pub fn save_autocast<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let Some(autocast) = env.host.autocast()? else {
        return Ok(());
    };
    let mut spell = Json::object();
    put(&mut spell, "book", text(autocast.book))?;
    put(&mut spell, "spellId", text(autocast.spell_id))?;
    put(root, "autocastSpell", spell)
}
/// The pinned method has an empty body.
pub fn save_player_monitor(_root: &mut Json) {}
pub fn save_music_player<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let unlocked = env.host.unlocked_music()?;
    put(
        root,
        "unlockedMusic",
        Json::Array(unlocked.into_iter().map(text).collect()),
    )
}
pub fn save_familiar_manager<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let view = env.host.familiars()?;
    let mut manager = Json::object();
    let mut details = Json::object();
    for (pet, values) in &view.pets {
        let mut data = Vec::with_capacity(values.len());
        for value in values {
            let mut pet = Json::object();
            put(
                &mut pet,
                "hunger",
                Json::Text(java_double::format(value.hunger)),
            )?;
            put(
                &mut pet,
                "growth",
                Json::Text(java_double::format(value.growth)),
            )?;
            data.push(pet);
        }
        put(&mut details, &pet.to_string(), Json::Array(data))?;
    }
    put(&mut manager, "petDetails", details)?;
    match &view.active {
        ActiveFamiliar::None => {}
        ActiveFamiliar::Pet { item_id } => put(&mut manager, "currentPet", text(item_id))?,
        ActiveFamiliar::Familiar {
            original_id,
            ticks,
            special_points,
            burden,
            lifepoints,
        } => {
            let mut familiar = Json::object();
            put(&mut familiar, "originalId", text(original_id))?;
            put(&mut familiar, "ticks", text(ticks))?;
            put(&mut familiar, "specialPoints", text(special_points))?;
            if let Some(container) = burden.as_ref().filter(|container| !container.empty()) {
                put(&mut familiar, "inventory", save_container(container))?;
            }
            // The one numeric value the saver does not stringify.
            put(
                &mut familiar,
                "lifepoints",
                Json::Int(i64::from(*lifepoints)),
            )?;
            put(&mut manager, "familiar", familiar)?;
        }
    }
    put(root, "familiarManager", manager)
}
pub fn save_bank_pin_data<T, H>(
    profile: &Profile<'_>,
    root: &mut Json,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let (pin, long_recovery, status, pending_delay, try_delay) = {
        let fields = profile.bank_pin.fields();
        let f = fields.read();
        (
            f.pin.clone(),
            f.long_recovery,
            f.status,
            f.pending_delay,
            f.try_delay,
        )
    };
    let mut manager = Json::object();
    if let Some(pin) = pin {
        put(
            &mut manager,
            "pin",
            Json::Text(String::from_utf16_lossy(&pin)),
        )?;
    }
    put(&mut manager, "longRecovery", Json::Bool(long_recovery))?;
    let status = status.ok_or(Error::Null)? as i32;
    if status != 0 {
        put(&mut manager, "status", text(status))?;
    }
    // Each comparison samples the clock again; -1 short-circuits the first read.
    if pending_delay != -1 && pending_delay > (env.clock)() {
        put(&mut manager, "pendingDelay", text(pending_delay))?;
    }
    if try_delay > (env.clock)() {
        put(&mut manager, "tryDelay", text(try_delay))?;
    }
    put(root, "bankPinManager", manager)
}
pub fn save_house_data<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let view = env.host.house()?;
    let mut house = Json::object();
    put(&mut house, "location", text(view.location))?;
    put(&mut house, "style", text(view.style))?;
    put(&mut house, "crest", text(view.crest))?;
    if let Some(view) = &view.servant {
        let mut servant = Json::object();
        put(&mut servant, "type", text(view.kind))?;
        put(&mut servant, "uses", text(view.uses))?;
        if let Some((id, amount)) = view.item {
            let mut item = Json::object();
            put(&mut item, "id", text(id))?;
            put(&mut item, "amount", text(amount))?;
            put(&mut servant, "item", item)?;
        }
        put(&mut servant, "greet", Json::Bool(view.greet))?;
        put(&mut house, "servant", servant)?;
    }
    let mut rooms = Vec::new();
    for (z, plane) in view.rooms.iter().enumerate() {
        for (x, column) in plane.iter().enumerate() {
            for (y, room) in column.iter().enumerate() {
                let Some(room) = room else {
                    continue;
                };
                let mut r = Json::object();
                put(&mut r, "z", text(z))?;
                put(&mut r, "x", text(x))?;
                put(&mut r, "y", text(y))?;
                put(&mut r, "properties", text(room.properties))?;
                put(&mut r, "rotation", text(room.rotation))?;
                let mut hotspots = Vec::new();
                for (index, decoration) in room.hotspots.iter().enumerate() {
                    if *decoration > -1 {
                        let mut hotspot = Json::object();
                        put(&mut hotspot, "hotspotIndex", text(index))?;
                        put(&mut hotspot, "decorationIndex", text(decoration))?;
                        hotspots.push(hotspot);
                    }
                }
                put(&mut r, "hotspots", Json::Array(hotspots))?;
                rooms.push(r);
            }
        }
    }
    put(&mut house, "rooms", Json::Array(rooms))?;
    put(root, "houseData", house)
}
pub fn save_achievement_data<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let flags = |values: &[bool]| Json::Array(values.iter().map(|v| Json::Bool(*v)).collect());
    let mut data = Vec::new();
    for view in env.host.diaries()? {
        let mut diary = Json::object();
        put(&mut diary, "startedLevels", flags(&view.started))?;
        put(
            &mut diary,
            "completedLevels",
            Json::Array(view.completed.iter().map(|level| flags(level)).collect()),
        )?;
        put(&mut diary, "rewardedLevels", flags(&view.rewarded))?;
        let mut collector = Json::object();
        put(&mut collector, &view.name, diary)?;
        data.push(collector);
    }
    put(root, "achievementDiaries", Json::Array(data))
}
pub fn save_iron_man_data(profile: &Profile<'_>, root: &mut Json) -> Result<()> {
    // A null mode differs from NONE, so the original then fails reading its ordinal.
    let mode = profile.ironman.mode().ok_or(Error::Null)?;
    if mode != ironman::Mode::None {
        put(root, "ironManMode", text(mode as i32))?;
    }
    Ok(())
}
pub fn save_emote_data<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let view = env.host.emotes()?;
    if view.save_required {
        put(
            root,
            "emoteData",
            Json::Array(view.ordinals.into_iter().map(text).collect()),
        )?;
    }
    Ok(())
}
/// The pinned method always writes an empty array.
pub fn save_stat_manager(root: &mut Json) -> Result<()> {
    put(root, "statistics", Json::Array(Vec::new()))
}
pub fn save_attributes<T, H: Host>(
    profile: &Profile<'_>,
    root: &mut Json,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let mut map = serde_json::Map::new();
    let host = &mut *env.host;
    profile
        .attributes
        .save_json_with_log(&mut map, &mut |message| host.log(Level::Warn, message));
    if let Some(attributes) = map.get("attributes") {
        put(
            root,
            "attributes",
            order::rows(attributes, order::ATTRIBUTE),
        )?;
    }
    Ok(())
}
pub fn save_pouches<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    env.host.save_pouches(root)
}
pub fn save_poh_storage_room<T, H: Host>(root: &mut Json, env: &mut Env<'_, T, H>) -> Result<()> {
    let storage = env.host.poh_storage()?;
    put(root, "pohstorage", storage)
}
pub use core_data::save_container;
/// PlayerSaver.populate. A failure discards the object being built.
pub fn populate<T: timers::Context<CoreData>, H: Host>(
    profile: &mut Profile<'_>,
    env: &mut Env<'_, T, H>,
) -> Result<Json> {
    let mut root = Json::object();
    save_version(profile, &mut root)?;
    save_core_data(profile, &mut root, env)?;
    save_skills(profile, &mut root)?;
    save_settings(profile, &mut root, env)?;
    save_quests(&mut root, env)?;
    save_appearance(profile, &mut root)?;
    save_spellbook(profile, &mut root)?;
    save_saved_data(profile, &mut root)?;
    save_autocast(&mut root, env)?;
    save_player_monitor(&mut root);
    save_music_player(&mut root, env)?;
    save_familiar_manager(&mut root, env)?;
    save_bank_pin_data(profile, &mut root, env)?;
    save_house_data(&mut root, env)?;
    save_achievement_data(&mut root, env)?;
    save_iron_man_data(profile, &mut root)?;
    save_emote_data(&mut root, env)?;
    save_stat_manager(&mut root)?;
    save_attributes(profile, &mut root, env)?;
    save_pouches(&mut root, env)?;
    save_poh_storage_room(&mut root, env)?;
    for hook in 0..env.host.content_hooks() {
        env.host.save_content(hook, &mut root)?;
    }
    Ok(root)
}
/// The text PlayerSaver.save writes on Java 11 and later: `populate().toJSONString()`.
pub fn save_text<T: timers::Context<CoreData>, H: Host>(
    profile: &mut Profile<'_>,
    env: &mut Env<'_, T, H>,
) -> Result<String> {
    Ok(populate(profile, env)?.text())
}
/// PlayerSaver.save for Java 11 and later. `save_path` is
/// ServerConstants.PLAYER_SAVE_PATH and is used as a plain string prefix. The
/// bytes are UTF-8 and the file is replaced in place, as FileWriter does.
pub fn save_file<T: timers::Context<CoreData>, H: Host>(
    profile: &mut Profile<'_>,
    env: &mut Env<'_, T, H>,
    save_path: &str,
) -> Result<SaveOutcome> {
    if !*profile.save_parsed {
        return Ok(SaveOutcome::NotParsed);
    }
    let json = save_text(profile, env)?;
    let path = format!("{save_path}{}.json", profile.name);
    let written = (|| {
        if !std::path::Path::new(&path).exists() {
            std::fs::create_dir_all(save_path)?;
        }
        std::fs::write(&path, json.as_bytes())
    })();
    Ok(match written {
        Ok(()) => SaveOutcome::Written,
        Err(error) => SaveOutcome::Failed(error.kind()),
    })
}
