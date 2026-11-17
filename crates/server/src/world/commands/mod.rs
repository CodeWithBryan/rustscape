//! The command system: `CommandSystem.parse`, `Command.attemptHandling`,
//! `CommandMapping` (the registry, a Java `HashMap` walked in its own
//! order), the legacy `CommandSet` enum with its linked `CommandPlugin`s,
//! `CommandPlugin`'s helpers, `sets.CommandSet`'s `reject`/`notify`, the
//! `Command` branch of `PacketProcessor.process` and the `COMMAND` decoder of
//! `Decoders530`; then the command sets an operator needs.
//!
//! | Original | Here |
//! | --- | --- |
//! | `CommandSystem.parse`, `CommandSet.interpret` | [`parse`], [`interpret`] |
//! | `Command.attemptHandling` | [`attempt_handling`] |
//! | `CommandMapping` | [`Registry`] |
//! | the class scan's `defineCommands`/`newInstance` calls | [`registration`] |
//! | `ModerationCommandSet` | [`moderation`] |
//! | `SystemCommandSet` | [`system`] |
//! | `TeleportCommandSet` | [`teleport`] |
//! | `PlayerAttributeCommandSet` | [`attributes`] |
//! | `MiscCommandSet`, `SpawnCommandSet`, `StatsCommandSet` | [`sets`] |
//! | the pulses, tasks, input handlers and coroutine they submit | [`deferred`] |
//!
//! The registry holds typed bodies: a [`NativeBody`] is run by this module,
//! a [`Body::Content`] (every command of a set or content class that is not
//! ported) is the required service [`Host::content_command`]. Dispatch goes
//! through the registry exactly as the original's does; no command name is
//! matched outside it. Every call into another system is a required method
//! of [`Host`] with no default; the account and authenticator calls join
//! `crate::auth` through [`accounts`]. See docs/commands.md.
pub mod accounts;
pub mod attributes;
pub mod deferred;
pub mod kotlin;
pub mod moderation;
pub mod registration;
pub mod sets;
pub mod system;
pub mod teleport;

use crate::auth::account::Account;
use crate::player::details::Rights;
use crate::world::repository::hash_map::JavaHashMap;
use deferred::{InputHandler, Job, Pulse, Task};

/// What stops a command, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `IllegalStateException` (`reject` throws one; `CommandSystem.parse`
    /// catches only this class).
    IllegalState(Option<String>),
    /// `NullPointerException`, Kotlin's null checks included.
    Null,
    /// `ArrayIndexOutOfBoundsException`/`IndexOutOfBoundsException` with
    /// the JDK's message.
    Index(String),
    /// `NumberFormatException` with its message.
    NumberFormat(String),
    /// `ClassCastException`.
    Cast,
    /// A service failed with what it threw.
    Binding(Option<String>),
}
impl Error {
    /// `Throwable.toString()`.
    pub fn java_string(&self) -> String {
        match self {
            Self::IllegalState(None) => "java.lang.IllegalStateException".into(),
            Self::IllegalState(Some(m)) => format!("java.lang.IllegalStateException: {m}"),
            Self::Null => "java.lang.NullPointerException".into(),
            Self::Index(m) => format!("java.lang.IndexOutOfBoundsException: {m}"),
            Self::NumberFormat(m) => format!("java.lang.NumberFormatException: {m}"),
            Self::Cast => "java.lang.ClassCastException".into(),
            Self::Binding(m) => m.clone().unwrap_or_else(|| "service failure".into()),
        }
    }
    /// The categories the oracle records.
    pub fn category(&self) -> &'static str {
        match self {
            Self::IllegalState(_) => "IllegalState",
            Self::Null => "Null",
            Self::Index(_) => "Index",
            Self::NumberFormat(_) => "NumberFormat",
            Self::Cast => "Cast",
            Self::Binding(_) => "Binding",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.java_string())
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// `Array<String>[index]`.
pub(crate) fn arg(args: &[String], index: usize) -> Result<&str> {
    args.get(index).map(String::as_str).ok_or_else(|| {
        Error::Index(format!(
            "Index {index} out of bounds for length {}",
            args.len()
        ))
    })
}

/// `List<String>[index]`.
pub(crate) fn arg_list<'a>(items: &[&'a str], index: usize) -> Result<&'a str> {
    items.get(index).copied().ok_or_else(|| {
        Error::Index(format!(
            "Index {index} out of bounds for length {}",
            items.len()
        ))
    })
}

