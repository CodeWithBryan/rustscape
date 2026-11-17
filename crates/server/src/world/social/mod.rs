//! Friends, ignores, private messages, clan chat, the management-event queue
//! that carries them in a single world, global chat and public-chat message
//! state, ported from the pinned `CommunicationInfo`, `ClanRepository`,
//! `ClanEntry`, `ClanRank`, `Contact`, `ManagementEvents`, `GlobalChat`,
//! `ChatMessage`, the social branches of `PacketProcessor`, the ten social
//! decoders of `Decoders530`, `Login.sendMSEvents`, `Player.sendLogoutEvents`,
//! `PacketRepository.send`, `PacketWriteQueue` and the outgoing
//! `ContactPackets`, `CommunicationMessage`, `UpdateClanChat` and
//! `MSPacketRepository` packets.
//!
//! A library: no session, login or tick driver installs it. Players are
//! handles of the host's type `P`; their details (account, communication info,
//! rights, mute) are the native `player::details::PlayerDetails` and
//! `player::communication::CommunicationInfo`, reached through
//! [`Host::with_details`]. Every other system is a required service of
//! [`Host`] in the order the original calls it, with no defaults. Host calls
//! cannot reenter this library while it runs (it is borrowed mutably), so the
//! original's fail-fast iterator paths for reentrant list changes are not
//! reachable here. See docs/social.md.
use crate::auth::account::UserAccountInfo;
use crate::player::communication::ClanRank;
use crate::player::details::{PlayerDetails, Rights};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

pub mod chat;
pub mod clan;
pub mod communication;
pub mod management;
pub mod names;
pub mod packets;

pub use chat::{ChatMessage, Decoded};
pub use clan::{Clan, ClanEntry, ClanId};
pub use management::Event;
pub use packets::{Header, Queued, Written};

/// A Java exception, or a failure thrown by another system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NullPointer,
    ArrayIndexOutOfBounds {
        index: i64,
        length: usize,
    },
    StringIndexOutOfBounds,
    IndexOutOfBounds,
    NumberFormat(String),
    ClassCast,
    BufferUnderflow,
    BufferOverflow,
    /// A failure another system threw: the text is its `toString()`.
    Host(String),
}
impl Error {
    /// The exception's simple class name, or the host's description.
    pub fn class_name(&self) -> String {
        match self {
            Self::NullPointer => "NullPointerException".into(),
            Self::ArrayIndexOutOfBounds { .. } => "ArrayIndexOutOfBoundsException".into(),
            Self::StringIndexOutOfBounds => "StringIndexOutOfBoundsException".into(),
            Self::IndexOutOfBounds => "IndexOutOfBoundsException".into(),
            Self::NumberFormat(_) => "NumberFormatException".into(),
            Self::ClassCast => "ClassCastException".into(),
            Self::BufferUnderflow => "BufferUnderflowException".into(),
            Self::BufferOverflow => "BufferOverflowException".into(),
            Self::Host(text) => text.clone(),
        }
    }
    /// `Throwable.toString()`: the first line `printStackTrace` writes.
    pub fn java_string(&self) -> String {
        match self {
            Self::NullPointer => "java.lang.NullPointerException".into(),
            Self::ArrayIndexOutOfBounds { index, length } => format!(
                "java.lang.ArrayIndexOutOfBoundsException: Index {index} out of bounds for length {length}"
            ),
            Self::StringIndexOutOfBounds => "java.lang.StringIndexOutOfBoundsException".into(),
            Self::IndexOutOfBounds => "java.lang.IndexOutOfBoundsException".into(),
            Self::NumberFormat(text) => format!("java.lang.NumberFormatException: {text}"),
            Self::ClassCast => "java.lang.ClassCastException".into(),
            Self::BufferUnderflow => "java.nio.BufferUnderflowException".into(),
            Self::BufferOverflow => "java.nio.BufferOverflowException".into(),
            Self::Host(text) => text.clone(),
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

/// `ClanRank.values()[ordinal]`.
pub fn rank_at(ordinal: i64) -> Result<ClanRank> {
    usize::try_from(ordinal)
        .ok()
        .and_then(|index| ClanRank::ALL.get(index).copied())
        .ok_or(Error::ArrayIndexOutOfBounds {
            index: ordinal,
            length: ClanRank::ALL.len(),
        })
}

/// `ClanRank.getValue()` of a nullable rank.
fn rank_value(rank: Option<ClanRank>) -> Result<i32> {
    rank.map(ClanRank::value).ok_or(Error::NullPointer)
}

/// `ClanRank.ordinal()` of a nullable rank.
fn rank_ordinal(rank: Option<ClanRank>) -> Result<i32> {
    rank.map(ClanRank::ordinal).ok_or(Error::NullPointer)
}

/// An attribute value as the social code stores and reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    Text(String),
    Bool(bool),
    Rights(Rights),
}

/// The log levels of `core.tools.Log`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fine,
    Info,
    Warn,
    Err,
    Debug,
}
impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fine => "FINE",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Err => "ERR",
            Self::Debug => "DEBUG",
        }
    }
}

