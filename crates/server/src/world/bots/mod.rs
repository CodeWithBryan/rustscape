//! The artificial player ("bot") engine of `core/game/bots`: `AIPlayer` with
//! `ArtificialSession` ([`ai_player`], the file reader of its name lists in
//! [`scanner`]), `AIRepository` ([`repository`]), `Script` ([`script`]),
//! `ScriptAPI` ([`script_api`]), `GeneralBotCreator` with its
//! `BotScriptPulse` ([`creator`]), `AIPBuilder`, `CombatBot`, `PvMBots` and
//! `PvMBotsBuilder` ([`builder`]), `CombatBotAssembler` and
//! `SkillingBotAssembler` ([`assemblers`]), and the `PlayerScripts` table
//! with the four script annotations (below).
//!
//! An artificial player is a `Player` without a client. Its `Player` object
//! is a host [`Host::Node`] (the canonical player once a world binds it);
//! what `AIPlayer` and its engine subclasses add to it is the native
//! [`AiPlayer`]. The class statics (`currentUID`, `botNames`, `botMapping`,
//! `OSRScopyLine`, the `AIRepository` maps, `botPulsesTriggeredThisTick`,
//! `botsSpawned`, the session singleton) are one [`Statics`] per world.
//! `Repository.players` is the native `NodeList` of [`super::repository`],
//! and `Player.clear` is [`super::disconnection::player_clear`].
//!
//! The individual bot scripts, ImmerseWorld's spawning decisions and every
//! other system the engine calls (the player's managers, containers, skills,
//! pathfinding, interaction listeners, the Grand Exchange, consumables,
//! random draws) are required [`Host`] services: none has a default and no
//! lock is held across one. The members at the top of the trait are plain
//! fields and getters of the original; every other method is one original
//! call. See docs/bots.md.
pub mod ai_player;
pub mod assemblers;
pub mod builder;
pub mod canonical;
pub mod creator;
pub mod repository;
pub mod scanner;
pub mod script;
pub mod script_api;
pub mod tables;

pub use super::content_api::Location;
pub use super::content_api::api_files::{Direction, Vector};
pub use super::repository::{JavaHashMap, NodeList};
pub use ai_player::{AiPlayer, ArtificialSession, BotClass};
pub use creator::ScriptPulse;
pub use repository::BotOffer;
pub use script::Script;
pub use script_api::ScriptApi;

use crate::definitions::item::Shared;
use rs09_game_core::java_json::Json;
use std::collections::HashMap;

/// What the original throws, by class. `Binding` is a host service failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException.
    Null,
    /// ClassCastException.
    Cast,
    /// IndexOutOfBoundsException / ArrayIndexOutOfBoundsException.
    Index,
    /// NumberFormatException.
    Number,
    /// IllegalArgumentException.
    Argument,
    /// NoSuchElementException.
    NoSuchElement,
    Binding(String),
}
impl Error {
    /// The exception class's short name as the corpus records it.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Cast => "Cast",
            Self::Index => "Index",
            Self::Number => "Number",
            Self::Argument => "Argument",
            Self::NoSuchElement => "NoSuchElement",
            Self::Binding(_) => "Binding",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// What `Player.clear` (the repository and disconnection port) stopped with,
/// as this engine's callers see it.
impl From<super::repository::Error> for Error {
    fn from(error: super::repository::Error) -> Self {
        use super::repository::Error as R;
        match error {
            R::NullPointer => Self::Null,
            R::IndexOutOfBounds | R::ArrayIndexOutOfBounds => Self::Index,
            R::NoSuchElement => Self::NoSuchElement,
            R::IllegalArgument => Self::Argument,
            R::Io(text) | R::Exception(text) | R::Throwable(text) => Self::Binding(text),
            other => Self::Binding(format!("{other:?}")),
        }
    }
}

/// `Skills` ids the engine names.
pub mod skill {
    pub const ATTACK: i32 = 0;
    pub const DEFENCE: i32 = 1;
    pub const STRENGTH: i32 = 2;
    pub const HITPOINTS: i32 = 3;
    pub const RANGE: i32 = 4;
    pub const PRAYER: i32 = 5;
    pub const MAGIC: i32 = 6;
    pub const SUMMONING: i32 = 23;
    /// `Skills.NUM_SKILLS`.
    pub const NUM_SKILLS: i32 = 24;
}

/// `EquipmentContainer.SLOT_*`.
pub mod slot {
    pub const HAT: i32 = 0;
    pub const CAPE: i32 = 1;
    pub const AMULET: i32 = 2;
    pub const WEAPON: i32 = 3;
    pub const CHEST: i32 = 4;
    pub const SHIELD: i32 = 5;
    pub const LEGS: i32 = 7;
    pub const HANDS: i32 = 9;
    pub const FEET: i32 = 10;
}

/// An `Item` value: `new Item(id, amount)` keeps the id in the upper 16 bits
/// of its id hash, so `getId()` is `id & 0xFFFF`, while its `definition` is
/// `ItemDefinition.forId(id)` of the id as given; the amount is kept as
/// given and only `setAmount` refuses a negative one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// `getId()`.
    pub id: i32,
    pub amount: i32,
    /// The id `getDefinition()` was looked up by. It differs from `id` only
    /// for an item made from an id above 65535.
    pub definition: i32,
}
impl Item {
    /// `new Item(id, amount)`.
    pub fn new(id: i32, amount: i32) -> Self {
        Self {
            id: id & 0xFFFF,
            amount,
            definition: id,
        }
    }
    /// `new Item(id)`.
    pub fn one(id: i32) -> Self {
        Self::new(id, 1)
    }
    /// `setAmount(amount)`.
    pub fn set_amount(&mut self, amount: i32) {
        self.amount = amount.max(0);
    }
}

/// What an `is` / `instanceof` test sees. A `GroundItem` is an `Item`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Player,
    Npc,
    Scenery,
    Item,
    Other,
}