/// `core.game.system.command.Privilege`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Privilege {
    Standard,
    Moderator,
    Admin,
}
impl Privilege {
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::Moderator => "MODERATOR",
            Self::Admin => "ADMIN",
        }
    }
}

/// `Location`: x, y, plane.
pub type Loc = [i32; 3];

/// `TeleportManager.TeleportType` values the commands pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportType {
    Normal,
}

/// `core.api.InputType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputType {
    Amount,
    Numeric,
    StringShort,
    StringLong,
    Message,
}
impl InputType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Amount => "AMOUNT",
            Self::Numeric => "NUMERIC",
            Self::StringShort => "STRING_SHORT",
            Self::StringLong => "STRING_LONG",
            Self::Message => "MESSAGE",
        }
    }
}

/// An attribute value as these commands store and print them.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    Bool(bool),
    Text(String),
    /// Any other object, by the text its `toString()` gives.
    Other(String),
}
impl Value {
    /// `String.valueOf(value)` / string templates.
    pub fn java_string(&self) -> String {
        match self {
            Self::Int(v) => v.to_string(),
            Self::Float(v) => kotlin::float_to_string(*v),
            Self::Bool(v) => v.to_string(),
            Self::Text(v) | Self::Other(v) => v.clone(),
        }
    }
}

/// A player's container the commands reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Container {
    Inventory,
    Bank,
    Equipment,
    BankPrimary,
    BankSecondary,
}
impl Container {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Bank => "bank",
            Self::Equipment => "equipment",
            Self::BankPrimary => "bankPrimary",
            Self::BankSecondary => "bankSecondary",
        }
    }
}

/// An `Item` in a container slot, as `Container.toArray()`/`get` give it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRef {
    pub id: i32,
    pub slot: i32,
    /// `item.getCharge()` as read from the item in the slot.
    pub charge: i32,
}

/// The legacy `core.game.system.command.CommandSet` constants, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LegacySet {
    Player,
    Moderator,
    Administrator,
    Developer,
    Beta,
}
impl LegacySet {
    pub const ALL: [Self; 5] = [
        Self::Player,
        Self::Moderator,
        Self::Administrator,
        Self::Developer,
        Self::Beta,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Player => "PLAYER",
            Self::Moderator => "MODERATOR",
            Self::Administrator => "ADMINISTRATOR",
            Self::Developer => "DEVELOPER",
            Self::Beta => "BETA",
        }
    }
}

/// A legacy `CommandPlugin` linked to a [`LegacySet`]: its class name. Its
/// `parse` is content, reached through [`Host::legacy_parse`].
pub type LegacyPlugin = &'static str;

/// A command body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Body {
    /// A body this module ports.
    Native(NativeBody),
    /// A body that is not ported: the defining class and its lambda's
    /// position in `defineCommands`, run by [`Host::content_command`].
    Content(ContentBody),
}

/// A not-ported body: `class$defineCommands$lambda`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentBody {
    pub class: &'static str,
    pub lambda: u16,
}

/// The ported command bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeBody {
    Kick,
    Ban,
    IpBan,
    Mute,
    Jail,
    ModCr,
    CsvModCr,
    ModerationGetAttribute,
    Update,
    CancelUpdate,
    ResetPassword,
    SetPasswordOther,
    GiveItem,
    RemoveItem,
    RemoveItemAll,
    Potato,
    Shutdown,
    Charge,
    To,
    Tele,
    TeleTo,
    TeleToMe,
    Home,
    GetAttribute,
    SetAttribute,
    /// A body of the Misc, Spawn or Stats set ([`sets`]).
    Set(sets::SetBody),
}

/// `core.game.system.command.Command`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub privilege: Privilege,
    pub usage: String,
    pub description: String,
    pub body: Body,
    /// Where it was defined (`class$defineCommands$lambda`): records only,
    /// never used to dispatch.
    pub origin: ContentBody,
}

