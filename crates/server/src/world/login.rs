//! Login into the world: from a decoded, authenticated login block to a
//! player standing in the world with the client configured. A port of
//! `Login.proceedWith` and the rest of `Login.kt` after decoding,
//! `LoginParser.kt`, `PlayerParser.parse`, `Player.init`, `Entity.init` and
//! `LoginConfiguration.java`, in the original order.
//!
//! A library: nothing installs it. It runs on the native repository
//! ([`super::repository::Repository`]: `addPlayer`, the uid map,
//! `removePlayer`, the lobby list and `DisconnectionQueue.remove`), social
//! ([`super::social::Social::login_events`]), the interface manager
//! ([`super::interfaces`]: windows pane, chatbox, default tabs, info bars,
//! close, interface config), `PlayerDetails` ([`crate::player::details`]) and
//! the login reply writer ([`rs09_protocol::login::reply`]). Every other call
//! the original makes is a required service of [`Host`], called in the
//! original order, with no default: the canonical player's own libraries
//! (containers, skills, Settings, variables, the save file) are bound behind
//! those services by the caller, and content (login listeners, login plugins,
//! interface listeners, quests, music, emotes, familiars) is reached only
//! through services that name what is not ported.
//!
//! The world pulse that finishes a login is not run here: [`admission`]
//! submits a [`pulse::LoginPulse`] through [`Host::submit_login_pulse`], and
//! the tick driver (not ported) runs [`pulse::run`] when the world pulse queue
//! reaches it. See docs/world-login.md.
pub mod admission;
pub mod configuration;
pub mod pulse;

use super::{doors::Location, interfaces, repository, social};
use crate::player::details::PlayerDetails;
use rs09_protocol::login::AuthResponse;

/// What stops a call, in the categories the original's `catch` clauses use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// An `Exception`: stopped by `catch (Exception)` and `catch (Throwable)`.
    Exception(String),
    /// A `Throwable` that is not an `Exception`: stopped only by `catch (Throwable)`.
    Throwable(String),
    /// A native bound with no counterpart in the original; nothing catches it.
    Native(String),
}
impl Error {
    /// Whether `catch (Exception)` stops it.
    pub fn is_exception(&self) -> bool {
        matches!(self, Self::Exception(_))
    }
    /// Whether `catch (Throwable)` stops it.
    pub fn is_throwable(&self) -> bool {
        !matches!(self, Self::Native(_))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

impl From<repository::Error> for Error {
    fn from(error: repository::Error) -> Self {
        match error {
            repository::Error::Throwable(text) => Self::Throwable(text),
            repository::Error::Capacity => Self::Native("node list capacity".into()),
            other => Self::Exception(format!("{other:?}")),
        }
    }
}
impl From<interfaces::Error> for Error {
    fn from(error: interfaces::Error) -> Self {
        match error {
            interfaces::Error::Binding(text) => Self::Native(text),
            interfaces::Error::Limit => Self::Native("interface loop bound".into()),
            other => Self::Exception(format!("{other:?}")),
        }
    }
}

/// The two levels the login code logs at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
}

/// `LoginParser.flag(response)` and `IoSession.write(response)` both write
/// through the session; `instant` is `write(context, true)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Write {
    Queued,
    Instant,
}

/// The constants and world settings the login code reads. An explicit input,
/// read from the world's configuration; no defaults.
#[derive(Clone, Debug)]
pub struct Constants {
    /// `ServerConstants.SERVER_NAME`.
    pub server_name: String,
    /// `GameWorld.settings.worldId`.
    pub world_id: i32,
    /// `ServerConstants.HOME_LOCATION`, nullable.
    pub home_location: Option<Location>,
    /// `ServerConstants.DAILY_ACCOUNT_LIMIT`.
    pub daily_account_limit: i32,
    /// `ServerConstants.CONFIG_PATH`, nullable (concatenated as "null" by Java).
    pub config_path: Option<String>,
    /// `ServerConstants.RULES_AND_INFO_ENABLED`.
    pub rules_and_info_enabled: bool,
    /// `GameWorld.settings.message_model`.
    pub message_model: i32,
    /// `GameWorld.settings.message_string`.
    pub message_string: String,
}

/// `Login.NORMAL_LOGIN_OP` and `RECONNECT_LOGIN_OP`.
pub const NORMAL_LOGIN_OP: i32 = 16;
pub const RECONNECT_LOGIN_OP: i32 = 18;

