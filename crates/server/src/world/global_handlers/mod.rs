//! The global handlers a plain playable world needs
//! (`content/global/handlers/{player,iface,scenery,npc}`): the player option
//! handlers, the tab and game-frame interface listeners, the bank booth,
//! chest and deposit-box listeners, the look-at, search and ladder option
//! handlers, Peer the Seer's deposit and the training dummy.
//!
//! | Original | Here |
//! | --- | --- |
//! | `AttackOptionPlugin`, `FollowOptionPlugin` (with its MovementPulse body and stop), `RequestOption`, `LoginValidationPlugin` | [`player`] |
//! | `LogoutTabInterface`, `SettingsTabInterface`, `StatsTabInterface`, `EquipmentTabInterface`, `CombatTabInterface` (and its two pulses), `MagicTabInterface`, `PrayerTabInterface`, `QuestTabInterface`, `ClanTabInterface` (and its prefix input), `EmoteTabInterface`, `MusicTabInterface` | [`tabs`] |
//! | `MainGameInterface`, `LoginInterface`, `DeathInterface`, `CharacterDesignInterface`, `WorldMapInterface`, `ExperienceInterface`, `FriendsListInterface.open`, `RulesAndInfo`/`RulesListener`, `ScrollInterface`, `GenericItemSelect`, `AutocastSelectPlugin`, `TeleotherInterface`, `RequestAssistInterface`, `BankDepositBoxInterface`, `BankChargeInterface` | [`ifaces`] |
//! | `BankBoothListener`, `BankChestListener`, `BankDepositBoxListener`, `LookAtOptionPlugin`, `SearchOptionPlugin`, `LadderManagingPlugin` | [`scenery`] |
//! | `ClimbActionHandler` (with `ClimbDialogue`), `SpecialLadders` | [`climb`], [`special_ladders`] |
//! | `NPCDepositListener`, `DummyAttackPlugin` | [`npc`] |
//! | the classes' registrations, in the original's scan order | [`registry`] |
//!
//! Each listener is a typed native callback ([`registry::Listener`],
//! [`registry::Handler`], [`registry::Plugin`]) registered into the native
//! registries (InterfaceListeners, InteractionListeners, the component
//! definition plugins, the option handlers) by [`registry::Registrar`]; its
//! body is a function here over [`Host`]. Every call the original makes
//! outside these bodies is a required [`Host`] method with no default; the
//! world binds them to the native systems that already exist (settings,
//! interface manager, prayer, music, quests, diaries, trade requests, the
//! assist session, EquipHandler, the InteractionListeners registry, the
//! input prompt's BankUtils, the clans) and leaves content (the quest
//! requirement tables, BankerNPC's Lunar Isle rule, climbing, activities,
//! character design) a named service. See docs/global-handlers.md.
pub mod climb;
pub mod ifaces;
pub mod npc;
pub mod player;
pub mod registry;
pub mod scenery;
pub mod special_ladders;
pub mod tabs;

pub use crate::world::prayer::PrayerType;
pub use crate::world::spells::SpellBook;

/// What stops a call, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `NullPointerException` (Kotlin's null checks and casts included).
    Null,
    /// `ClassCastException`.
    Cast,
    /// `IndexOutOfBoundsException`.
    Index,
    /// A service failed with what it threw (its message, if any).
    Binding(Option<String>),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// The node classes the bodies cast to or test with `is`/`instanceof`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    /// `AIPlayer` (also a `Player`).
    Bot,
    Npc,
    /// `BankerNPC` (also an `NPC`).
    Banker,
    Scenery,
    Item,
}
impl Kind {
    pub fn is_player(self) -> bool {
        matches!(self, Self::Player | Self::Bot)
    }
    pub fn is_npc(self) -> bool {
        matches!(self, Self::Npc | Self::Banker)
    }
}

/// `Java (Player) node`: null passes, another class fails the cast.
pub(crate) fn java_cast<H: Host + ?Sized>(
    h: &H,
    node: Option<&H::Node>,
    is: impl Fn(Kind) -> bool,
) -> Result<()> {
    match node {
        Some(n) if !is(h.kind(n)) => Err(Error::Cast),
        _ => Ok(()),
    }
}
/// Kotlin `node as Player`: null fails Kotlin's check, another class the cast.
pub(crate) fn kotlin_cast<'a, H: Host + ?Sized>(
    h: &H,
    node: Option<&'a H::Node>,
    is: impl Fn(Kind) -> bool,
) -> Result<&'a H::Node> {
    let node = node.ok_or(Error::Null)?;
    if is(h.kind(node)) {
        Ok(node)
    } else {
        Err(Error::Cast)
    }
}

/// An attribute's value as the original stores it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value<O> {
    Int(i32),
    Long(i64),
    Bool(bool),
    Text(String),
    Location([i32; 3]),
    /// A Kotlin function or plugin object (item-select callback, caller).
    Object(O),
}
impl<O> Value<O> {
    /// `(Integer) value` / Kotlin's unboxing cast.
    pub fn int(v: Option<Self>, fail: i32) -> Result<i32> {
        match v {
            None => Ok(fail),
            Some(Self::Int(i)) => Ok(i),
            Some(_) => Err(Error::Cast),
        }
    }
    /// `(Boolean) value`.
    pub fn bool(v: Option<Self>, fail: bool) -> Result<bool> {
        match v {
            None => Ok(fail),
            Some(Self::Bool(b)) => Ok(b),
            Some(_) => Err(Error::Cast),
        }
    }
    /// `(String) value`.
    pub fn text(v: Option<Self>, fail: &str) -> Result<String> {
        match v {
            None => Ok(fail.to_owned()),
            Some(Self::Text(t)) => Ok(t),
            Some(_) => Err(Error::Cast),
        }
    }
}

/// The value an input dialogue hands its callback (`value.toString()`).
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Text(String),
    Int(i32),
}
impl Input {
    pub fn to_java_string(&self) -> String {
        match self {
            Self::Text(t) => t.clone(),
            Self::Int(i) => i.to_string(),
        }
    }
}

/// An object attribute's runtime class, for `is Plugin<*>` / the function cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Plugin,
    Function,
    Other,
}