/// `CommandMapping`: the name map, plus the legacy enum's plugin lists.
#[derive(Default)]
pub struct Registry {
    mapping: JavaHashMap<Option<String>, Command>,
    legacy: Vec<(LegacySet, Vec<LegacyPlugin>)>,
}
impl Registry {
    pub fn new() -> Self {
        Self {
            mapping: JavaHashMap::new(),
            legacy: LegacySet::ALL.iter().map(|s| (*s, Vec::new())).collect(),
        }
    }
    /// `CommandMapping.get(name)`.
    pub fn get(&self, name: &str) -> Option<&Command> {
        self.mapping.get(&Some(name.to_owned()))
    }
    /// `CommandMapping.register(command)`: the last registration of a name
    /// wins and keeps the first one's place.
    pub fn register(&mut self, command: Command) {
        self.mapping.put(Some(command.name.clone()), command);
    }
    /// `CommandMapping.getNames()`, in the map's walk order.
    pub fn names(&self) -> Vec<String> {
        self.mapping
            .iter()
            .map(|(k, _)| k.clone().unwrap_or_default())
            .collect()
    }
    /// `CommandMapping.getCommands()`, in the map's walk order.
    pub fn commands(&self) -> Vec<&Command> {
        self.mapping.iter().map(|(_, v)| v).collect()
    }
    pub fn len(&self) -> usize {
        self.mapping.len() as usize
    }
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }
    /// `CommandPlugin.link(set)`: appended to the set's list.
    pub fn link(&mut self, set: LegacySet, plugin: LegacyPlugin) {
        if let Some((_, list)) = self.legacy.iter_mut().find(|(s, _)| *s == set) {
            list.push(plugin);
        }
    }
    /// `CommandSet.getPlugins()`.
    pub fn plugins(&self, set: LegacySet) -> &[LegacyPlugin] {
        self.legacy
            .iter()
            .find(|(s, _)| *s == set)
            .map_or(&[], |(_, l)| l.as_slice())
    }
    /// `CommandMapping.getPageIndices(rights)`.
    pub fn page_indices(&self, rights: i32) -> Vec<i32> {
        let mut list = vec![0];
        let mut line_counter = 0;
        for (index, command) in self
            .commands()
            .into_iter()
            .filter(|c| c.privilege.ordinal() <= rights)
            .enumerate()
        {
            line_counter += 2;
            if !command.usage.is_empty() {
                line_counter += 1;
            }
            if !command.description.is_empty() {
                line_counter += 1;
            }
            if line_counter > 306 {
                list.push(index as i32);
                line_counter = 0;
            }
        }
        list
    }
}

/// What the commands need from the world. There are no defaults: methods
/// without a `Result` are field or constant reads of the original; every
/// other method is a call into a system this module does not port and may
/// fail with what that system threw.
pub trait Host {
    /// A player; `==` is identity.
    type Player: Clone + PartialEq + std::fmt::Debug;

    // Constants and settings.
    /// `ServerConstants.PLAYER_COMMANDS`.
    fn player_commands(&mut self) -> bool;
    /// `ServerConstants.I_AM_A_CHEATER`.
    fn i_am_a_cheater(&mut self) -> bool;
    /// `GameWorld.settings`: `None` when it is null, else `isDevMode`.
    fn dev_mode(&mut self) -> Option<bool>;
    /// `GameWorld.getSettings().isBeta()`; `None` when settings is null.
    fn beta(&mut self) -> Option<bool>;
    /// `ServerConstants.HOME_LOCATION`.
    fn home_location(&mut self) -> Option<Loc>;
    /// The location `CastleWars.lobbyBankArea.randomLoc` gave the last row
    /// of `ServerConstants.TELEPORT_DESTINATIONS` when the constants were
    /// initialised.
    fn castle_wars_destination(&mut self) -> Loc;
    /// `ServerConstants.DATA_PATH`.
    fn data_path(&mut self) -> Option<String>;
    /// `System.currentTimeMillis()`.
    fn now(&mut self) -> i64;

    // Player fields.
    /// `player.details.accountInfo`: the native account record.
    fn account(&mut self, player: &Self::Player) -> Account;
    /// `player.getUsername()`.
    fn username(&mut self, player: &Self::Player) -> String;
    /// `player.location` (null when absent).
    fn location(&mut self, player: &Self::Player) -> Result<Option<Loc>>;