/// `Gender`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
}
impl Gender {
    pub fn name(self) -> &'static str {
        match self {
            Self::Male => "MALE",
            Self::Female => "FEMALE",
        }
    }
}

/// `IntType`, as far as `ScriptAPI` picks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntType {
    Scenery,
    Npc,
    Item,
}
impl IntType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Scenery => "SCENERY",
            Self::Npc => "NPC",
            Self::Item => "ITEM",
        }
    }
}

/// `DestinationFlag`, as far as the engine names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationFlag {
    FollowEntity,
    Object,
}
impl DestinationFlag {
    pub fn name(self) -> &'static str {
        match self {
            Self::FollowEntity => "FOLLOW_ENTITY",
            Self::Object => "OBJECT",
        }
    }
}

/// Which `Pathfinder.find` form: `find(mover, destination)` or
/// `find(mover, destination, near, Pathfinder.SMART)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finder {
    Default,
    Smart { near: bool },
}

/// A `Node` destination: a `Location` is a `Node` of the original.
#[derive(Clone, Debug, PartialEq)]
pub enum Dest<N> {
    Node(N),
    Location(Location),
}

/// One of a player's containers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Inventory,
    Equipment,
    Bank,
}
impl Which {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Equipment => "equipment",
            Self::Bank => "bank",
        }
    }
}

/// A value of `ItemDefinition.getHandlers()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handler {
    /// An `Int` (`equipment_slot`).
    Int(i32),
    /// The `requirements` map, skill to level, in the order the original's
    /// `HashMap<Int, Int>` iterates.
    Requirements(Vec<(i32, i32)>),
    /// Anything else.
    Other,
}

/// What `NodeUsageEvent` is built from.
#[derive(Clone, Debug, PartialEq)]
pub enum Used<N> {
    Item(Option<Item>),
    Node(N),
}

/// What a `Consumable.consume` call receives the consumable as.
pub enum Food<C> {
    /// `Consumables.getConsumableById(id).consumable`.
    Known(C),
    /// `Food(ids, HealingEffect(1))`, built on the spot.
    Plain { ids: Vec<i32>, heal: i32 },
}

/// `PlayerScripts.PlayerScript`: what the `ScriptIdentifier`,
/// `ScriptDescription` and `ScriptName` annotations of a script class say,
/// and the class. `PlayerCompatible` is the precondition of an entry.
#[derive(Clone, Debug)]
pub struct PlayerScript<C> {
    pub identifier: String,
    pub description: Vec<String>,
    pub name: String,
    pub clazz: C,
}