/// `PulseManager.run`'s pulse type (`None` for the one-argument overload).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseType {
    Standard,
    Strong,
}
impl PulseType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::Strong => "STRONG",
        }
    }
}

/// The player settings toggles the handlers call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Run,
    ChatEffects,
    SplitPrivateChat,
    MouseButton,
    AcceptAid,
    Retaliating,
    SpecialBar,
}

/// `IronmanMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IronmanMode {
    None,
    Standard,
    Hardcore,
    Ultimate,
}
impl IronmanMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Standard => "STANDARD",
            Self::Hardcore => "HARDCORE",
            Self::Ultimate => "ULTIMATE",
        }
    }
}

/// `core.api.Container`: where ContentAPI adds and removes items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    Inventory,
    Bank,
    Equipment,
}
impl Store {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "INVENTORY",
            Self::Bank => "BANK",
            Self::Equipment => "EQUIPMENT",
        }
    }
}

/// `ClanRank`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClanRank {
    None,
    Friend,
    Recruit,
    Corporal,
    Sergeant,
    Lieutenant,
    Captain,
    General,
    Owner,
    Administrator,
}
impl ClanRank {
    pub const ALL: [Self; 10] = [
        Self::None,
        Self::Friend,
        Self::Recruit,
        Self::Corporal,
        Self::Sergeant,
        Self::Lieutenant,
        Self::Captain,
        Self::General,
        Self::Owner,
        Self::Administrator,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Friend => "FRIEND",
            Self::Recruit => "RECRUIT",
            Self::Corporal => "CORPORAL",
            Self::Sergeant => "SERGEANT",
            Self::Lieutenant => "LIEUTENANT",
            Self::Captain => "CAPTAIN",
            Self::General => "GENERAL",
            Self::Owner => "OWNER",
            Self::Administrator => "ADMINISTRATOR",
        }
    }
}

/// The clan requirement a ClanTabInterface button sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClanField {
    Join,
    Message,
    Kick,
    Loot,
}
impl ClanField {
    pub fn name(self) -> &'static str {
        match self {
            Self::Join => "join",
            Self::Message => "message",
            Self::Kick => "kick",
            Self::Loot => "loot",
        }
    }
}

/// `Log` levels the handlers log at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Warn,
    Err,
}
impl LogLevel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Warn => "WARN",
            Self::Err => "ERR",
        }
    }
}

/// `Quests` constants ExperienceInterface checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quest {
    RuneMysteries,
    DruidicRitual,
    WolfWhistle,
}
impl Quest {
    pub fn name(self) -> &'static str {
        match self {
            Self::RuneMysteries => "RUNE_MYSTERIES",
            Self::DruidicRitual => "DRUIDIC_RITUAL",
            Self::WolfWhistle => "WOLF_WHISTLE",
        }
    }
}

/// `FacialExpression`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    Neutral,
    Annoyed,
}
impl Expression {
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutral => "NEUTRAL",
            Self::Annoyed => "ANNOYED",
        }
    }
}

/// `TeleportType` of TeleotherInterface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportType {
    TeleOther,
}

/// A run script argument (`sendRunScript(id, types, args...)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptArg {
    Int(i32),
    Text(String),
}

/// What `getExtension(WeaponInterface.class)` holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Extension<W> {
    /// The player's WeaponInterface.
    Weapon(W),
    /// The `WeaponInterfaces.STAFF` constant itself (only an identity
    /// comparison in MainGameInterface can tell it apart).
    StaffConstant,
}

/// A `MusicEntry`'s id and list index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Song {
    pub id: i32,
    pub index: i32,
}

/// `openDialogue(player, dialogue, args...)` arguments the handlers pass.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogueArg {
    /// `NPC(id, location)`: a new NPC object.
    NewNpc { id: i32, location: Option<[i32; 3]> },
}

/// The deferred bodies the handlers hand the world: pulses, tasks, input
/// callbacks and close events. The world stores each with what it was
/// submitted to and runs it through [`run_task`], [`stop_task`] or
/// [`close_event`].
#[derive(Clone, Debug, PartialEq)]
pub enum Task<N, C> {
    /// FollowOptionPlugin's MovementPulse (`pulse` faces the target, `stop`
    /// stops then faces nobody).
    Follow { player: N, target: Option<N> },
    /// CombatTabInterface's first pulse: toggle retaliation.
    Retaliate { player: N },
    /// CombatTabInterface's second pulse: the special attack bar.
    SpecialBar { player: N },
    /// GenericItemSelect's open pulse (`pulse` is false, `stop` closes the
    /// single tab).
    ItemSelect { player: N },
    /// LoginInterface's `runTask { configureGameWorld }`.
    ConfigureWorld { player: N },
    /// RulesListener's `runTask { openFor; sendDialogue }`.
    Rules { player: N },
    /// BankDepositBoxInterface's world task.
    Deposit {
        player: N,
        item: N,
        opcode: i32,
        slot: i32,
    },
    /// `player.bank::refreshDepositBoxInterface` (BankUtils' `after`).
    RefreshDepositBox { player: N },
    /// ClanTabInterface's prefix input.
    ClanPrefix { player: N, clan: Option<C> },
    /// MainGameInterface's report-abuse close event.
    ReportClose,
    /// BankDepositBoxListener's close event.
    DepositBoxClose,
    /// `ClimbActionHandler.climb`'s pulse: the teleport location and messages.
    Climb {
        player: N,
        destination: [i32; 3],
        messages: Vec<String>,
    },
    /// `ClimbDialogue`'s pulse: `climbLadder(player, object, option)`.
    ClimbChoice {
        player: N,
        object: Option<N>,
        option: &'static str,
    },
}

/// What a deferred body hands back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `pulse()`/`close()`'s boolean.
    Bool(bool),
    /// A `() -> Unit` body.
    Unit,
}