    // Messages and chat.
    /// `player.sendMessage(text)`.
    fn message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `core.api.sendMessage(player, text)`.
    fn api_message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `core.api.sendChat(player, text)`: `entity.sendChat(text)`.
    fn send_chat(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `core.api.sendDialogue(player, text)`.
    fn dialogue(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `core.api.sendInputDialogue(player, type, prompt, handler)`.
    fn input_dialogue(
        &mut self,
        player: &Self::Player,
        kind: InputType,
        prompt: &str,
        handler: InputHandler<Self::Player>,
    ) -> Result<()>;
    /// `core.api.runTask(player, delay, repeat, task)`.
    fn run_task(
        &mut self,
        player: &Self::Player,
        delay: i32,
        repeat: i32,
        task: Task<Self::Player>,
    ) -> Result<()>;

    // The command system's outside calls.
    /// `PlayerMonitor.log(player, LogType.COMMAND, line)`.
    fn monitor_log(&mut self, player: &Self::Player, line: &str) -> Result<()>;
    /// `player.getZoneMonitor().parseCommand(player, name, arguments)`.
    fn zone_parse_command(
        &mut self,
        player: &Self::Player,
        name: &str,
        arguments: &[String],
    ) -> Result<bool>;
    /// `plugin.parse(player, name, arguments)` of a legacy `CommandPlugin`.
    fn legacy_parse(
        &mut self,
        plugin: LegacyPlugin,
        player: &Self::Player,
        name: &str,
        arguments: &[String],
    ) -> Result<bool>;
    /// A not-ported command body.
    fn content_command(
        &mut self,
        body: ContentBody,
        player: &Self::Player,
        args: &[String],
    ) -> Result<()>;

    // The repository and the player's own systems.
    /// `Repository.getPlayerByName(name)`.
    fn player_by_name(&mut self, name: Option<&str>) -> Result<Option<Self::Player>>;
    /// `player.clear()`.
    fn clear(&mut self, player: &Self::Player) -> Result<()>;
    /// `player.getAttribute(key)` (absent or null is `None`).
    fn attribute(&mut self, player: &Self::Player, key: &str) -> Result<Option<Value>>;
    /// `core.api.getAttribute(player, key, default)` (a stored null is `None`).
    fn api_attribute(
        &mut self,
        player: &Self::Player,
        key: &str,
        default: Value,
    ) -> Result<Option<Value>>;
    /// `core.api.setAttribute(player, key, value)`; `None` is a null value.
    fn set_attribute(
        &mut self,
        player: &Self::Player,
        key: &str,
        value: Option<Value>,
    ) -> Result<()>;
    /// `player.teleporter.send(location, type)`.
    fn teleport(
        &mut self,
        player: &Self::Player,
        location: Loc,
        kind: TeleportType,
    ) -> Result<bool>;
    /// `player.properties.teleportLocation = location`.
    fn set_teleport_location(&mut self, player: &Self::Player, location: Option<Loc>)
    -> Result<()>;

    // Accounts (native: crate::auth, see [`accounts`]).
    /// `GameWorld.accountStorage.checkUsernameTaken(name)`.
    fn username_taken(&mut self, name: &str) -> Result<bool>;
    /// `GameWorld.accountStorage.getAccountInfo(name)`.
    fn account_info(&mut self, name: Option<&str>) -> Result<Account>;
    /// `GameWorld.accountStorage.update(info)`.
    fn account_update(&mut self, info: &Account) -> Result<()>;
    /// `GameWorld.accountStorage.getUsernamesWithIP(ip)`.
    fn usernames_with_ip(&mut self, ip: &str) -> Result<Vec<Option<String>>>;
    /// `GameWorld.authenticator.checkPassword(player, password)`.
    fn check_password(&mut self, player: &Self::Player, password: &str) -> Result<bool>;
    /// `GameWorld.authenticator.updatePassword(username, password)`.
    fn update_password(&mut self, username: &str, password: &str) -> Result<()>;
    /// `ServerStore.getArchive(archive).addToList(key, value)`.
    fn store_add_to_list(&mut self, archive: &str, key: &str, value: &str) -> Result<()>;

    // Scheduling.
    /// `GameWorld.Pulser.submit(pulse)`.
    fn submit_pulse(&mut self, pulse: Pulse<Self::Player>) -> Result<()>;
    /// `GlobalScope.launch { .. }`.
    fn launch(&mut self, job: Job<Self::Player>) -> Result<()>;
    /// `File(directory + File.separator + name).exists()`, then its lines
    /// (`readLines`) when it does; the directory is `DATA_PATH` as read.
    fn file_lines(&mut self, directory: Option<&str>, name: &str) -> Result<Option<Vec<String>>>;

    // The system manager.
    /// `SystemManager.getUpdater().setCountdown(ticks)`.
    fn set_update_countdown(&mut self, seconds: i32) -> Result<()>;
    /// `SystemManager.flag(SystemState.UPDATING)`.
    fn flag_updating(&mut self) -> Result<()>;
    /// `SystemManager.getUpdater().cancel()`.
    fn cancel_update(&mut self) -> Result<()>;
    /// `exitProcess(status)`.
    fn exit_process(&mut self, status: i32) -> Result<()>;

    // Items and containers.
    /// `ItemDefinition.getDefinitions().size`.
    fn item_definitions_size(&mut self) -> Result<i32>;
    /// `Item(id).getName()` (`getDefinition().getName()`).
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `Item(id).getNoteChange()`.
    fn note_change(&mut self, id: i32) -> Result<i32>;
    /// `container.isFull()`.
    fn is_full(&mut self, player: &Self::Player, container: Container) -> Result<bool>;
    /// `container.add(Item(id, amount))`.
    fn add(
        &mut self,
        player: &Self::Player,
        container: Container,
        id: i32,
        amount: i32,
    ) -> Result<bool>;
    /// `container.remove(Item(id, amount))`.
    fn remove(
        &mut self,
        player: &Self::Player,
        container: Container,
        id: i32,
        amount: i32,
    ) -> Result<bool>;
    /// `container.getItem(Item(id, amount))`, then its amount: `None` when
    /// it answers null.
    fn item_amount(
        &mut self,
        player: &Self::Player,
        container: Container,
        id: i32,
        amount: i32,
    ) -> Result<Option<i32>>;
    /// `container.getAmount(id)`.
    fn amount(&mut self, player: &Self::Player, container: Container, id: i32) -> Result<i32>;
    /// `container.getAmount(Item(id))`.
    fn amount_item(&mut self, player: &Self::Player, container: Container, id: i32) -> Result<i32>;
    /// `container.toArray()`: the slots.
    fn slots(
        &mut self,
        player: &Self::Player,
        container: Container,
    ) -> Result<Vec<Option<ItemRef>>>;
    /// `container.get(slot)`.
    fn get(
        &mut self,
        player: &Self::Player,
        container: Container,
        slot: i32,
    ) -> Result<Option<ItemRef>>;
    /// `item.setCharge(charge)` of the item in that slot.
    fn set_charge(
        &mut self,
        player: &Self::Player,
        container: Container,
        item: &ItemRef,
        charge: i32,
    ) -> Result<()>;
    /// `container.replace(Item(id), slot)`.
    fn replace(
        &mut self,
        player: &Self::Player,
        container: Container,
        id: i32,
        slot: i32,
    ) -> Result<()>;
    /// `ChargedItem.forId(id) != null`.
    fn charged_item(&mut self, id: i32) -> Result<bool>;
    /// `ChargedItem.forId(id).forCharge(charge)`.
    fn charged_for_charge(&mut self, id: i32, charge: i32) -> Result<i32>;
    /// `ChargedItem.getCharge(id)` (`null` is `None`).
    fn charged_charge(&mut self, id: i32) -> Result<Option<i32>>;

    /// The same world as the command sets' host ([`sets::Host`]), for the
    /// bodies of the Misc, Spawn and Stats sets.
    fn sets(&mut self) -> Result<&mut dyn sets::Host<Player = Self::Player>>;
}

/// `colorize(line)` (`core.tools.Globals`).
pub fn colorize(line: &str) -> String {
    crate::timers::content::colorize(line)
}

/// `sets.CommandSet.reject(player, messages...)`: each message in red, then
/// an `IllegalStateException`.
pub fn reject<H: Host + ?Sized, T>(
    host: &mut H,
    player: &H::Player,
    messages: &[&str],
) -> Result<T> {
    for message in messages {
        host.message(player, &colorize(&format!("-->%R{message}")))?;
    }
    Err(Error::IllegalState(None))
}

/// `sets.CommandSet.notify(player, message)`.
pub fn notify<H: Host + ?Sized>(host: &mut H, player: &H::Player, message: &str) -> Result<()> {
    host.message(player, &colorize(&format!("-->{message}")))
}

/// `player.getRights()`: `Rights.values()[accountInfo.rights]`.
pub fn rights<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<Rights> {
    let ordinal = crate::auth::storage::lock(&host.account(player)).rights;
    Rights::from_ordinal(ordinal)
        .map_err(|_| Error::Index(format!("Index {ordinal} out of bounds for length 3")))
}

/// `CommandPlugin.toInteger(string)`: 1 for anything `toInt` refuses.
pub fn to_integer(text: &str) -> i32 {
    kotlin::to_int(text).unwrap_or(1)
}

/// `CommandPlugin.getArgumentLine(args, offset, length)`.
pub fn argument_line(args: &[Option<String>], offset: usize, length: usize) -> Result<String> {
    let mut out = String::new();
    for i in offset..length {
        if i != offset {
            out.push(' ');
        }
        let value = args.get(i).ok_or_else(|| {
            Error::Index(format!("Index {i} out of bounds for length {}", args.len()))
        })?;
        out.push_str(value.as_deref().unwrap_or("null"));
    }
    Ok(out)
}

/// `Decoders530.COMMAND`: what the decoder makes of a payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded {
    /// `Packet.Command(player, line)`.
    Command(String),
    /// `Packet.NoProcess` (one byte or less).
    NoProcess,
    /// The string had no terminator: "Something went wrong there! Please
    /// try again." is sent and the packet is `NoProcess`.
    Underflow,
}

/// `Decoders530.COMMAND.decode`: more than one byte is read as a
/// zero-terminated string (bytes widened as signed values) and lower-cased
/// in the root locale.
pub fn decode(payload: &[u8]) -> Decoded {
    if payload.len() <= 1 {
        return Decoded::NoProcess;
    }
    let Some(end) = payload.iter().position(|b| *b == 0) else {
        return Decoded::Underflow;
    };
    let units: Vec<u16> = payload[..end]
        .iter()
        .map(|b| *b as i8 as i16 as u16)
        .collect();
    let text = String::from_utf16(&units).expect("no surrogates are produced");
    Decoded::Command(rs09_game_core::java_lower::lowercase_root(&text))
}

/// The `Command` branch of `PacketProcessor.process`, after the decoder:
/// `PlayerMonitor.log` then `CommandSystem.parse`.
pub fn process<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    line: &str,
) -> Result<bool> {
    host.monitor_log(player, line)?;
    parse(host, registry, Some(player), line)
}