/// The pulses the engine hands to a pulse manager or the world pulser. The
/// host keeps them and runs them with [`Pulse::pulse`]; movement itself
/// (`MovementPulse.update`) is the host's.
pub enum Pulse<H: Host> {
    /// `GeneralBotCreator.BotScriptPulse`.
    Script(ScriptPulse<H>),
    /// `GeneralBotCreator.TransitionPulse`.
    Transition(creator::TransitionPulse<H>),
    /// `AIPlayer.follow`'s anonymous `MovementPulse`.
    Follow { bot: H::Node, entity: H::Node },
    /// `ScriptAPI.walkToIterator`'s anonymous `MovementPulse`.
    Walk { bot: H::Node, destination: Location },
    /// `ScriptAPI.walkArray`'s anonymous `Pulse`.
    WalkArray(script_api::WalkArray<H>),
    /// `teleportToGE`'s and `teleport`'s anonymous `Pulse(4, bot)`.
    Teleport {
        bot: H::Node,
        location: Location,
        to_ge: bool,
    },
    /// `sellOnGE`'s `toCounterPulse`.
    SellOnGe { api: ScriptApi<H>, id: i32 },
    /// `sellAllOnGe`'s `toCounterPulseAll`.
    SellAll { api: ScriptApi<H> },
    /// `sellAllOnGeAdv`'s `toCounterPulseAll`.
    SellAllAdv { api: ScriptApi<H>, desk: H::Node },
    /// `depositAtBank`'s `BankingPulse`.
    Deposit { api: ScriptApi<H>, bank: H::Node },
    /// `bankItem`'s `BankingPulse`.
    BankItem { api: ScriptApi<H>, item: i32 },
    /// `bankAll`'s `BankingPulse`.
    BankAll(script_api::BankAll<H>),
    /// `buyFromGE`'s anonymous `Pulse(5)`.
    Buy(script_api::BuyTask<H>),
}
impl<H: Host> Clone for Pulse<H> {
    fn clone(&self) -> Self {
        match self {
            Self::Script(p) => Self::Script(p.clone()),
            Self::Transition(p) => Self::Transition(p.clone()),
            Self::Follow { bot, entity } => Self::Follow {
                bot: bot.clone(),
                entity: entity.clone(),
            },
            Self::Walk { bot, destination } => Self::Walk {
                bot: bot.clone(),
                destination: *destination,
            },
            Self::WalkArray(p) => Self::WalkArray(p.clone()),
            Self::Teleport {
                bot,
                location,
                to_ge,
            } => Self::Teleport {
                bot: bot.clone(),
                location: *location,
                to_ge: *to_ge,
            },
            Self::SellOnGe { api, id } => Self::SellOnGe {
                api: api.clone(),
                id: *id,
            },
            Self::SellAll { api } => Self::SellAll { api: api.clone() },
            Self::SellAllAdv { api, desk } => Self::SellAllAdv {
                api: api.clone(),
                desk: desk.clone(),
            },
            Self::Deposit { api, bank } => Self::Deposit {
                api: api.clone(),
                bank: bank.clone(),
            },
            Self::BankItem { api, item } => Self::BankItem {
                api: api.clone(),
                item: *item,
            },
            Self::BankAll(p) => Self::BankAll(p.clone()),
            Self::Buy(p) => Self::Buy(p.clone()),
        }
    }
}
impl<H: Host> Pulse<H> {
    /// The pulse's delay, as its constructor sets it.
    pub fn delay(&self) -> i32 {
        match self {
            Self::Transition(p) => p.delay,
            Self::Teleport { .. } => 4,
            Self::BankItem { .. } | Self::BankAll(_) => 20,
            Self::Buy(_) => 5,
            _ => 1,
        }
    }
    /// The nodes a `Pulse(delay, checks...)` constructor was given. A
    /// `MovementPulse` passes its mover and destination; those are
    /// [`Pulse::movement`].
    pub fn checks(&self) -> Vec<H::Node> {
        match self {
            Self::Teleport { bot, .. } => vec![bot.clone()],
            _ => Vec::new(),
        }
    }
    /// A `MovementPulse`'s mover, destination and destination flag.
    #[allow(clippy::type_complexity)]
    pub fn movement(&self) -> Option<(H::Node, Dest<H::Node>, Option<DestinationFlag>)> {
        const COUNTER: (i32, i32, i32) = (3165, 3487, 0);
        let counter = || Dest::Location(Location::new(COUNTER.0, COUNTER.1, COUNTER.2));
        match self {
            Self::Follow { bot, entity } => Some((
                bot.clone(),
                Dest::Node(entity.clone()),
                Some(DestinationFlag::FollowEntity),
            )),
            Self::Walk { bot, destination } => {
                Some((bot.clone(), Dest::Location(*destination), None))
            }
            Self::SellOnGe { api, .. } | Self::SellAll { api } => {
                Some((api.bot.clone(), counter(), None))
            }
            Self::SellAllAdv { api, desk } => Some((
                api.bot.clone(),
                Dest::Node(desk.clone()),
                Some(DestinationFlag::Object),
            )),
            Self::Deposit { api, bank } => Some((
                api.bot.clone(),
                Dest::Node(bank.clone()),
                Some(DestinationFlag::Object),
            )),
            _ => None,
        }
    }
    /// The binary name of the pulse's class (pulse telemetry classifies by
    /// it: a name holding `ScriptAPI`, or `BotScriptPulse`).
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Script(_) => "core.game.bots.GeneralBotCreator$BotScriptPulse",
            Self::Transition(_) => "core.game.bots.GeneralBotCreator$TransitionPulse",
            Self::Follow { .. } => "core.game.bots.AIPlayer$1",
            Self::Walk { .. } => "core.game.bots.ScriptAPI$walkToIterator$1",
            Self::WalkArray(_) => "core.game.bots.ScriptAPI$walkArray$1",
            Self::Teleport { to_ge: true, .. } => "core.game.bots.ScriptAPI$teleportToGE$1",
            Self::Teleport { to_ge: false, .. } => "core.game.bots.ScriptAPI$teleport$1",
            Self::SellOnGe { .. } => "core.game.bots.ScriptAPI$sellOnGE$toCounterPulse",
            Self::SellAll { .. } => "core.game.bots.ScriptAPI$sellAllOnGe$toCounterPulseAll",
            Self::SellAllAdv { .. } => "core.game.bots.ScriptAPI$sellAllOnGeAdv$toCounterPulseAll",
            Self::Deposit { .. } => "core.game.bots.ScriptAPI$depositAtBank$BankingPulse",
            Self::BankItem { .. } => "core.game.bots.ScriptAPI$bankItem$BankingPulse",
            Self::BankAll(_) => "core.game.bots.ScriptAPI$bankAll$BankingPulse",
            Self::Buy(_) => "core.game.bots.ScriptAPI$buyFromGE$1$1",
        }
    }
    /// `pulse()`.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        match self {
            Self::Script(p) => p.pulse(host),
            Self::Transition(p) => p.pulse(host),
            Self::Follow { bot, entity } => {
                host.face(bot, entity)?;
                Ok(false)
            }
            Self::Walk { .. } => Ok(true),
            Self::WalkArray(p) => p.pulse(host),
            Self::Teleport { bot, location, .. } => {
                host.unlock(bot)?;
                host.properties_set_teleport_location(bot, *location)?;
                host.pulse_manager_clear(bot)?;
                host.animator_reset(bot)?;
                Ok(true)
            }
            Self::SellOnGe { api, id } => api.sell_on_ge_pulse(host, *id),
            Self::SellAll { api } => api.sell_all_pulse(host, false),
            Self::SellAllAdv { api, .. } => api.sell_all_pulse(host, true),
            Self::Deposit { api, bank } => api.deposit_pulse(host, bank),
            Self::BankItem { api, item } => api.bank_item_pulse(host, *item),
            Self::BankAll(p) => p.pulse(host),
            Self::Buy(p) => p.pulse(),
        }
    }
    /// `stop()`: `BotScriptPulse` overrides it; every other pulse's is
    /// `Pulse.stop`, the host's `running = false`.
    pub fn stop(&self, host: &mut H) -> Result<()> {
        match self {
            Self::Script(p) => p.stop(host),
            _ => Ok(()),
        }
    }
}

/// The statics of the engine's classes: one value per world.
pub struct StaticsState<H: Host> {
    /// Whether `AIPlayer`'s static initialiser ran (`loadNames("botnames.txt")`).
    pub initialised: bool,
    /// `AIPlayer.currentUID`.
    pub current_uid: i32,
    /// `AIPlayer.botNames`.
    pub bot_names: Vec<String>,
    /// `AIPlayer.botMapping`.
    pub bot_mapping: HashMap<i32, AiPlayer<H>>,
    /// `AIPlayer.OSRScopyLine`.
    pub osrs_copy_line: Option<String>,
    /// `ArtificialSession.SINGLETON`.
    pub session: ArtificialSession<H::Node>,
    /// `AIRepository.groundItems`, keyed by the dropper.
    pub ground_items: Vec<(H::Node, Vec<H::Node>)>,
    /// `AIRepository.GEOffers`.
    pub ge_offers: Vec<(H::Node, BotOffer<H::Node>)>,
    /// `AIRepository.PulseRepository`: lower-case user name to pulse.
    pub pulse_repository: JavaHashMap<Option<String>, ScriptPulse<H>>,
    /// `GeneralBotCreator.botPulsesTriggeredThisTick`.
    pub bot_pulses_triggered_this_tick: i32,
    /// `PvMBotsBuilder.botsSpawned`.
    pub bots_spawned: i32,
    /// `PlayerScripts.identifierMap`.
    pub identifier_map: JavaHashMap<Option<String>, PlayerScript<H::ScriptClass>>,
}