/// The services the handlers call, in the original's terms. A method that
/// returns no `Result` is a field read of the original.
pub trait Host {
    /// A node (player, NPC, scenery, item); `==` is identity.
    type Node: Clone + PartialEq + std::fmt::Debug;
    /// A container (inventory, equipment, the death containers, a beast's).
    type Container: Clone;
    /// A pulse the world holds.
    type Pulse: Clone;
    /// A player's WeaponInterface.
    type Weapon: Clone + PartialEq + std::fmt::Debug;
    /// A special attack handler.
    type Special: Clone;
    /// An item's operate handler.
    type Operate: Clone;
    /// A quest (`QuestRepository.forButtonId`).
    type Quest: Clone;
    /// An achievement diary.
    type Diary: Clone;
    /// A clan (`ClanRepository`).
    type Clan: Clone + PartialEq + std::fmt::Debug;
    /// An assist session.
    type Session: Clone;
    /// An object attribute (function or plugin).
    type Object: Clone + PartialEq + std::fmt::Debug;

    // ---------------------------------------------------------- node fields
    fn kind(&self, node: &Self::Node) -> Kind;
    /// `node.getId()`.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// `node.getName()`.
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `node.getLocation()`.
    fn location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `item.getAmount()`.
    fn amount(&mut self, item: &Self::Node) -> Result<i32>;
    /// `String.toLowerCase()` in the world's default locale.
    fn lowercase_default(&self, text: &str) -> String;
    /// `ServerConstants.SERVER_NAME`.
    fn server_name(&mut self) -> String;
    /// `ServerConstants.BANK_BOOTH_QUICK_OPEN`.
    fn booth_quick_open(&mut self) -> bool;
    /// `ServerConstants.BANK_BOOTH_NOTE_ENABLED`.
    fn booth_note_enabled(&mut self) -> bool;
    /// `ServerConstants.BANK_BOOTH_NOTE_UIM`.
    fn booth_note_uim(&mut self) -> bool;
    /// `GameWorld.ticks`.
    fn ticks(&mut self) -> i32;
    /// `System.currentTimeMillis()`.
    fn current_time_millis(&mut self) -> i64;
    /// `GameWorld.settings!!.worldId` (`Null` without settings).
    fn world_id(&mut self) -> Result<i32>;