/// The decoder and the branch together, as the packet reaches the world:
/// an unterminated string is told to the player and dropped, a short
/// payload dropped.
pub fn handle_packet<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    payload: &[u8],
) -> Result<()> {
    match decode(payload) {
        Decoded::NoProcess => Ok(()),
        Decoded::Underflow => host.message(player, "Something went wrong there! Please try again."),
        Decoded::Command(line) => process(host, registry, player, &line).map(|_| ()),
    }
}

/// `CommandSystem.parse(player, message)`.
pub fn parse<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: Option<&H::Player>,
    message: &str,
) -> Result<bool> {
    let Some(player) = player else {
        return Ok(false);
    };
    let arguments: Vec<String> = message.split(' ').map(str::to_owned).collect();
    match registry.get(&arguments[0]) {
        None => {
            for set in LegacySet::ALL {
                if interpret(host, registry, set, player, &arguments[0], &arguments)? {
                    host.message(
                        player,
                        &colorize(&format!("-->%Y{}: Deprecated command", arguments[0])),
                    )?;
                    return Ok(true);
                }
            }
            host.message(
                player,
                &colorize(&format!("-->%R{}: command not found", arguments[0])),
            )?;
        }
        Some(command) => match attempt_handling(host, registry, command, player, &arguments) {
            Err(Error::IllegalState(_)) => return Ok(true),
            other => other?,
        },
    }
    Ok(false)
}