/// A shared handle to the statics. No lock is held across a service.
pub struct Statics<H: Host>(Shared<StaticsState<H>>);
impl<H: Host> Clone for Statics<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> Default for Statics<H> {
    fn default() -> Self {
        Self::new()
    }
}
impl<H: Host> Statics<H> {
    pub fn new() -> Self {
        Self(Shared::new(StaticsState {
            initialised: false,
            current_uid: 1,
            bot_names: Vec::new(),
            bot_mapping: HashMap::new(),
            osrs_copy_line: None,
            session: ArtificialSession::new(),
            ground_items: Vec::new(),
            ge_offers: Vec::new(),
            pulse_repository: JavaHashMap::new(),
            bot_pulses_triggered_this_tick: 0,
            bots_spawned: 0,
            identifier_map: JavaHashMap::new(),
        }))
    }
    pub fn read<T>(&self, f: impl FnOnce(&StaticsState<H>) -> T) -> T {
        f(&self.0.read())
    }
    pub fn write<T>(&self, f: impl FnOnce(&mut StaticsState<H>) -> T) -> T {
        f(&mut self.0.write())
    }
}

/// Every service the engine reaches. One `Node` type stands for players,
/// NPCs, scenery, ground items and item nodes.
pub trait Host: Sized {
    type Node: Clone + PartialEq + std::fmt::Debug;
    /// A `Path`.
    type Path;
    /// A `Region`.
    type Region;
    /// Any `Pulse` object a pulse manager holds.
    type PulseRef: Clone;
    /// A `Consumable`.
    type Consumable;
    /// A `Quests` constant.
    type Quest: Clone + std::fmt::Debug;
    /// A `Quest` object.
    type QuestObject;
    /// An `Option` of an `InteractPlugin`.
    type InteractOption: Clone;
    /// The content class of a `Script` object.
    type ScriptClass: Clone + std::fmt::Debug;
    /// What `ScriptProcessor.getActiveScript()` answers.
    type ActiveScript;
    /// A `PestControlTestBot` or `PestControlTestBot2` object (content).
    type PestControlBot;

    // ---- plain fields and getters (not calls the original makes)
    /// The engine's statics.
    fn statics(&mut self) -> Statics<Self>;
    /// The `Repository` object (`Repository.getPlayers()` is its player list).
    fn repository(&mut self) -> super::repository::Repository<Self::Node>;
    /// `Node.getLocation()` / the `location` field.
    fn location(&mut self, node: &Self::Node) -> Option<Location>;
    /// `instanceof` / `is`.
    fn kind(&mut self, node: &Self::Node) -> NodeKind;
    /// `Node.getId()`.
    fn id(&mut self, node: &Self::Node) -> i32;
    /// `Node.getName()`.
    fn name(&mut self, node: &Self::Node) -> Option<String>;
    /// `Node.isActive()`.
    fn is_active(&mut self, node: &Self::Node) -> bool;
    /// `Node.getDirection()`.
    fn direction(&mut self, node: &Self::Node) -> Option<Direction>;
    /// `Node.setIndex(index)`, as `NodeList.add` calls it.
    fn set_index(&mut self, node: &Self::Node, index: i32);
    /// `Player.artificial = artificial`.
    fn set_artificial(&mut self, player: &Self::Node, artificial: bool);
    /// The `AIPlayer` object a player node is (`as? AIPlayer`).
    fn ai_player(&mut self, node: &Self::Node) -> Option<AiPlayer<Self>>;
    /// Binds a new `AIPlayer` object to the player node it extends.
    fn attach_ai_player(&mut self, node: &Self::Node, ai: &AiPlayer<Self>);
    /// `ServerConstants.BOT_DATA_PATH`.
    fn bot_data_path(&mut self) -> Option<String>;
    /// The text of a file as the JVM's default charset decodes it, or `None`
    /// where `new Scanner(new File(path))` fails with FileNotFoundException.
    fn read_file(&mut self, path: &str) -> Option<String>;
    /// `ServerConstants.MAX_PATHFIND_DISTANCE`.
    fn max_pathfind_distance(&mut self) -> i32;
    /// `ServerConstants.SERVER_GE_NAME`.
    fn server_ge_name(&mut self) -> String;
    /// `Server.running`.
    fn server_running(&mut self) -> bool;
    /// `GameWorld.settings?.ge_announcement_limit`: `None` without settings.
    fn ge_announcement_limit(&mut self) -> Option<i32>;
    /// `ItemDefinition.forId(id).getName()`.
    fn item_name(&mut self, id: i32) -> Option<String>;
    /// `GroundItem.getDropper()`.
    fn dropper(&mut self, ground: &Self::Node) -> Option<Self::Node>;
    /// `Option.getName()`.
    fn option_name(&mut self, option: &Self::InteractOption) -> Option<String>;
    /// `pulse is MovementPulse`.
    fn is_movement_pulse(&mut self, pulse: &Self::PulseRef) -> bool;
    /// `Path.isMoveNear()`.
    fn path_is_move_near(&mut self, path: &Self::Path) -> bool;
    /// `script.getClass().getSimpleName()`.
    fn script_simple_name(&mut self, class: &Self::ScriptClass) -> String;
    /// `script is Idler`.
    fn script_is_idler(&mut self, class: &Self::ScriptClass) -> bool;
    /// `(script as Adventurer).state.name` when the script is an `Adventurer`.
    fn adventurer_state(&mut self, script: &Script<Self>) -> Option<String>;