    // ---------------------------------------------------------- attributes
    /// `entity.getAttribute(key, ...)`: the stored value, if any.
    fn get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>>;
    /// `entity.setAttribute(key, value)`.
    fn set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Object>,
    ) -> Result<()>;
    /// `entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()>;
    /// `entity.attributes[key]` (the map read).
    fn attributes_get(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>>;
    /// ContentAPI `getAttribute(entity, key, default)`: the stored value.
    fn api_get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>>;
    /// ContentAPI `setAttribute`.
    fn api_set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Object>,
    ) -> Result<()>;
    /// ContentAPI `removeAttribute`.
    fn api_remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()>;
    /// ContentAPI `removeAttributes(entity, keys...)`.
    fn api_remove_attributes(&mut self, entity: &Self::Node, keys: &[&str]) -> Result<()>;
    /// The object attribute's class.
    fn object_kind(&mut self, object: &Self::Object) -> ObjectKind;
    /// `(caller as Plugin).handleSelectionCallback(skill, player)`.
    fn selection_callback(
        &mut self,
        object: &Self::Object,
        skill: i32,
        player: &Self::Node,
    ) -> Result<()>;
    /// `(caller as (Int, Player) -> Unit).invoke(skill, player)`.
    fn caller_invoke(
        &mut self,
        object: &Self::Object,
        skill: i32,
        player: &Self::Node,
    ) -> Result<()>;
    /// `(callback as (Int, Int) -> Unit).invoke(slot, option)`.
    fn item_select_invoke(&mut self, object: &Self::Object, slot: i32, option: i32) -> Result<()>;

    // ---------------------------------------------------------- messages, interfaces
    /// ContentAPI `sendMessage`.
    fn api_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.sendMessage`.
    fn message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.packetDispatch.sendMessage`.
    fn packet_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.packetDispatch.sendMessage(text, ticks)`.
    fn packet_message_delay(&mut self, player: &Self::Node, text: &str, ticks: i32) -> Result<()>;
    /// `player.packetDispatch.sendString`.
    fn packet_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()>;
    /// ContentAPI `setInterfaceText`.
    fn set_interface_text(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()>;
    /// ContentAPI `sendDialogue`.
    fn dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `sendNPCDialogue(player, npc, msg, expr, hide)`.
    fn npc_dialogue(
        &mut self,
        player: &Self::Node,
        npc: i32,
        text: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()>;
    /// ContentAPI `sendItemDialogue`.
    fn item_dialogue(&mut self, player: &Self::Node, item: i32, text: &str) -> Result<()>;
    /// ContentAPI `sendDoubleItemDialogue`.
    fn double_item_dialogue(
        &mut self,
        player: &Self::Node,
        first: i32,
        second: i32,
        text: &str,
    ) -> Result<()>;
    /// ContentAPI `openDialogue(player, id, args...)`.
    fn open_dialogue(
        &mut self,
        player: &Self::Node,
        dialogue: i32,
        args: Vec<DialogueArg>,
    ) -> Result<()>;
    /// `DialogueInterpreter.contains(id)`.
    fn dialogue_contains(&mut self, id: i32) -> Result<bool>;
    /// ContentAPI `openInterface`.
    fn open_interface(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// ContentAPI `closeInterface`.
    fn close_interface(&mut self, player: &Self::Node) -> Result<()>;
    /// ContentAPI `openSingleTab`.
    fn open_single_tab(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// ContentAPI `closeTabInterface`.
    fn close_tab_interface(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.open(Component(id))`: false for a null answer.
    fn open(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `component.closeEvent = event` on the component just opened.
    fn set_close_event(
        &mut self,
        player: &Self::Node,
        id: i32,
        event: Task<Self::Node, Self::Clan>,
    ) -> Result<()>;
    /// `interfaceManager.opened != null`.
    fn opened(&mut self, player: &Self::Node) -> Result<bool>;
    /// `interfaceManager.close()`.
    fn close_manager(&mut self, player: &Self::Node) -> Result<bool>;
    /// `interfaceManager.closeSingleTab()`.
    fn close_single_tab(&mut self, player: &Self::Node) -> Result<bool>;
    /// `interfaceManager.removeTabs(tabs...)`.
    fn remove_tabs(&mut self, player: &Self::Node, tabs: &[i32]) -> Result<()>;
    /// `interfaceManager.openDefaultTabs()`.
    fn open_default_tabs(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.openTab(slot, Component(id))`.
    fn open_tab(&mut self, player: &Self::Node, slot: i32, id: i32) -> Result<()>;
    /// `interfaceManager.openTab(weaponInterface)`.
    fn open_tab_weapon(&mut self, player: &Self::Node, weapon: &Self::Weapon) -> Result<()>;
    /// `interfaceManager.openWindowsPane(Component(id))` / `(Component(id), type)`.
    fn open_windows_pane(&mut self, player: &Self::Node, id: i32, kind: Option<i32>) -> Result<()>;
    /// `interfaceManager.isResizable`.
    fn is_resizable(&mut self, player: &Self::Node) -> Result<bool>;
    /// `super.open(player)` of a Component subclass.
    fn component_open(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `packetDispatch.sendRunScript(id, types, args...)`.
    fn run_script(
        &mut self,
        player: &Self::Node,
        id: i32,
        types: &str,
        args: Vec<ScriptArg>,
    ) -> Result<()>;
    /// `packetDispatch.sendIfaceSettings(hash, child, iface, offset, length)`.
    fn iface_settings(
        &mut self,
        player: &Self::Node,
        hash: i32,
        child: i32,
        iface: i32,
        offset: i32,
        length: i32,
    ) -> Result<()>;
    /// `packetDispatch.sendInterfaceConfig(iface, child, hide)`.
    fn interface_config(
        &mut self,
        player: &Self::Node,
        iface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()>;
    /// `packetDispatch.sendScriptConfigs(id, value, type, params...)`.
    fn script_configs(
        &mut self,
        player: &Self::Node,
        id: i32,
        value: i32,
        kind: &str,
        params: Vec<ScriptArg>,
    ) -> Result<()>;
    /// `InterfaceContainer.generateItems(player, items, options, iface, child, x, y[, key])`.
    #[allow(clippy::too_many_arguments)]
    fn generate_items(
        &mut self,
        player: &Self::Node,
        items: Vec<Option<Self::Node>>,
        options: &[&str],
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
        key: Option<i32>,
    ) -> Result<()>;
    /// `player.updateSceneGraph(login)`.
    fn update_scene_graph(&mut self, player: &Self::Node, login: bool) -> Result<()>;
    /// ContentAPI `log(origin, level, message)`.
    fn log(&mut self, level: LogLevel, text: &str) -> Result<()>;
    /// `player.debug(text)`.
    fn debug(&mut self, player: &Self::Node, text: &str) -> Result<()>;

    // ---------------------------------------------------------- tasks and pulses
    /// `object : MovementPulse(player, target, FOLLOW_ENTITY) { ... }`.
    fn movement_pulse(
        &mut self,
        mover: &Self::Node,
        target: Option<&Self::Node>,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<Self::Pulse>;
    /// `object : Pulse() { ... }` (no event: construction only).
    fn new_pulse(&mut self, body: Task<Self::Node, Self::Clan>) -> Result<Self::Pulse>;
    /// `player.pulseManager.run(pulse[, type])`.
    fn run_pulse(
        &mut self,
        player: &Self::Node,
        pulse: &Self::Pulse,
        kind: Option<PulseType>,
    ) -> Result<()>;
    /// `Pulse.stop()` (the base body: `running = false`).
    fn base_stop(&mut self, pulse: &Self::Pulse) -> Result<()>;
    /// `MovementPulse.stop()`'s `super.stop()` (the base body).
    fn movement_stop(&mut self, pulse: &Self::Pulse) -> Result<()>;
    /// `GameWorld.Pulser.submit(object : Pulse(delay, checks...) { ... })`.
    fn submit(
        &mut self,
        delay: i32,
        checks: &[&Self::Node],
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()>;
    /// ContentAPI `runTask(entity, delay, repeatTimes) { ... }`.
    fn run_task(
        &mut self,
        entity: &Self::Node,
        delay: i32,
        repeat: i32,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()>;
    /// ContentAPI `runWorldTask { ... }`.
    fn run_world_task(&mut self, body: Task<Self::Node, Self::Clan>) -> Result<()>;
    /// ContentAPI `sendInputDialogue(player, numeric, prompt) { ... }`.
    fn input_dialogue(
        &mut self,
        player: &Self::Node,
        numeric: bool,
        prompt: &str,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()>;

    // ---------------------------------------------------------- entity actions
    /// `player.attack(node)`.
    fn attack(&mut self, player: &Self::Node, node: Option<&Self::Node>) -> Result<()>;
    /// `entity.face(target)`.
    fn face(&mut self, entity: &Self::Node, target: Option<&Self::Node>) -> Result<()>;
    /// `entity.faceLocation(location)`.
    fn face_location(&mut self, entity: &Self::Node, location: Option<[i32; 3]>) -> Result<()>;
    /// `entity.inCombat()`.
    fn in_combat(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `entity.lock()`.
    fn lock_all(&mut self, entity: &Self::Node) -> Result<()>;
    /// `entity.lock(ticks)`.
    fn lock_ticks(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `entity.unlock()`.
    fn unlock(&mut self, entity: &Self::Node) -> Result<()>;
    /// `entity.animate(Animation)` (null for none).
    fn animate_entity(&mut self, entity: &Self::Node, animation: Option<i32>) -> Result<()>;
    /// ContentAPI `animate(entity, anim, forced)`.
    fn api_animate(&mut self, entity: &Self::Node, animation: i32, forced: bool) -> Result<()>;
    /// `entity.visualize(Animation, Graphics)`.
    fn visualize(&mut self, entity: &Self::Node, animation: i32, graphics: i32) -> Result<()>;
    /// `player.locks.lock(key, ticks)`.
    fn lock_key(&mut self, player: &Self::Node, key: &str, ticks: i32) -> Result<()>;
    /// `player.locks.isLocked(key)`.
    fn is_locked(&mut self, player: &Self::Node, key: &str) -> Result<bool>;
    /// `player.locks.isInteractionLocked`.
    fn interaction_locked(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.locks.isMovementLocked`.
    fn movement_locked(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.teleporter.send(location, type)`.
    fn teleport(
        &mut self,
        player: &Self::Node,
        location: Option<[i32; 3]>,
        kind: TeleportType,
    ) -> Result<bool>;
    /// ContentAPI `playAudio(player, id, delay, loops)`.
    fn play_audio(&mut self, player: &Self::Node, id: i32, delay: i32, loops: i32) -> Result<()>;

    // ---------------------------------------------------------- player state
    /// `player.isArtificial()`.
    fn is_artificial(&mut self, player: &Self::Node) -> Result<bool>;
    /// `SystemManager.getSystemConfig().validLogin(player)`.
    fn valid_login(&mut self, player: &Self::Node) -> Result<bool>;
    /// `GameWorld.getSettings().isDevMode()`.
    fn dev_mode(&mut self) -> Result<bool>;
    /// `player.toggleDebug()`.
    fn toggle_debug(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.isAdmin`.
    fn is_admin(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.username`.
    fn username(&mut self, player: &Self::Node) -> Result<String>;
    /// `player.details.rights`.
    fn rights(&mut self, player: &Self::Node) -> Result<crate::player::details::Rights>;
    /// `player.isTeleporting`.
    fn is_teleporting(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.zoneMonitor.canLogout()`.
    fn can_logout(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.zoneMonitor.type`.
    fn zone_type(&mut self, player: &Self::Node) -> Result<i32>;
    /// `Repository.disconnectionQueue.add(player)`.
    fn disconnect(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.settings.toggleX()`.
    fn toggle(&mut self, player: &Self::Node, toggle: Toggle) -> Result<()>;
    /// `player.settings.isSpecialToggled`.
    fn special_toggled(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.ironmanManager.checkRestriction(mode)`.
    fn check_restriction(&mut self, player: &Self::Node, mode: IronmanMode) -> Result<bool>;
    /// `player.skullManager.isSkulled`.
    fn is_skulled(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.prayer[type]`.
    fn prayer_active(&mut self, player: &Self::Node, prayer: PrayerType) -> Result<bool>;
    /// `PrayerType.get(button)`.
    fn prayer_type(&mut self, button: i32) -> Result<Option<PrayerType>>;
    /// `player.prayer.toggle(type)`.
    fn prayer_toggle(&mut self, player: &Self::Node, prayer: PrayerType) -> Result<bool>;
    /// `player.familiarManager.hasFamiliar()`.
    fn has_familiar(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.familiarManager.familiar.isBurdenBeast`.
    fn is_burden_beast(&mut self, player: &Self::Node) -> Result<bool>;
    /// `(familiar as BurdenBeast).container` (a failed cast is `Cast`).
    fn beast_container(&mut self, player: &Self::Node) -> Result<Self::Container>;
    /// `player.timers.getTimer(id) != null`.
    fn has_timer(&mut self, player: &Self::Node, id: &str) -> Result<bool>;
    /// `player.savedData.questData.getDragonSlayerAttribute(name)`.
    fn dragon_slayer_attribute(&mut self, player: &Self::Node, name: &str) -> Result<bool>;
    /// `globalData.getLootShareDelay()`.
    fn loot_share_delay(&mut self, player: &Self::Node) -> Result<i64>;
    /// `globalData.setLootShareDelay(value)`.
    fn set_loot_share_delay(&mut self, player: &Self::Node, value: i64) -> Result<()>;
    /// `globalData.getLootSharePoints()`.
    fn loot_share_points(&mut self, player: &Self::Node) -> Result<i32>;
    /// `globalData.setLootSharePoints(value)`.
    fn set_loot_share_points(&mut self, player: &Self::Node, value: i32) -> Result<()>;
    /// `globalData.setDisableDeathScreen(value)`.
    fn set_disable_death_screen(&mut self, player: &Self::Node, value: bool) -> Result<()>;
    /// `ActivityManager.start(player, name, login)`.
    fn activity_start(&mut self, player: &Self::Node, name: &str, login: bool) -> Result<bool>;
    /// ContentAPI `setVarp(player, index, value, save)`.
    fn set_varp(&mut self, player: &Self::Node, index: i32, value: i32, save: bool) -> Result<()>;
    /// ContentAPI `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &Self::Node, id: i32, value: i32, save: bool) -> Result<()>;
    /// `LevelUp.sendFlashingIcons(player, skill)`.
    fn flashing_icons(&mut self, player: &Self::Node, skill: i32) -> Result<()>;
    /// `Emotes.handle(player, button)`.
    fn emote(&mut self, player: &Self::Node, button: i32) -> Result<()>;
    /// `CharacterDesign.handleButtons(player, button)`.
    fn character_design(&mut self, player: &Self::Node, button: i32) -> Result<()>;
    /// `LoginConfiguration.configureGameWorld(player)`.
    fn configure_game_world(&mut self, player: &Self::Node) -> Result<()>;
    /// `RandomFunction.random(a, b)`.
    fn random(&mut self, low: i32, high: i32) -> Result<i32>;

    // ---------------------------------------------------------- requests, assist
    /// `player.requestManager.request(target, type)`.
    fn request(
        &mut self,
        player: &Self::Node,
        target: &Self::Node,
        kind: crate::world::trade::RequestType,
    ) -> Result<bool>;
    /// `AIRepository.sendBotInfo(player, bot)`.
    fn bot_info(&mut self, player: &Self::Node, bot: &Self::Node) -> Result<()>;
    /// `AssistSession.getExtension(player)`.
    fn assist_session(&mut self, player: &Self::Node) -> Result<Option<Self::Session>>;
    /// `session.getPlayer()`.
    fn assist_player(&mut self, session: &Self::Session) -> Result<Option<Self::Node>>;
    /// `session.toggleButton((byte) index)`.
    fn assist_toggle(&mut self, session: &Self::Session, index: i32) -> Result<()>;
    /// `session.refresh()`.
    fn assist_refresh(&mut self, session: &Self::Session) -> Result<()>;

    // ---------------------------------------------------------- items and containers
    /// `player.inventory` (a field read).
    fn inventory(&mut self, player: &Self::Node) -> Result<Self::Container>;
    /// `player.equipment` (a field read).
    fn equipment(&mut self, player: &Self::Node) -> Result<Self::Container>;
    /// `DeathTask.getContainers(player)`: kept, lost.
    fn death_containers(&mut self, player: &Self::Node) -> Result<[Self::Container; 2]>;
    /// `container.get(slot)` (null outside the array).
    fn get(&mut self, container: &Self::Container, slot: i32) -> Result<Option<Self::Node>>;
    /// `container.getNew(slot).getId()`.
    fn get_new_id(&mut self, container: &Self::Container, slot: i32) -> Result<i32>;
    /// `container.itemCount()`.
    fn item_count(&mut self, container: &Self::Container) -> Result<i32>;
    /// `container.getId(slot)`.
    fn container_id(&mut self, container: &Self::Container, slot: i32) -> Result<i32>;
    /// `container.isEmpty`.
    fn is_empty(&mut self, container: &Self::Container) -> Result<bool>;
    /// `container.toArray()`.
    fn to_array(&mut self, container: &Self::Container) -> Result<Vec<Option<Self::Node>>>;
    /// `container.getAmount(item)`.
    fn get_amount(&mut self, container: &Self::Container, item: &Self::Node) -> Result<i32>;
    /// `container.refresh()`.
    fn refresh(&mut self, container: &Self::Container) -> Result<()>;
    /// `container.containItems(ids...)`.
    fn contain_items(&mut self, container: &Self::Container, ids: &[i32]) -> Result<bool>;
    /// `container.add(Item(id, amount))`.
    fn add(&mut self, container: &Self::Container, id: i32, amount: i32) -> Result<bool>;
    /// `EquipmentContainer.update(player)`.
    fn equipment_update(&mut self, player: &Self::Node) -> Result<()>;
    /// `EquipHandler.unequip(player, slot, itemId)`.
    fn unequip(&mut self, player: &Self::Node, slot: i32, item: i32) -> Result<()>;
    /// `InteractionListeners.run(id, type, option, player, node)`.
    fn listeners(
        &mut self,
        id: i32,
        kind: crate::world::interaction_listeners::Kind,
        option: &str,
        player: &Self::Node,
        node: &Self::Node,
    ) -> Result<bool>;
    /// `item.operateHandler`.
    fn operate_handler(&mut self, item: &Self::Node) -> Result<Option<Self::Operate>>;
    /// `handler.handle(player, item, option)`.
    fn operate_handle(
        &mut self,
        handler: &Self::Operate,
        player: &Self::Node,
        item: &Self::Node,
        option: &str,
    ) -> Result<bool>;
    /// `item.definition.examine`.
    fn examine(&mut self, item: &Self::Node) -> Result<Option<String>>;
    /// `item.definition.options` (null when the definition has none).
    fn inventory_options(&mut self, item: &Self::Node) -> Result<Option<Vec<Option<String>>>>;
    /// `item.noteChange`.
    fn note_change(&mut self, item: &Self::Node) -> Result<i32>;
    /// `item.definition.isUnnoted`.
    fn is_unnoted(&mut self, item: &Self::Node) -> Result<bool>;
    /// ContentAPI `anyInEquipment(player, ids...)`.
    fn any_in_equipment(&mut self, player: &Self::Node, ids: &[i32]) -> Result<bool>;
    /// ContentAPI `removeItem(player, Item(id, amount), container)`.
    fn remove_item(
        &mut self,
        player: &Self::Node,
        id: i32,
        amount: i32,
        store: Store,
    ) -> Result<bool>;
    /// ContentAPI `addItem(player, id, amount, container)`.
    fn add_item(&mut self, player: &Self::Node, id: i32, amount: i32, store: Store)
    -> Result<bool>;
    /// ContentAPI `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// ContentAPI `freeSlots(player)`.
    fn free_slots(&mut self, player: &Self::Node) -> Result<i32>;
    /// ContentAPI `getItemName(id)`.
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// ContentAPI `hasOption(node, option)`.
    fn has_option(&mut self, node: &Self::Node, option: &str) -> Result<bool>;

    // ---------------------------------------------------------- bank
    /// ContentAPI `openBankAccount`.
    fn open_bank(&mut self, player: &Self::Node) -> Result<()>;
    /// ContentAPI `openGrandExchangeCollectionBox`.
    fn open_collection_box(&mut self, player: &Self::Node) -> Result<()>;
    /// ContentAPI `openDepositBox`.
    fn open_deposit_box(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.bank.addItem(slot, amount)`.
    fn bank_add_item(&mut self, player: &Self::Node, slot: i32, amount: i32) -> Result<()>;
    /// `player.bank.refreshDepositBoxInterface()`.
    fn refresh_deposit_box(&mut self, player: &Self::Node) -> Result<()>;
    /// `BankUtils.transferX(player, slot, withdraw, after)`
    /// (`world::input_prompt::transfer_x`).
    fn transfer_x(
        &mut self,
        player: &Self::Node,
        slot: i32,
        withdraw: bool,
        after: Option<Task<Self::Node, Self::Clan>>,
    ) -> Result<()>;
    /// ContentAPI `dumpBeastOfBurden`.
    fn dump_beast_of_burden(&mut self, player: &Self::Node) -> Result<()>;
    /// `BankerNPC.checkLunarIsleRestriction(player, node)`.
    fn lunar_restriction(&mut self, player: &Self::Node, node: &Self::Node) -> Result<bool>;
    /// `Repository.findNPC(location)`.
    fn find_npc(&mut self, location: [i32; 3]) -> Result<Option<Self::Node>>;

    // ---------------------------------------------------------- combat, magic
    /// `player.getExtension(WeaponInterface.class)`.
    fn weapon_interface(&mut self, player: &Self::Node) -> Result<Option<Extension<Self::Weapon>>>;
    /// `weapon.isSpecialBar`.
    fn is_special_bar(&mut self, weapon: &Self::Weapon) -> Result<bool>;
    /// `weapon.setAttackStyle(button)`.
    fn set_attack_style(&mut self, weapon: &Self::Weapon, button: i32) -> Result<bool>;
    /// `weapon.openAutocastSelect()`.
    fn open_autocast_select(&mut self, weapon: &Self::Weapon) -> Result<()>;
    /// `weapon.selectAutoSpell(button, adjust, forget)`.
    fn select_auto_spell(
        &mut self,
        weapon: &Self::Weapon,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> Result<()>;
    /// `weapon.updateInterface()`.
    fn update_weapon_interface(&mut self, weapon: &Self::Weapon) -> Result<()>;
    /// `player.properties.autocastSpell != null`.
    fn has_autocast_spell(&mut self, player: &Self::Node) -> Result<bool>;
    /// `CombatStyle.MELEE.swingHandler.getSpecial(itemId)`.
    fn special_handler(&mut self, item: i32) -> Result<Option<Self::Special>>;
    /// `(handler as Plugin).fireEvent("instant_spec", player) == Boolean.TRUE`
    /// (`Cast` when the handler is no plugin).
    fn special_instant(&mut self, handler: &Self::Special, player: &Self::Node) -> Result<bool>;
    /// `player.properties.combatPulse.getVictim()`.
    fn combat_victim(&mut self, player: &Self::Node) -> Result<Option<Self::Node>>;
    /// `handler.swing(player, victim, null)`.
    fn special_swing(
        &mut self,
        handler: &Self::Special,
        player: &Self::Node,
        victim: Option<&Self::Node>,
    ) -> Result<()>;
    /// `SpellListeners.run(button, type, book, player[, node])`.
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        player: &Self::Node,
        node: Option<&Self::Node>,
    ) -> Result<()>;
    /// `MagicSpell.castSpell(player, book, spell, target)`.
    fn cast_spell(
        &mut self,
        player: &Self::Node,
        book: SpellBook,
        spell: i32,
        target: &Self::Node,
    ) -> Result<bool>;
    /// `player.properties.attackAnimation`.
    fn attack_animation(&mut self, player: &Self::Node) -> Result<i32>;
    /// `player.properties.currentCombatLevel`.
    fn combat_level(&mut self, player: &Self::Node) -> Result<i32>;
    /// `player.properties.attackStyle.style`.
    fn attack_style(&mut self, player: &Self::Node) -> Result<i32>;
    /// `player.skills.addExperience(skill, experience)`.
    fn add_experience(&mut self, player: &Self::Node, skill: i32, experience: f64) -> Result<()>;

    // ---------------------------------------------------------- quests, diaries, music, clans
    /// `player.achievementDiaryManager.openTab()`.
    fn diary_open_tab(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.questRepository.forButtonId(button)`.
    fn quest_for_button(&mut self, player: &Self::Node, button: i32)
    -> Result<Option<Self::Quest>>;
    /// `quest.getStage(player)`.
    fn quest_stage(&mut self, quest: &Self::Quest, player: &Self::Node) -> Result<i32>;
    /// `quest.drawJournal(player, stage)`.
    fn draw_journal(&mut self, quest: &Self::Quest, player: &Self::Node, stage: i32) -> Result<()>;
    /// `QuestTabUtils.showRequirementsInterface(player, button)` (content:
    /// the quest requirement tables are not ported).
    fn quest_requirements(&mut self, player: &Self::Node, button: i32) -> Result<()>;
    /// `player.questRepository.syncronizeTab(player)`.
    fn quest_sync_tab(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.questRepository.isComplete(quest)`.
    fn quest_complete(&mut self, player: &Self::Node, quest: Quest) -> Result<bool>;
    /// `player.questRepository.hasCompletedAll()`.
    fn has_completed_all(&mut self, player: &Self::Node) -> Result<bool>;
    /// `DiaryType.forChild(child)`: an index into
    /// `world::player_managers::diary_data::TYPES`.
    fn diary_for_child(&mut self, child: i32) -> Result<Option<usize>>;
    /// `player.achievementDiaryManager.getDiary(type)`.
    fn get_diary(
        &mut self,
        player: &Self::Node,
        kind: Option<usize>,
    ) -> Result<Option<Self::Diary>>;
    /// `diary.open(player)`.
    fn diary_open(&mut self, diary: &Self::Diary, player: &Self::Node) -> Result<()>;
    /// `player.musicPlayer.toggleLooping()`.
    fn toggle_looping(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.musicPlayer.unlocked[slot]`.
    fn unlocked(&mut self, player: &Self::Node, slot: i32) -> Result<Option<Song>>;
    /// `player.musicPlayer.play(entry)`.
    fn play(&mut self, player: &Self::Node, song: Option<Song>) -> Result<()>;
    /// `MusicEntry.getSongs().values` in the map's iteration order.
    fn songs(&mut self) -> Result<Vec<Song>>;
    /// `player.musicPlayer.unlock(id)`.
    fn unlock_song(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `ClanRepository.openSettings(player)`.
    fn clan_open_settings(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.communication.toggleLootshare(player)`.
    fn toggle_lootshare(&mut self, player: &Self::Node) -> Result<()>;
    /// `ClanRepository.get(owner, create)`.
    fn clan_get(&mut self, owner: Option<&str>, create: bool) -> Result<Option<Self::Clan>>;
    /// `clan.name`.
    fn clan_name(&mut self, clan: &Self::Clan) -> Result<Option<String>>;
    /// `clan.name = name`.
    fn clan_set_name(&mut self, clan: &Self::Clan, name: &str) -> Result<()>;
    /// `clan.xRequirement`.
    fn clan_rank(&mut self, clan: &Self::Clan, field: ClanField) -> Result<Option<ClanRank>>;
    /// `clan.xRequirement = rank`.
    fn clan_set_rank(
        &mut self,
        clan: &Self::Clan,
        field: ClanField,
        rank: Option<ClanRank>,
    ) -> Result<()>;
    /// `clan.updateSettings(player)`.
    fn clan_update_settings(&mut self, clan: &Self::Clan, player: &Self::Node) -> Result<()>;
    /// `clan.update()`.
    fn clan_update(&mut self, clan: &Self::Clan) -> Result<()>;
    /// `clan.delete()`.
    fn clan_delete(&mut self, clan: &Self::Clan) -> Result<()>;
    /// `player.communication.clanName`.
    fn comm_clan_name(&mut self, player: &Self::Node) -> Result<Option<String>>;
    /// `player.communication.clanName = name`.
    fn comm_set_clan_name(&mut self, player: &Self::Node, name: &str) -> Result<()>;
    /// `player.communication.xRequirement = rank`.
    fn comm_set_rank(
        &mut self,
        player: &Self::Node,
        field: ClanField,
        rank: Option<ClanRank>,
    ) -> Result<()>;
    /// `WorldCommunicator.isEnabled()`.
    fn world_communicator_enabled(&mut self) -> Result<bool>;
    /// `MSPacketRepository.sendClanRename(player, name)`.
    fn ms_clan_rename(&mut self, player: &Self::Node, name: Option<&str>) -> Result<()>;
    /// `MSPacketRepository.setClanSetting(player, type, rank)`.
    fn ms_clan_setting(
        &mut self,
        player: &Self::Node,
        kind: i32,
        rank: Option<ClanRank>,
    ) -> Result<()>;

    // ---------------------------------------------------------- scenery, ladders
    /// `SpecialLadders.getDestination(location)` (`climb::Ladders`).
    fn special_destination(&mut self, at: [i32; 3]) -> Result<Option<[i32; 3]>>;
    /// `SpecialLadders.getSpecialLadder(location)` (`climb::Ladders`).
    fn special_ladder(&mut self, at: [i32; 3]) -> Result<Option<&'static climb::SpecialLadder>>;
    /// `scenery.definition.sizeX`, `sizeY`.
    fn scenery_size(&mut self, scenery: &Self::Node) -> Result<[i32; 2]>;
    /// `scenery.rotation`.
    fn scenery_rotation(&mut self, scenery: &Self::Node) -> Result<i32>;
    /// `scenery.definition.walkingFlag`.
    fn scenery_walking_flag(&mut self, scenery: &Self::Node) -> Result<i32>;
    /// `scenery.definition.options`.
    fn scenery_options(&mut self, scenery: &Self::Node) -> Result<Option<Vec<Option<String>>>>;
    /// `RegionManager.getObject(location)`.
    fn get_object(&mut self, at: [i32; 3]) -> Result<Option<Self::Node>>;
    /// `RegionManager.isTeleportPermitted(location)`.
    fn teleport_permitted(&mut self, at: [i32; 3]) -> Result<bool>;
    /// `RegionManager.getClippingFlag(z, x, y)`.
    fn clipping_flag(&mut self, at: [i32; 3]) -> Result<i32>;
    /// `player.properties.teleportLocation = location`.
    fn set_teleport_location(&mut self, player: &Self::Node, at: [i32; 3]) -> Result<()>;
    /// `player.achievementDiaryManager.finishTask(player, type, level, task)`.
    fn finish_task(
        &mut self,
        player: &Self::Node,
        kind: usize,
        level: i32,
        task: i32,
    ) -> Result<()>;
    /// `ZogreFleshEatersListeners.ladderMakesSithikTurnIntoOgre(player)` (content).
    fn zogre_ladder(&mut self, player: &Self::Node) -> Result<()>;
    /// `interpreter.sendOptions(title, options...)`.
    fn send_options(&mut self, player: &Self::Node, title: &str, options: &[&str]) -> Result<()>;
    /// `player.dialogueInterpreter.setDialogue(ClimbDialogue)`.
    fn set_dialogue(
        &mut self,
        player: &Self::Node,
        dialogue: climb::ClimbDialogue<Self::Node>,
    ) -> Result<()>;
    /// `DialoguePlugin.end()`.
    fn dialogue_end(&mut self, player: &Self::Node) -> Result<()>;
}

/// A string template's text for a nullable string.
pub(crate) fn or_null(text: Option<String>) -> String {
    text.unwrap_or_else(|| "null".into())
}

/// `IfaceSettingsBuilder().enableAllOptions().build()`: options 0 to 9.
pub const ALL_OPTIONS: i32 = {
    let mut value = 0;
    let mut i = 0;
    while i <= 9 {
        value |= 1 << (i + 1);
        i += 1;
    }
    value
};

/// Runs a deferred body: a pulse's `pulse()`, a task, an input callback (with
/// its value) or a close event (with the component's id).
pub fn run_task<H: Host + ?Sized>(
    h: &mut H,
    task: &Task<H::Node, H::Clan>,
    player: &H::Node,
    input: Option<&Input>,
) -> Result<Outcome> {
    match task {
        Task::Follow { player, target } => {
            player::follow_pulse(h, player, target.as_ref()).map(Outcome::Bool)
        }
        Task::Retaliate { player } => tabs::retaliate_pulse(h, player).map(Outcome::Bool),
        Task::SpecialBar { player } => tabs::special_bar_pulse(h, player).map(Outcome::Bool),
        Task::ItemSelect { .. } => Ok(Outcome::Bool(false)),
        Task::ConfigureWorld { player } => {
            h.configure_game_world(player)?;
            Ok(Outcome::Unit)
        }
        Task::Rules { player } => {
            ifaces::rules_task(h, player)?;
            Ok(Outcome::Unit)
        }
        Task::Deposit {
            player,
            item,
            opcode,
            slot,
        } => {
            ifaces::deposit_task(h, player, item, *opcode, *slot)?;
            Ok(Outcome::Unit)
        }
        Task::RefreshDepositBox { player } => {
            h.refresh_deposit_box(player)?;
            Ok(Outcome::Unit)
        }
        Task::ClanPrefix { player, clan } => {
            tabs::clan_prefix(h, player, clan, input.ok_or(Error::Null)?)?;
            Ok(Outcome::Unit)
        }
        Task::ReportClose => ifaces::report_close(h, player).map(Outcome::Bool),
        Task::DepositBoxClose => {
            // CloseEvent { p, _ -> p.interfaceManager.openDefaultTabs(); true }
            h.open_default_tabs(player)?;
            Ok(Outcome::Bool(true))
        }
        Task::Climb {
            player,
            destination,
            messages,
        } => climb::climb_pulse(h, player, *destination, messages).map(Outcome::Bool),
        Task::ClimbChoice {
            player,
            object,
            option,
        } => {
            climb::climb_ladder(h, player, object.as_ref(), Some(option))?;
            Ok(Outcome::Bool(true))
        }
    }
}

/// A pulse's `stop()` override, if the body has one (the base stop runs
/// otherwise).
pub fn stop_task<H: Host + ?Sized>(
    h: &mut H,
    task: &Task<H::Node, H::Clan>,
    pulse: &H::Pulse,
) -> Result<()> {
    match task {
        Task::Follow { player, .. } => player::follow_stop(h, player, pulse),
        Task::ItemSelect { player } => {
            // super.stop(); player.interfaceManager.closeSingleTab()
            h.base_stop(pulse)?;
            h.close_single_tab(player)?;
            Ok(())
        }
        _ => Ok(()),
    }
}