/// The legacy `CommandSet.validate(player)`.
fn validate<H: Host + ?Sized>(host: &mut H, set: LegacySet, player: &H::Player) -> Result<bool> {
    Ok(match set {
        LegacySet::Player => true,
        LegacySet::Moderator => rights(host, player)?.ordinal() > 0,
        LegacySet::Administrator | LegacySet::Developer => {
            rights(host, player)? == Rights::Administrator
        }
        LegacySet::Beta => {
            host.beta().ok_or(Error::Null)?
                || validate(host, LegacySet::Administrator, player)?
                || host.dev_mode().ok_or(Error::Null)?
        }
    })
}

/// The legacy `CommandSet.interpret(player, name, arguments...)`.
pub fn interpret<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    set: LegacySet,
    player: &H::Player,
    name: &str,
    arguments: &[String],
) -> Result<bool> {
    if !validate(host, set, player)? {
        return Ok(false);
    }
    if host.zone_parse_command(player, name, arguments)? {
        return Ok(true);
    }
    // `CommandPlugin.validate` is final and answers true.
    for plugin in registry.plugins(set).to_vec() {
        if host.legacy_parse(plugin, player, name, arguments)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// "Hey, everyone, I just tried to do something very silly!"
pub const SILLY: &str = "Hey, everyone, I just tried to do something very silly!";

/// `Command.attemptHandling(player, args)`.
pub fn attempt_handling<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    command: &Command,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    let has_rights = rights(host, player)? == Rights::Administrator
        || (host.player_commands()
            && rights(host, player)?.ordinal() >= command.privilege.ordinal());
    let is_dev = host.dev_mode() == Some(true);
    let is_cheater = host.i_am_a_cheater();
    if has_rights || is_dev || is_cheater {
        run(host, registry, command.body, player, args)
    } else if host.player_commands() || arg(args, 0)? == "bank" {
        host.send_chat(player, SILLY)
    } else {
        Ok(())
    }
}

/// `command.handle(player, args)`: the body itself, without the rights
/// check (as `csvmodcr` and `AbuseReport` call it through the registry).
pub fn run<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    body: Body,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    use NativeBody::*;
    let body = match body {
        Body::Content(content) => return host.content_command(content, player, args),
        Body::Native(body) => body,
    };
    match body {
        Kick => moderation::kick(host, player, args),
        Ban => moderation::ban(host, player, args),
        IpBan => moderation::ipban(host, player, args),
        Mute => moderation::mute(host, player, args),
        Jail => moderation::jail(host, player, args),
        ModCr => moderation::modcr(host, player, args),
        CsvModCr => moderation::csvmodcr(host, player, args),
        ModerationGetAttribute => moderation::get_attribute(host, player, args),
        Update => system::update(host, args),
        CancelUpdate => system::cancel_update(host),
        ResetPassword => system::reset_password(host, player),
        SetPasswordOther => system::set_password_other(host, player, args),
        GiveItem => system::give_item(host, player, args),
        RemoveItem => system::remove_item(host, player, args),
        RemoveItemAll => system::remove_item_all(host, player, args),
        Potato => system::potato(host, player),
        Shutdown => host.exit_process(0),
        Charge => system::charge(host, player, args),
        To => teleport::to(host, player, args),
        Tele => teleport::tele(host, player, args),
        TeleTo => teleport::tele_to(host, player, args),
        TeleToMe => teleport::tele_to_me(host, player, args),
        Home => teleport::home(host, player),
        GetAttribute => attributes::get(host, player, args),
        SetAttribute => attributes::set(host, player, args),
        Set(body) => sets::run(host.sets()?, registry, body, player, args),
    }
}

/// A command error as the driver's `Exception`.
pub fn driver_error(error: Error) -> crate::world::driver::Error {
    crate::world::driver::Error::Exception(error.java_string())
}