/// What the login code needs from systems that are not ported with it, and
/// from the canonical player's own libraries, which the caller binds.
///
/// There are no defaults. Methods without a `Result` are field reads and
/// writes of the original that cannot fail, or host inputs (the clock) at the
/// original's call sites; every other method is a call into another system
/// and may fail with what that system threw.
pub trait Host<P>: repository::Host<P> {
    /// The interface context of [`interfaces`] for one player.
    type Ui: interfaces::Context;
    /// `PlayerSaveParser`.
    type Parser;
    /// The services of [`social`].
    type Social: social::Host<P>;

    // Host inputs.
    /// `System.currentTimeMillis()`, at the original's call sites.
    fn now_millis(&mut self) -> i64;
    /// `ServerConstants` and `GameWorld.settings`.
    fn constants(&mut self) -> Constants;
    /// The seed of an unseeded `new java.util.Random()` (LoginConfiguration.autoSelect).
    fn random_seed(&mut self) -> i64;

    // The session.
    /// `IoSession.getAddress()`, read through `PlayerDetails.getIpAddress()`.
    fn session_address(&mut self, session: &Self::Session) -> Option<String>;
    /// `IoSession.getServerKey()`, written after reply 21.
    fn server_key(&mut self, session: &Self::Session) -> i64;
    /// The bytes `LoginWriteEvent.write` queues on the socket.
    fn write_bytes(&mut self, session: &Self::Session, bytes: Vec<u8>, write: Write) -> Result<()>;
    /// `session.setProducer(GAME_PRODUCER)` after a successful reply.
    fn set_game_producer(&mut self, session: &Self::Session);
    /// `session.lastPing = time`.
    fn set_last_ping(&mut self, session: &Self::Session, millis: i64);

    // Systems the admission reaches.
    /// `ServerStore.getArchive("flagged-ips").getList<String>("ips")`.
    fn flagged_ips(&mut self) -> Result<Vec<String>>;
    /// `Discord.postPlayerAlert(username, message)`.
    fn post_player_alert(&mut self, username: &str, message: &str) -> Result<()>;
    /// `new Player(details)`: the canonical player for these details, not yet
    /// in the repository, at `ServerConstants.START_LOCATION`, inactive.
    fn create_player(&mut self, details: PlayerDetails) -> Result<P>;
    /// `player.getDetails()`; the closure must not call back into the host.
    fn with_details<R>(&mut self, player: &P, f: impl FnOnce(&mut PlayerDetails) -> R)
    -> Result<R>;
    /// `PlayerMonitor.log(player, LogType.IP_LOG, ip)`.
    fn log_ip(&mut self, player: &P, ip: Option<&str>) -> Result<()>;
    /// The `account_limit_exceptions.conf` block of `checkAccountLimit`: the
    /// file's existence, its reload when modified and the lookup of
    /// `exceptions."<ip>"` (`getLong(key, 0)`). `Ok(None)` when the file or
    /// the key is absent; an `Err` is what the block's `catch` stops.
    fn account_limit_exception(&mut self, path: &str, ip: Option<&str>) -> Result<Option<i64>>;
    /// `ServerStore.getArchive("daily-accounts").getList<String>(ip)`.
    fn daily_accounts(&mut self, ip: Option<&str>) -> Result<Vec<String>>;
    /// `archive.addToList(ip, username)`.
    fn add_daily_account(&mut self, ip: Option<&str>, username: &str) -> Result<()>;
    /// `e.printStackTrace()`.
    fn print_stack(&mut self, error: &Error);
    /// `SystemManager.isUpdating()`.
    fn is_updating(&mut self) -> bool;
    /// `PlayerParser.parse(player)`: `Ok(None)` when the parse failed and
    /// PlayerParser printed it and answered null.
    fn parse_save(&mut self, player: &P) -> Result<Option<Self::Parser>>;
    /// `GameWorld.Pulser.submit(pulse)` of LoginParser's `Pulse(1)`.
    fn submit_login_pulse(
        &mut self,
        pulse: pulse::LoginPulse<P, Self::Parser, Self::Session>,
    ) -> Result<()>;
    /// The social library and its services (`Login.sendMSEvents`).
    fn social(&mut self) -> (&mut social::Social<P>, &mut Self::Social);