/// A pulse this library submitted: the loot-share timer of one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PulseId(pub usize);

/// What the social code needs from systems that are not ported with it.
///
/// There are no defaults. Methods without a `Result` are field reads of the
/// original that cannot fail (or, for the clock and the seed, host inputs at
/// call sites the original reads them); every other method is a call into
/// another system and may fail with what that system threw.
pub trait Host<P> {
    // Player fields.
    /// `player.getName()`.
    fn name(&mut self, player: &P) -> String;
    /// `player.getDetails()`: the account, rights, mute and communication info.
    /// The closure must not call back into the host.
    fn with_details<R>(&mut self, player: &P, f: impl FnOnce(&mut PlayerDetails) -> R) -> R;
    /// `player.getSettings().getPrivateChatSetting()`.
    fn private_chat_setting(&mut self, player: &P) -> i32;
    /// `player instanceof AIPlayer`.
    fn is_ai(&mut self, player: &P) -> bool;
    /// `player.isArtificial()`.
    fn is_artificial(&mut self, player: &P) -> bool;
    /// `player.getSession() != null`.
    fn has_session(&mut self, player: &P) -> bool;
    /// `player.getInterfaceManager().isResizable()`.
    fn is_resizable(&mut self, player: &P) -> bool;

    // World settings and server constants.
    /// `GameWorld.getSettings().getWorldId()`.
    fn world_id(&mut self) -> i32;
    /// `GameWorld.getSettings().getName()`.
    fn world_name(&mut self) -> String;
    /// `GameWorld.getSettings().isDevMode()`.
    fn dev_mode(&mut self) -> bool;
    /// `GameWorld.settings.enable_default_clan`.
    fn enable_default_clan(&mut self) -> bool;
    /// `ServerConstants.SERVER_NAME`.
    fn server_name(&mut self) -> String;
    /// `ServerConstants.MS_SECRET_KEY`.
    fn ms_secret_key(&mut self) -> String;
    /// `ServerConstants.ENABLE_GLOBAL_CHAT`.
    fn global_chat_enabled(&mut self) -> bool;
    /// `WorldCommunicator.isEnabled()`: the management server is AVAILABLE.
    /// The pinned Server.kt never connects, so a pinned world answers false.
    fn ms_enabled(&mut self) -> bool;
    /// `WorldCommunicator.getSession() != null`.
    fn ms_session(&mut self) -> bool;

    // The repository (native: world::repository::Repository).
    /// `Repository.getPlayerByName(name)`: lower case, spaces to underscores,
    /// then the name map.
    fn player_by_name(&mut self, name: &str) -> Option<P>;
    /// `Repository.playerNames.keys` in the order the map is walked.
    fn player_names(&mut self) -> Vec<String>;
    /// `Repository.players` in list order.
    fn players(&mut self) -> Vec<P>;

    // Host inputs at the original's call sites.
    /// `System.currentTimeMillis()`.
    fn current_time_millis(&mut self) -> i64;
    /// The seed of an unseeded `new java.util.Random()`.
    fn random_seed(&mut self) -> i64;