    // ---- random draws
    /// `RandomFunction.random(maxValue)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `RandomFunction.random(a, b)`.
    fn random_between(&mut self, a: i32, b: i32) -> Result<i32>;
    /// `RandomFunction.random(a, b)` of doubles.
    fn random_double(&mut self, a: f64, b: f64) -> Result<f64>;
    /// `RandomFunction.getRandom(maxValue)`.
    fn get_random(&mut self, max: i32) -> Result<i32>;
    /// `RandomFunction.roll(chance)`.
    fn roll(&mut self, chance: i32) -> Result<bool>;
    /// `RandomFunction.linearDecreaseRand(max)`.
    fn linear_decrease_rand(&mut self, max: i32) -> Result<i32>;
    /// `RandomFunction.normalRandDist(max)`.
    fn normal_rand_dist(&mut self, max: i32) -> Result<i32>;
    /// `new Random().nextInt(bound)` of `java.util.Random`.
    fn new_random_next_int(&mut self, bound: i32) -> Result<i32>;
    /// `Random().nextInt()` of `java.util.Random`.
    fn new_random_int(&mut self) -> Result<i32>;
    /// `kotlin.random.Random.nextInt(from, until)`, `from < until`.
    fn kotlin_next_int(&mut self, from: i32, until: i32) -> Result<i32>;
    /// The index `Array.random()` / `Collection.random()` picks of `size`
    /// elements (`kotlin.random.Random.nextInt(size)`), `size > 0`.
    fn kotlin_random_index(&mut self, size: i32) -> Result<i32>;