    // The login pulse.
    /// `getAttribute(player, "/save:spawnLocation", HOME_LOCATION)`.
    fn saved_spawn_location(
        &mut self,
        player: &P,
        default: Option<Location>,
    ) -> Result<Option<Location>>;
    /// `player.properties.spawnLocation = location`.
    fn set_spawn_location(&mut self, player: &P, location: Option<Location>) -> Result<()>;
    /// One `LoginListener.login(player)` of `GameWorld.loginListeners`, by class
    /// name; content, not ported.
    fn login_listener(&mut self, listener: &str, player: &P) -> Result<()>;
    /// `parser.runContentHooks()`.
    fn run_content_hooks(&mut self, player: &P, parser: &Self::Parser) -> Result<()>;
    /// `player.initReconnect()`.
    fn init_reconnect(&mut self, player: &P) -> Result<()>;
    /// `Node.active = active`.
    fn set_active(&mut self, player: &P, active: bool);
    /// `TimerRegistry.addAutoTimers(entity)`.
    fn add_auto_timers(&mut self, player: &P) -> Result<()>;
    /// `player.updateSceneGraph(login)`: UpdateSceneGraph, or BuildDynamicScene
    /// in a dynamic region, both sent at once by PacketWriteQueue.
    fn update_scene_graph(&mut self, player: &P, login: bool) -> Result<()>;
    /// `reinitVarps(player)`.
    fn reinit_varps(&mut self, player: &P) -> Result<()>;
    /// `player.getAttribute(key, default)` for a boolean.
    fn attribute_bool(&mut self, player: &P, key: &str, default: bool) -> Result<bool>;
    /// `player.getAttribute("login_type", LoginType.NORMAL_LOGIN) != LoginType.RECONNECT_TYPE`.
    fn login_type_not_reconnect(&mut self, player: &P) -> Result<bool>;
    /// `player.setAttribute(key, value)` for a boolean.
    fn set_attribute_bool(&mut self, player: &P, key: &str, value: bool) -> Result<()>;
    /// `core.api.log(origin, level, message)`.
    fn log_line(&mut self, origin: &str, level: Level, message: &str) -> Result<()>;
    /// `node.getIndex()`.
    fn index(&mut self, player: &P) -> i32;