    // Services.
    /// `player.sendMessage(text)`.
    fn player_message(&mut self, player: &P, text: &str) -> Result<()>;
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn dispatch_message(&mut self, player: &P, text: &str) -> Result<()>;
    /// `core.api.sendMessage(player, text)`.
    fn api_message(&mut self, player: &P, text: &str) -> Result<()>;
    /// `player.getPacketDispatch().sendString(text, interface, child)`.
    fn send_string(&mut self, player: &P, text: &str, interface: i32, child: i32) -> Result<()>;
    /// `player.getInterfaceManager().open(new Component(id))`.
    fn open_component(&mut self, player: &P, id: i32) -> Result<()>;
    /// `player.getAttribute(key, ...)`: the stored value, `None` when absent or null.
    fn get_attribute(&mut self, player: &P, key: &str) -> Result<Option<Value>>;
    /// `player.setAttribute(key, value)`.
    fn set_attribute(&mut self, player: &P, key: &str, value: Value) -> Result<()>;
    /// `player.getLocks().isLocked(key)`.
    fn is_locked(&mut self, player: &P, key: &str) -> Result<bool>;
    /// `player.getLocks().lock(key, ticks)`.
    fn lock(&mut self, player: &P, key: &str, ticks: i32) -> Result<()>;
    /// `player.getUpdateMasks().register(EntityFlag.Chat, message)`; the mask
    /// encoding itself is world/update_masks.
    fn register_chat(&mut self, player: &P, message: &ChatMessage<P>, sync: bool) -> Result<bool>;
    /// `session.getIsaacPair().getOutput().getNextValue()`.
    fn cipher(&mut self, player: &P) -> Result<i32>;
    /// `player.getSession().write(buffer)`.
    fn write(&mut self, player: &P, packet: Written) -> Result<()>;
    /// `WorldCommunicator.getSession().write(buffer)`.
    fn ms_write(&mut self, packet: Written) -> Result<()>;
    /// `setVarp(player, index, value, save)`.
    fn varp(&mut self, player: &P, index: i32, value: i32, save: bool) -> Result<()>;
    /// `GameWorld.getPulser().submit(pulse)`; the pulse runs through
    /// [`Social::run_pulse`] when the host's pulse queue says so.
    fn submit_pulse(&mut self, pulse: PulseId, delay: i32) -> Result<()>;
    /// `pulse.stop()`.
    fn stop_pulse(&mut self, pulse: PulseId) -> Result<()>;
    /// `core.api.log(origin, level, message)`; stack frames are not part of the message.
    fn log(&mut self, origin: &str, level: Level, message: &str) -> Result<()>;
    /// `SystemLogger.logMS(text)`.
    fn log_ms(&mut self, text: &str) -> Result<()>;
    /// `PlayerMonitor.logChat(player, kind, message)`.
    fn log_chat(&mut self, player: &P, kind: &str, message: &str) -> Result<()>;
    /// `PlayerMonitor.logPrivateChat(player, receiver, message)`.
    fn log_private_chat(&mut self, player: &P, receiver: &str, message: &str) -> Result<()>;
    /// `GameWorld.accountStorage.getAccountInfo(username)`.
    fn account_info(&mut self, username: &str) -> Result<UserAccountInfo>;
    /// `GameWorld.accountStorage.getOnlineFriends(username)`.
    fn online_friends(&mut self, username: &str) -> Result<Vec<String>>;
    /// `GameWorld.accountStorage.update(info)`.
    fn account_update(&mut self, info: &UserAccountInfo) -> Result<()>;
    /// `GameWorld.authenticator.createAccountWith(info)`.
    fn create_account(&mut self, info: &UserAccountInfo) -> Result<bool>;
    /// `Commands.define(name, privilege, usage, description, handler)`; the
    /// one handler this library defines is [`Social::mute_global`].
    fn define_command(
        &mut self,
        name: &str,
        privilege: &str,
        usage: &str,
        description: &str,
    ) -> Result<()>;
    /// `Settings.updateChatSettings(public, private, trade)` (native: player::settings).
    fn update_chat_settings(
        &mut self,
        player: &P,
        public: i32,
        private: i32,
        trade: i32,
    ) -> Result<()>;
    /// `((AIPlayer) player).handleIncomingChat(context)`. Not reached through
    /// `PacketRepository.send`, which queues nothing for an artificial player.
    fn ai_incoming_chat(
        &mut self,
        player: &P,
        opcode: i32,
        other: &str,
        message: &str,
    ) -> Result<()>;
    /// The management coroutine takes `event` from its queue. Recording only.
    fn management_take(&mut self, event: &Event);
}

/// Per-player fields of `CommunicationInfo` that hold objects of this library.
#[derive(Clone, Debug, Default)]
struct Links {
    clan: Option<ClanId>,
    loot_share_pulse: Option<PulseId>,
}

/// The state the original keeps in statics: the clan registry, the
/// management queue and its waiting lists, the packet write queue, plus the
/// clan objects themselves and the pulses this library submitted.
pub struct Social<P> {
    clans: Vec<Clan<P>>,
    registry: HashMap<String, ClanId>,
    queue: VecDeque<Event>,
    waiting: HashMap<String, VecDeque<Event>>,
    requested: HashMap<String, bool>,
    writes: VecDeque<Queued<P>>,
    flushing: VecDeque<Queued<P>>,
    links: HashMap<P, Links>,
    pulses: Vec<P>,
}
impl<P> Default for Social<P> {
    fn default() -> Self {
        Self {
            clans: Vec::new(),
            registry: HashMap::new(),
            queue: VecDeque::new(),
            waiting: HashMap::new(),
            requested: HashMap::new(),
            writes: VecDeque::new(),
            flushing: VecDeque::new(),
            links: HashMap::new(),
            pulses: Vec::new(),
        }
    }
}