    // ---- Player, Entity and Node members
    /// `new Player(new PlayerDetails(username))`.
    fn new_player(&mut self, username: &str) -> Result<Self::Node>;
    /// `setLocation(location)`.
    fn set_location(&mut self, node: &Self::Node, location: Option<Location>) -> Result<()>;
    /// `getDetails().setSession(ArtificialSession.getSingleton())`.
    fn details_set_session(&mut self, player: &Self::Node) -> Result<()>;
    /// `setDirection(direction)`.
    fn set_direction(&mut self, node: &Self::Node, direction: Direction) -> Result<()>;
    /// `Player.init()` (`super.init()`).
    fn player_init(&mut self, player: &Self::Node) -> Result<()>;
    /// `Player.tick()` (`super.tick()`).
    fn player_tick(&mut self, player: &Self::Node) -> Result<()>;
    /// `Player.reset()` (`super.reset()`).
    fn player_reset(&mut self, player: &Self::Node) -> Result<()>;
    /// `Player.finalizeDeath(killer)` (`super.finalizeDeath`).
    fn player_finalize_death(
        &mut self,
        player: &Self::Node,
        killer: Option<&Self::Node>,
    ) -> Result<()>;
    /// `fullRestore()`.
    fn full_restore(&mut self, player: &Self::Node) -> Result<()>;
    /// `face(entity)`.
    fn face(&mut self, entity: &Self::Node, target: &Self::Node) -> Result<()>;
    /// `faceLocation(location)`.
    fn face_location(&mut self, entity: &Self::Node, location: Option<Location>) -> Result<()>;
    /// `inCombat()`.
    fn in_combat(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `attack(target)`.
    fn attack(&mut self, entity: &Self::Node, target: &Self::Node) -> Result<()>;
    /// `isTeleBlocked()`.
    fn is_tele_blocked(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `lock()`.
    fn lock(&mut self, entity: &Self::Node) -> Result<()>;
    /// `lock(ticks)`.
    fn lock_for(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `unlock()`.
    fn unlock(&mut self, entity: &Self::Node) -> Result<()>;
    /// `visualize(Animation(animation), Graphics(graphics))`.
    fn visualize(&mut self, entity: &Self::Node, animation: i32, graphics: i32) -> Result<()>;
    /// `sendChat(message)`.
    fn send_chat(&mut self, entity: &Self::Node, message: &str) -> Result<()>;
    /// `updateMasks.register(EntityFlag.Chat, ChatMessage(player, message, 0, 0))`.
    fn register_chat_mask(&mut self, player: &Self::Node, message: &str) -> Result<()>;
    /// `hasModalOpen()`.
    fn has_modal_open(&mut self, player: &Self::Node) -> Result<bool>;
    /// `debug(text)`.
    fn debug(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `setAttribute(key, true)`.
    fn set_attribute_true(&mut self, entity: &Self::Node, key: &str) -> Result<()>;
    /// `setAttribute("botting:script", pulse)`.
    fn set_script_attribute(
        &mut self,
        player: &Self::Node,
        pulse: &ScriptPulse<Self>,
    ) -> Result<()>;
    /// `getAttribute("botting:drops", null)`.
    fn botting_drops(&mut self, player: &Self::Node) -> Result<Option<Vec<Self::Node>>>;
    /// `setAttribute("botting:drops", items)`.
    fn set_botting_drops(&mut self, player: &Self::Node, items: &[Self::Node]) -> Result<()>;
    /// `animator.reset()`.
    fn animator_reset(&mut self, entity: &Self::Node) -> Result<()>;
    /// `impactHandler.disabledTicks = ticks`.
    fn impact_set_disabled_ticks(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `walkingQueue.isMoving`.
    fn walking_queue_is_moving(&mut self, entity: &Self::Node) -> Result<bool>;

    // ---- appearance, settings, flags, interfaces
    /// `getAppearance().setGender(gender)`.
    fn appearance_set_gender(&mut self, player: &Self::Node, gender: Gender) -> Result<()>;
    /// `getAppearance().sync()`.
    fn appearance_sync(&mut self, player: &Self::Node) -> Result<()>;
    /// `getAppearance().copy(other.getAppearance())`.
    fn appearance_copy(&mut self, player: &Self::Node, from: &Self::Node) -> Result<()>;
    /// `appearance.parse(json)`.
    fn appearance_parse(&mut self, player: &Self::Node, json: &Json) -> Result<()>;
    /// `CharacterDesign.randomize(player, head)`.
    fn character_design_randomize(&mut self, player: &Self::Node, head: bool) -> Result<()>;
    /// `getSettings().setRunToggled(enabled)`.
    fn settings_set_run_toggled(&mut self, player: &Self::Node, enabled: bool) -> Result<()>;
    /// `getPlayerFlags().isUpdateSceneGraph()`.
    fn flags_is_update_scene_graph(&mut self, player: &Self::Node) -> Result<bool>;
    /// `getPlayerFlags().setLastSceneGraph(location)`.
    fn flags_set_last_scene_graph(
        &mut self,
        player: &Self::Node,
        location: Option<Location>,
    ) -> Result<()>;
    /// `getInterfaceManager().openDefaultTabs()`.
    fn interface_open_default_tabs(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.closeChatbox()`.
    fn interface_close_chatbox(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.openChatbox(id)`.
    fn interface_open_chatbox(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `interfaceManager.close()`.
    fn interface_close(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.openOverlay(Component(id))`.
    fn interface_open_overlay(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `dialogueInterpreter.close()`.
    fn dialogue_close(&mut self, player: &Self::Node) -> Result<()>;
    /// `packetDispatch.sendInterfaceConfig(interface, child, hide)`.
    fn send_interface_config(
        &mut self,
        player: &Self::Node,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()>;
    /// `packetDispatch.sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;

    // ---- properties, combat, skull, prayer
    /// `getProperties().setSpawnLocation(location)`.
    fn properties_set_spawn_location(
        &mut self,
        entity: &Self::Node,
        location: Option<Location>,
    ) -> Result<()>;
    /// `properties.teleportLocation = location`.
    fn properties_set_teleport_location(
        &mut self,
        entity: &Self::Node,
        location: Location,
    ) -> Result<()>;
    /// `properties.isMultiZone`.
    fn properties_is_multi_zone(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `getProperties().getCombatPulse().getVictim()`.
    fn combat_victim(&mut self, entity: &Self::Node) -> Result<Option<Self::Node>>;
    /// `properties.combatPulse.delayNextAttack(ticks)`.
    fn combat_delay_next_attack(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `getSkullManager().isWilderness()`.
    fn skull_is_wilderness(&mut self, player: &Self::Node) -> Result<bool>;
    /// `getSkullManager().setLevel(level)`.
    fn skull_set_level(&mut self, player: &Self::Node, level: i32) -> Result<()>;
    /// `WildernessZone.getWilderness(entity)`.
    fn wilderness_level(&mut self, entity: &Self::Node) -> Result<i32>;
    /// `prayer.toggle(type)`; `prayer` is the `PrayerType` constant's name,
    /// `None` for a null element.
    fn prayer_toggle(&mut self, player: &Self::Node, prayer: Option<&str>) -> Result<()>;
    /// `definition.hasAction(action)` of an NPC.
    fn npc_has_action(&mut self, npc: &Self::Node, action: &str) -> Result<bool>;

    // ---- skills
    /// `skills.setStaticLevel(skill, level)`.
    fn skills_set_static_level(
        &mut self,
        entity: &Self::Node,
        skill: i32,
        level: i32,
    ) -> Result<()>;
    /// `skills.setLevel(skill, level)`.
    fn skills_set_level(&mut self, entity: &Self::Node, skill: i32, level: i32) -> Result<()>;
    /// `skills.getStaticLevel(skill)`.
    fn skills_static_level(&mut self, entity: &Self::Node, skill: i32) -> Result<i32>;
    /// `skills.getLevel(skill)`.
    fn skills_level(&mut self, entity: &Self::Node, skill: i32) -> Result<i32>;
    /// `skills.getExperience(skill)`.
    fn skills_experience(&mut self, entity: &Self::Node, skill: i32) -> Result<f64>;
    /// `skills.addExperience(skill, experience)`.
    fn skills_add_experience(
        &mut self,
        entity: &Self::Node,
        skill: i32,
        experience: f64,
    ) -> Result<()>;
    /// `skills.levelFromXP(experience)`.
    fn skills_level_from_xp(&mut self, entity: &Self::Node, experience: f64) -> Result<i32>;
    /// `skills.getLifepoints()`.
    fn skills_lifepoints(&mut self, entity: &Self::Node) -> Result<i32>;
    /// `skills.setLifepoints(lifepoints)`.
    fn skills_set_lifepoints(&mut self, entity: &Self::Node, lifepoints: i32) -> Result<()>;
    /// `skills.updateCombatLevel()`.
    fn skills_update_combat_level(&mut self, entity: &Self::Node) -> Result<()>;
    /// `skills.getHighestCombatSkillId()`.
    fn skills_highest_combat_skill(&mut self, entity: &Self::Node) -> Result<i32>;
    /// `getSkills().copy(other.getSkills())`.
    fn skills_copy(&mut self, entity: &Self::Node, from: &Self::Node) -> Result<()>;

    // ---- containers
    /// `container.get(slot)`.
    fn container_get(
        &mut self,
        player: &Self::Node,
        which: Which,
        slot: i32,
    ) -> Result<Option<Item>>;
    /// `container.add(item)`.
    fn container_add(&mut self, player: &Self::Node, which: Which, item: Item) -> Result<bool>;
    /// `equipment.add(item, fire, fromInventory)`.
    fn equipment_add(
        &mut self,
        player: &Self::Node,
        item: Item,
        fire: bool,
        from_inventory: bool,
    ) -> Result<bool>;
    /// `equipment.add(item, inventorySlot, fire, fromInventory)`.
    fn equipment_add_slot(
        &mut self,
        player: &Self::Node,
        item: Item,
        slot: i32,
        fire: bool,
        from_inventory: bool,
    ) -> Result<bool>;
    /// `container.replace(item, slot)`.
    fn container_replace(
        &mut self,
        player: &Self::Node,
        which: Which,
        item: Item,
        slot: i32,
    ) -> Result<()>;
    /// `container.remove(item)`.
    fn container_remove(&mut self, player: &Self::Node, which: Which, item: Item) -> Result<bool>;
    /// `container.clear()`.
    fn container_clear(&mut self, player: &Self::Node, which: Which) -> Result<()>;
    /// `container.refresh()`.
    fn container_refresh(&mut self, player: &Self::Node, which: Which) -> Result<()>;
    /// `container.toArray()`.
    fn container_to_array(
        &mut self,
        player: &Self::Node,
        which: Which,
    ) -> Result<Vec<Option<Item>>>;
    /// `container.capacity()`.
    fn container_capacity(&mut self, player: &Self::Node, which: Which) -> Result<i32>;
    /// `container.getAmount(id)`.
    fn container_amount(&mut self, player: &Self::Node, which: Which, id: i32) -> Result<i32>;
    /// `container.getAmount(item)`.
    fn container_amount_of(&mut self, player: &Self::Node, which: Which, item: Item)
    -> Result<i32>;
    /// `container.getItem(item)`.
    fn container_get_item(
        &mut self,
        player: &Self::Node,
        which: Which,
        item: Item,
    ) -> Result<Option<Item>>;
    /// `container.containsItem(item)`.
    fn container_contains_item(
        &mut self,
        player: &Self::Node,
        which: Which,
        item: Item,
    ) -> Result<bool>;
    /// `container.contains(id, amount)`.
    fn container_contains(
        &mut self,
        player: &Self::Node,
        which: Which,
        id: i32,
        amount: i32,
    ) -> Result<bool>;
    /// `container.hasSpaceFor(item)`.
    fn container_has_space_for(
        &mut self,
        player: &Self::Node,
        which: Which,
        item: Item,
    ) -> Result<bool>;
    /// `container.getMaximumAdd(item)`.
    fn container_maximum_add(
        &mut self,
        player: &Self::Node,
        which: Which,
        item: Item,
    ) -> Result<i32>;
    /// `container.copy(other's container)`.
    fn container_copy(
        &mut self,
        player: &Self::Node,
        which: Which,
        from: &Self::Node,
    ) -> Result<()>;
    /// `container.parse(jsonArray)`.
    fn container_parse(&mut self, player: &Self::Node, which: Which, json: &Json) -> Result<()>;

    // ---- item definitions
    /// `item.definition.handlers[key]` / `getOrDefault(key, null)`.
    fn item_handler(&mut self, id: i32, key: &str) -> Result<Option<Handler>>;
    /// `ItemDefinition.forId(id).noteId`.
    fn item_note_id(&mut self, id: i32) -> Result<i32>;
    /// `ItemDefinition.forId(id).getAlchemyValue(true)`.
    fn item_alchemy_value(&mut self, id: i32) -> Result<i32>;
    /// `item.definition.isTradeable`.
    fn item_is_tradeable(&mut self, id: i32) -> Result<bool>;
    /// `ItemDefinition.forId(id).value`.
    fn item_value(&mut self, id: i32) -> Result<i32>;
    /// `itemDefinition(id).getConfiguration(ItemConfigParser.GE_PRICE)`.
    fn item_ge_price(&mut self, id: i32) -> Result<Option<i32>>;

    // ---- pulses and scripts
    /// `pulseManager.run(pulse)`; `standard` when `PulseType.STANDARD` is named.
    fn pulse_manager_run(
        &mut self,
        entity: &Self::Node,
        pulse: Pulse<Self>,
        standard: bool,
    ) -> Result<()>;
    /// `GameWorld.Pulser.submit(pulse)`.
    fn pulser_submit(&mut self, pulse: Pulse<Self>) -> Result<()>;
    /// `pulseManager.hasPulseRunning()`.
    fn pulse_manager_has_pulse_running(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `pulseManager.getCurrent()`.
    fn pulse_manager_current(&mut self, entity: &Self::Node) -> Result<Option<Self::PulseRef>>;
    /// `pulseManager.clear()`.
    fn pulse_manager_clear(&mut self, entity: &Self::Node) -> Result<()>;
    /// `pulse.stop()` of a pulse a manager holds.
    fn pulse_stop(&mut self, pulse: &Self::PulseRef) -> Result<()>;
    /// `Pulse.stop()` of a `BotScriptPulse` (`super.stop()`).
    fn script_pulse_super_stop(&mut self, pulse: &ScriptPulse<Self>) -> Result<()>;
    /// `pulse::class.java.name`.
    fn pulse_class_name(&mut self, pulse: &Self::PulseRef) -> Result<String>;
    /// The text `"" + pulse` gives (`Object.toString`, `"null"` for none).
    fn pulse_text(&mut self, pulse: Option<&Self::PulseRef>) -> Result<String>;
    /// `scripts.getActiveScript()`.
    fn active_script(&mut self, entity: &Self::Node) -> Result<Option<Self::ActiveScript>>;
    /// `script.execution!!::class.java.name`; `None` for a null execution.
    fn active_script_execution_class(
        &mut self,
        script: &Self::ActiveScript,
    ) -> Result<Option<String>>;
    /// `scripts.delay`.
    fn scripts_delay(&mut self, entity: &Self::Node) -> Result<i32>;
    /// The abstract `Script.tick()` of a content script.
    fn script_tick(&mut self, script: &Script<Self>) -> Result<()>;
    /// The abstract `Script.newInstance()`.
    fn script_new_instance(&mut self, script: &Script<Self>) -> Result<Script<Self>>;
    /// `getWorldTicks()`.
    fn world_ticks(&mut self) -> Result<i32>;
    /// `GlobalScope.launch { .. }`: the host runs [`script_api::BuyTask::run`]
    /// off the tick.
    fn launch(&mut self, task: script_api::BuyTask<Self>) -> Result<()>;

    // ---- quests
    /// `questRepository.getQuest(quest)`.
    fn quest(
        &mut self,
        player: &Self::Node,
        quest: &Self::Quest,
    ) -> Result<Option<Self::QuestObject>>;
    /// `questRepository.setStage(quest, stage)`.
    fn quest_set_stage(
        &mut self,
        player: &Self::Node,
        quest: Option<&Self::QuestObject>,
        stage: i32,
    ) -> Result<()>;

    // ---- world
    /// `Pathfinder.find(mover, destination, ..)`.
    fn pathfinder_find(
        &mut self,
        mover: &Self::Node,
        destination: &Dest<Self::Node>,
        finder: Finder,
    ) -> Result<Self::Path>;
    /// `path.walk(entity)`.
    fn path_walk(&mut self, path: &Self::Path, entity: &Self::Node) -> Result<()>;
    /// `RegionManager.getLocalNPCs(location, radius)`.
    fn local_npcs(&mut self, location: Option<Location>, radius: i32) -> Result<Vec<Self::Node>>;
    /// `RegionManager.getObject(x, y, z)`.
    fn region_object(&mut self, x: i32, y: i32, z: i32) -> Result<Option<Self::Node>>;
    /// `RegionManager.forId(regionId)`.
    fn region_for_id(&mut self, region_id: i32) -> Result<Self::Region>;
    /// `region.assembleObjectList(z)`.
    fn region_object_list(
        &mut self,
        region: &Self::Region,
        z: i32,
    ) -> Result<Vec<Option<Self::Node>>>;
    /// `region.assembleNodeList(z)`.
    fn region_node_list(
        &mut self,
        region: &Self::Region,
        z: i32,
    ) -> Result<Vec<Option<Self::Node>>>;
    /// `GroundItemManager.getItems().contains(item)`.
    fn ground_items_contain(&mut self, item: Option<&Self::Node>) -> Result<bool>;
    /// `Repository.sendGrandExchangeNews(text)`.
    fn send_grand_exchange_news(&mut self, text: &str) -> Result<()>;
    /// `GrandExchange.addBotOffer(id, amount)`.
    fn ge_add_bot_offer(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `log(ScriptAPI::class.java, Log.WARN, message)`.
    fn log_warn(&mut self, message: &str) -> Result<()>;
    /// `throwable.printStackTrace()` of an exception the engine catches.
    fn print_stack_trace(&mut self, class: &str);

    // ---- interaction
    /// `node.getInteraction()`: whether the node has an `InteractPlugin`.
    fn interaction(&mut self, node: &Self::Node) -> Result<bool>;
    /// `interaction.getOptions()`.
    fn interaction_options(
        &mut self,
        node: &Self::Node,
    ) -> Result<Vec<Option<Self::InteractOption>>>;
    /// `interaction.get(index)`.
    fn interaction_get(
        &mut self,
        node: &Self::Node,
        index: i32,
    ) -> Result<Option<Self::InteractOption>>;
    /// `interaction.handle(player, option)`.
    fn interaction_handle(
        &mut self,
        node: &Self::Node,
        player: &Self::Node,
        option: Option<&Self::InteractOption>,
    ) -> Result<()>;
    /// `InteractionListeners.run(id, type, option, player, node)`.
    fn listeners_run(
        &mut self,
        id: i32,
        kind: IntType,
        option: &str,
        player: &Self::Node,
        node: &Self::Node,
    ) -> Result<bool>;
    /// `InteractionListeners.run(used, with, type, player)`.
    fn listeners_run_use(
        &mut self,
        used: Option<Item>,
        with: &Self::Node,
        kind: IntType,
        player: &Self::Node,
    ) -> Result<bool>;
    /// `node.asScenery().getChild(player)`.
    fn scenery_child(
        &mut self,
        scenery: &Self::Node,
        player: &Self::Node,
    ) -> Result<Option<Self::Node>>;
    /// `PluginInteractionManager.handle(player, NodeUsageEvent(player, 0, used, with))`.
    fn plugin_interaction_handle(
        &mut self,
        player: &Self::Node,
        used: &Used<Self::Node>,
        with: &Used<Self::Node>,
    ) -> Result<bool>;
    /// `UseWithHandler.run(NodeUsageEvent(player, 0, used, with))`.
    fn use_with_run(
        &mut self,
        player: &Self::Node,
        used: &Used<Self::Node>,
        with: &Used<Self::Node>,
    ) -> Result<()>;

    // ---- consumables
    /// `Consumables.getConsumableById(id)?.consumable`.
    fn consumable(&mut self, id: i32) -> Result<Option<Self::Consumable>>;
    /// `consumable.consume(food, player)`.
    fn consume(
        &mut self,
        consumable: Food<Self::Consumable>,
        food: Option<Item>,
        player: &Self::Node,
    ) -> Result<()>;

    // ---- content bots the engine constructs
    /// `PestControlTestBot(location)` / `PestControlTestBot2(location)`.
    fn new_pest_control_test_bot(
        &mut self,
        second: bool,
        location: Location,
    ) -> Result<Self::PestControlBot>;
}

/// `node.getLocation()` where the original dereferences it.
pub(crate) fn loc<H: Host>(host: &mut H, node: &H::Node) -> Result<Location> {
    host.location(node).ok_or(Error::Null)
}

/// `player.getUsername()`: `AIPlayer` overrides it with its own field; a
/// plain player's is `StringUtils.formatDisplayName(getName())`.
pub fn username<H: Host>(host: &mut H, player: &H::Node) -> Result<String> {
    match host.ai_player(player) {
        Some(ai) => ai.username().ok_or(Error::Null),
        None => {
            let name = host.name(player).ok_or(Error::Null)?;
            Ok(super::social::names::format_display_name(&name))
        }
    }
}

/// `bot.username.lowercase()`, the key of `AIRepository.PulseRepository`.
pub(crate) fn username_key<H: Host>(host: &mut H, player: &H::Node) -> Result<String> {
    Ok(rs09_game_core::java_lower::lowercase_root(&username(
        host, player,
    )?))
}