    // LoginConfiguration.
    /// The player's interface manager and the context it runs in.
    fn ui(&mut self, player: &P) -> Result<(&mut Self::Ui, interfaces::Manager)>;
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn dispatch_message(&mut self, player: &P, text: &str) -> Result<()>;
    /// `setInterfaceText(player, text, interface, child)`.
    fn set_interface_text(
        &mut self,
        player: &P,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `player.getPacketDispatch().sendLastLoginInfo()`.
    fn send_last_login_info(&mut self, player: &P) -> Result<()>;
    /// `player.getInventory().refresh()`.
    fn inventory_refresh(&mut self, player: &P) -> Result<()>;
    /// `player.getEquipment().refresh()`.
    fn equipment_refresh(&mut self, player: &P) -> Result<()>;
    /// `player.getSkills().refresh()`.
    fn skills_refresh(&mut self, player: &P) -> Result<()>;
    /// `player.getSkills().configure()`.
    fn skills_configure(&mut self, player: &P) -> Result<()>;
    /// `player.getSettings().update()`.
    fn settings_update(&mut self, player: &P) -> Result<()>;
    /// `player.getInteraction().setDefault()`.
    fn interaction_set_default(&mut self, player: &P) -> Result<()>;
    /// `player.getPacketDispatch().sendRunEnergy()`.
    fn send_run_energy(&mut self, player: &P) -> Result<()>;
    /// `player.getFamiliarManager().login()`.
    fn familiar_login(&mut self, player: &P) -> Result<()>;
    /// `player.getQuestRepository().syncronizeTab(player)`.
    fn quest_synchronize_tab(&mut self, player: &P) -> Result<()>;
    /// `player.getEmoteManager().refresh()`.
    fn emote_refresh(&mut self, player: &P) -> Result<()>;
    /// `LOGIN_PLUGINS`, in order, by class name.
    fn login_plugins(&mut self) -> Vec<String>;
    /// One `plugin.newInstance(player)`; content, not ported.
    fn login_plugin(&mut self, plugin: &str, player: &P) -> Result<()>;
    /// `player.getAppearance().sync()`.
    fn appearance_sync(&mut self, player: &P) -> Result<()>;
    /// `UpdateSequence.getRenderablePlayers().add(player)`.
    fn renderable_players_add(&mut self, player: &P) -> Result<()>;
    /// `RegionManager.move(player)`.
    fn region_move(&mut self, player: &P) -> Result<()>;
    /// `player.getMusicPlayer().init()`.
    fn music_init(&mut self, player: &P) -> Result<()>;
    /// `player.updateAppearance()`.
    fn update_appearance(&mut self, player: &P) -> Result<()>;
    /// `player.getPlayerFlags().setUpdateSceneGraph(value)`.
    fn set_update_scene_graph_flag(&mut self, player: &P, value: bool) -> Result<()>;
    /// `player.getGlobalData().getTestStage()`.
    fn test_stage(&mut self, player: &P) -> Result<i32>;
    /// `player.getEmoteManager().isUnlocked(Emotes.SAFETY_FIRST)`.
    fn safety_first_unlocked(&mut self, player: &P) -> Result<bool>;
    /// `player.getEmoteManager().unlock(Emotes.SAFETY_FIRST)`.
    fn unlock_safety_first(&mut self, player: &P) -> Result<()>;
    /// `player.getSettings().getAttackStyleIndex()`.
    fn attack_style_index(&mut self, player: &P) -> Result<i32>;
    /// `player.getEquipment().toArray()`: per slot, the item's id, or null.
    fn equipment_items(&mut self, player: &P) -> Result<Vec<Option<i32>>>;
    /// The body of the loop over equipped items for one non-null item (equip
    /// hooks, unequip, bank, weapon style); content and equipment services.
    fn rerun_equip_hooks(&mut self, player: &P, slot: usize, saved_attack_style: i32)
    -> Result<()>;
    /// `player.getSpellBookManager().getSpellBook()`, an interface id.
    fn spell_book(&mut self, player: &P) -> Result<i32>;
    /// `player.sendMessage(text)`.
    fn player_message(&mut self, player: &P, text: &str) -> Result<()>;
    /// `hasRequirement(player, Quests.<quest>, message)`, by the enum constant's name.
    fn has_requirement(&mut self, player: &P, quest: &str, message: bool) -> Result<bool>;

    /// `player.getSpellBookManager().setSpellBook(book)`, an interface id.
    fn set_spell_book(&mut self, player: &P, book: i32) -> Result<()>;
    /// `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &P, id: i32, value: i32, save: bool) -> Result<()>;
    /// `RulesAndInfo.openFor(player)`; content.
    fn rules_and_info(&mut self, player: &P) -> Result<()>;
    /// `player.getDetails().setLastLogin(time)`.
    fn set_last_login(&mut self, player: &P, millis: i64) -> Result<()>;
}

/// `LoginWriteEvent.write(session, response)`: the response ordinal, then
/// for Success the player's rights and index from `session.getPlayer()` and
/// a switch of the session to the game producer, for ordinal 21 the server
/// key's low byte.
pub fn write_response<P: Clone + PartialEq, H: Host<P>>(
    host: &mut H,
    repository: &repository::Repository<P>,
    session: &H::Session,
    response: AuthResponse,
    write: Write,
) -> Result<()> {
    let (rights, index) = if response == AuthResponse::Success {
        let player = super::disconnection::session_player(host, repository, session)
            .ok_or_else(|| Error::Exception("NullPointerException: session.getPlayer()".into()))?;
        let rights = host.with_details(&player, |details| details.rights())?;
        let rights = rights
            .map_err(|_| Error::Exception("ArrayIndexOutOfBoundsException: rights".into()))?
            .ordinal();
        (rights, host.index(&player))
    } else {
        (0, 0)
    };
    let server_key = if response == AuthResponse::ErrorLoadingProfile {
        host.server_key(session)
    } else {
        0
    };
    let bytes = rs09_protocol::login::reply(response, rights, index, server_key);
    if response == AuthResponse::Success {
        host.set_game_producer(session);
    }
    host.write_bytes(session, bytes, write)
}

/// `PlayerDetails.getIpAddress()` once the session is set.
pub(crate) fn ip(details: &PlayerDetails) -> Option<String> {
    details.ip_address().map(str::to_owned)
}

/// `Player.getUsername()`: the account's user name.
pub(crate) fn username<P, H: Host<P>>(host: &mut H, player: &P) -> Result<String> {
    host.with_details(player, |details| {
        crate::auth::storage::lock(&details.account)
            .username
            .clone()
    })
}