impl<P: Clone + Eq + Hash> Social<P> {
    pub fn new() -> Self {
        Self::default()
    }

    /// A clan object, registered or not.
    pub fn clan(&self, id: ClanId) -> &Clan<P> {
        &self.clans[id.0]
    }
    /// `ClanRepository.getClans()` as (owner, clan) pairs, in no particular order.
    pub fn registry(&self) -> impl Iterator<Item = (&str, ClanId)> {
        self.registry
            .iter()
            .map(|(owner, &id)| (owner.as_str(), id))
    }
    /// `ClanRepository.getClans().get(owner)`, without loading anything.
    pub fn registered(&self, owner: &str) -> Option<ClanId> {
        self.registry.get(owner).copied()
    }
    /// `communication.getClan()`.
    pub fn clan_of(&self, player: &P) -> Option<ClanId> {
        self.links.get(player).and_then(|links| links.clan)
    }
    /// `communication.lootSharePulse`.
    pub fn loot_share_pulse(&self, player: &P) -> Option<PulseId> {
        self.links
            .get(player)
            .and_then(|links| links.loot_share_pulse)
    }
    /// The management events waiting in the queue, in order.
    pub fn pending(&self) -> impl Iterator<Item = &Event> {
        self.queue.iter()
    }
    /// `waitingOnClanInfo`.
    pub fn waiting(&self) -> impl Iterator<Item = (&str, &VecDeque<Event>)> {
        self.waiting
            .iter()
            .map(|(name, queue)| (name.as_str(), queue))
    }
    /// `hasRequestedClanInfo`.
    pub fn requested(&self) -> impl Iterator<Item = (&str, bool)> {
        self.requested
            .iter()
            .map(|(name, &value)| (name.as_str(), value))
    }
    /// The packets waiting for the end-of-tick flush, in order.
    pub fn queued_writes(&self) -> impl Iterator<Item = &Queued<P>> {
        self.writes.iter()
    }

    /// `CommunicationInfo.setClan(clan)`: the clan, and the current clan's
    /// owner (or null).
    pub fn set_clan<H: Host<P>>(&mut self, host: &mut H, player: &P, clan: Option<ClanId>) {
        let owner = clan.map(|id| self.clans[id.0].owner.clone());
        self.links.entry(player.clone()).or_default().clan = clan;
        host.with_details(player, |details| details.communication.current_clan = owner);
    }
}

/// `player.getDetails().getRights()`: `Rights.values()[accountInfo.rights]`.
fn rights<P, H: Host<P>>(host: &mut H, player: &P) -> Result<Rights> {
    let ordinal = host.with_details(player, |details| {
        crate::auth::storage::lock(&details.account).rights
    });
    Rights::from_ordinal(ordinal).map_err(|_| Error::ArrayIndexOutOfBounds {
        index: i64::from(ordinal),
        length: 3,
    })
}

/// `player.getDetails().isMuted()`; the details read their own clock.
fn is_muted<P, H: Host<P>>(host: &mut H, player: &P) -> bool {
    host.with_details(player, |details| details.is_muted())
}

/// `Rights.getChatIcon(player)`: the `visible_rank` attribute, defaulting to
/// the player's rights (read first); its ordinal unless regular or null.
pub fn chat_icon<P, H: Host<P>>(host: &mut H, player: &P) -> Result<i32> {
    let rights = rights(host, player)?;
    let shown = match host.get_attribute(player, "visible_rank")? {
        None => Some(rights),
        Some(Value::Rights(value)) => Some(value),
        Some(_) => return Err(Error::ClassCast),
    };
    Ok(match shown {
        Some(value) if value != Rights::RegularPlayer => value.ordinal(),
        _ => 0,
    })
}

/// Kotlin `CharSequence.isBlank()`: every unit is `Character.isWhitespace`
/// or `Character.isSpaceChar`.
fn is_blank(text: &str) -> bool {
    text.chars().all(|ch| {
        matches!(
            ch,
            '\t' | '\n'
                | '\u{0B}'
                | '\u{0C}'
                | '\r'
                | '\u{1C}'..='\u{20}'
                | '\u{A0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
        )
    })
}

/// Kotlin `String.split(delimiter)`: every part, trailing empties included.
fn kotlin_split<'a>(text: &'a str, delimiter: &str) -> Vec<&'a str> {
    text.split(delimiter).collect()
}
