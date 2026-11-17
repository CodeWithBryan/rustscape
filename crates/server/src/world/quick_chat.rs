//! Quick chat: the `QUICKCHAT` decoder of `Decoders530` (and the same
//! decoding in `QuickChatPacketHandler`, with its dev-mode dump),
//! `QCRepository.sendQC` and its phrase lookups, and the packets it leaves
//! for viewers: a public quick-chat message is a `ChatMessage` with
//! `isQuickChat` registered on the chat mask by a pulse of delay 0 (native:
//! `world::social::ChatMessage` and the host's mask registry); a clan one is
//! a `ClanMessage` management event (native: `world::social::Event`,
//! published on the native management queue).
//!
//! The phrase text is cache index 24's file (archive 1) read as a
//! zero-terminated string of signed bytes; the selections come from item
//! names and the cache's data maps (index 17). Both are host services: the
//! world answers them from the pinned cache through the native readers
//! (`startup::loaders::music::DataMap`). Every outside call is a required
//! method of [`Host`] with no default. See docs/commands.md.
use crate::auth::account::Account;
use crate::auth::storage::lock;
use crate::player::details::Rights;
use crate::player::skills::Skills;
use crate::world::social::{ChatMessage, Event};

/// What stops quick chat, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `NullPointerException`, Kotlin's null checks included.
    Null,
    /// `BufferUnderflowException`.
    BufferUnderflow,
    /// `ArrayIndexOutOfBoundsException`/`IndexOutOfBoundsException`.
    Index(String),
    /// `ClassCastException` (a data map value that is not a string).
    Cast,
    /// A service failed with what it threw.
    Binding(Option<String>),
}
impl Error {
    pub fn java_string(&self) -> String {
        match self {
            Self::Null => "java.lang.NullPointerException".into(),
            Self::BufferUnderflow => "java.nio.BufferUnderflowException".into(),
            Self::Index(m) => format!("java.lang.ArrayIndexOutOfBoundsException: {m}"),
            Self::Cast => "java.lang.ClassCastException".into(),
            Self::Binding(m) => m.clone().unwrap_or_else(|| "service failure".into()),
        }
    }
    pub fn category(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::BufferUnderflow => "BufferUnderflow",
            Self::Index(_) => "Index",
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

/// `QCPacketType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketType {
    Standard,
    Single,
    Double,
    Unhandled,
}
impl PacketType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::Single => "SINGLE",
            Self::Double => "DOUBLE",
            Self::Unhandled => "UNHANDLED",
        }
    }
}

/// `Packet.QuickChat`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickChat {
    pub index_a: i32,
    pub index_b: i32,
    pub for_clan: bool,
    pub multiplier: i32,
    pub offset: i32,
    pub kind: PacketType,
}

/// The levels of `core.tools.Log` the decoders use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fine,
    Warn,
}

/// `Decoders530.QUICKCHAT.decode`: the packet and the log lines the
/// decoder wrote before it finished or failed. The type is the payload's
/// length (the buffer wraps an array of exactly the payload).
pub fn decode(payload: &[u8]) -> (Vec<(Level, String)>, Result<QuickChat>) {
    let mut logs = Vec::new();
    let kind = match payload.len() {
        3 | 4 => PacketType::Standard,
        5 => PacketType::Single,
        7 => PacketType::Double,
        size => {
            logs.push((Level::Warn, format!("UNHANDLED QC PACKET TYPE Size {size}")));
            PacketType::Unhandled
        }
    };
    let mut at = 0usize;
    let mut byte = || -> Result<i32> {
        let b = *payload.get(at).ok_or(Error::BufferUnderflow)?;
        at += 1;
        Ok(i32::from(b as i8))
    };
    let decoded = (|| {
        let for_clan = (byte()? & 0xFF) == 1;
        let multiplier = byte()?;
        let offset = byte()?;
        let (mut index_a, mut index_b) = (-1, -1);
        match kind {
            PacketType::Single => {
                let high = byte()?;
                let low = byte()?;
                index_a = i32::from((((high & 0xFF) << 8) | (low & 0xFF)) as u16 as i16);
            }
            PacketType::Double => {
                byte()?;
                index_a = byte()?;
                byte()?;
                index_b = byte()?;
            }
            PacketType::Unhandled => logs.push((
                Level::Warn,
                "Unhandled packet type, skipping remaining buffer contents.".into(),
            )),
            PacketType::Standard => {}
        }
        Ok(QuickChat {
            index_a,
            index_b,
            for_clan,
            multiplier,
            offset,
            kind,
        })
    })();
    (logs, decoded)
}

/// A pulse quick chat submits: `Pulse(0, player)` registering the chat mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatPulse<P> {
    pub player: P,
    pub message: ChatMessage<P>,
}
impl<P> ChatPulse<P> {
    /// The original's anonymous class, for records.
    pub const LABEL: &'static str = "core.net.packet.QCRepository$sendQC$1";
    pub const DELAY: i32 = 0;
}

/// What quick chat needs from the world. No defaults; methods without a
/// `Result` are field reads.
pub trait Host {
    type Player: Clone + PartialEq + std::fmt::Debug;

    /// `Cache.getIndexes()[24].getFileData(1, index)`.
    fn qc_file(&mut self, index: i32) -> Result<Option<Vec<u8>>>;
    /// `DataMap.get(map).getString(key)`: `Err(Cast)` for an integer value.
    fn data_map_string(&mut self, map: i32, key: i32) -> Result<Option<String>>;
    /// `ItemDefinition.forId(id).getName()` (`None` for a null name; a
    /// missing definition fails with `Null`).
    fn item_name(&mut self, id: i32) -> Result<Option<String>>;
    /// `player.skills.getExperience(slot)`.
    fn experience(&mut self, player: &Self::Player, slot: i32) -> Result<f64>;
    /// `player.skills.getStaticLevel(slot)`.
    fn static_level(&mut self, player: &Self::Player, slot: i32) -> Result<i32>;
    /// `SlayerManager.getInstance(player).amount`.
    fn slayer_amount(&mut self, player: &Self::Player) -> Result<i32>;
    /// `getSlayerTask(player)?.name` (the task's enum name).
    fn slayer_task_name(&mut self, player: &Self::Player) -> Result<Option<String>>;
    /// `player.setAttribute(key, value)`.
    fn set_attribute(&mut self, player: &Self::Player, key: &str, value: i32) -> Result<()>;
    /// `player.getName()`.
    fn name(&mut self, player: &Self::Player) -> String;
    /// `player.details.accountInfo`.
    fn account(&mut self, player: &Self::Player) -> Account;
    /// `player.communication.clan` (native: `Social::clan_of`) and its
    /// owner; `None` when the player is in no clan.
    fn clan_owner(&mut self, player: &Self::Player) -> Result<Option<String>>;
    /// `Rights.getChatIcon(player)` as `ChatMessage`'s constructor reads it
    /// (native: `world::social::chat::chat_icon`).
    fn chat_icon(&mut self, player: &Self::Player) -> Result<i32>;
    /// `ManagementEvents.publish(event)` (native: `Social::publish`).
    fn publish(&mut self, event: Event) -> Result<()>;
    /// `GameWorld.Pulser.submit(pulse)`.
    fn submit(&mut self, pulse: ChatPulse<Self::Player>) -> Result<()>;
    /// `player.updateMasks.register(EntityFlag.Chat, message)`.
    fn register_chat(
        &mut self,
        player: &Self::Player,
        message: &ChatMessage<Self::Player>,
    ) -> Result<()>;
    /// `log(origin, level, message)`.
    fn log(&mut self, origin: &str, level: Level, message: &str) -> Result<()>;
    /// `GameWorld.settings?.isDevMode == true`.
    fn dev_mode(&mut self) -> bool;
}

/// The class the `QUICKCHAT` decoder logs as (`this::class.java` of the
/// enum constant's body).
pub const DECODER_ORIGIN: &str = "core.net.packet.in.Decoders530$QUICKCHAT";

/// `getIndex(offset, multiplier)`.
pub fn index(offset: i32, multiplier: i32) -> i32 {
    if offset >= 0 {
        256i32.wrapping_mul(multiplier).wrapping_add(offset)
    } else {
        match multiplier {
            0 => 256 + offset,
            1 => 512 + offset,
            2 => 768 + offset,
            _ => 1024 + offset,
        }
    }
}

/// `getQCString(index)`: the file as a zero-terminated string of signed
/// bytes.
pub fn phrase<H: Host + ?Sized>(host: &mut H, index: i32) -> Result<String> {
    let data = host.qc_file(index)?.ok_or(Error::Null)?;
    let end = data
        .iter()
        .position(|b| *b == 0)
        .ok_or(Error::BufferUnderflow)?;
    let units: Vec<u16> = data[..end].iter().map(|b| *b as i8 as i16 as u16).collect();
    Ok(String::from_utf16(&units).expect("no surrogates are produced"))
}

/// `getFromMap(map, index)`: Kotlin's non-null return check.
fn from_map<H: Host + ?Sized>(host: &mut H, map: i32, index: i32) -> Result<String> {
    host.data_map_string(map, index)?.ok_or(Error::Null)
}

/// Kotlin `String.replaceFirst(old, new)` (literal).
fn replace_first(text: &str, old: &str, new: &str) -> String {
    text.replacen(old, new, 1)
}

/// `Rights.values()[accountInfo.rights].ordinal`.
fn rights_ordinal<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<i32> {
    let ordinal = lock(&host.account(player)).rights;
    Rights::from_ordinal(ordinal)
        .map(Rights::ordinal)
        .map_err(|_| Error::Index(format!("Index {ordinal} out of bounds for length 3")))
}

/// `getStandardQC(player, index)`: next-level experience, a skill level or
/// the slayer assignment filled in.
pub fn standard<H: Host + ?Sized>(host: &mut H, player: &H::Player, index: i32) -> Result<String> {
    let mut text = phrase(host, index)?;
    if text.contains("to get my next") {
        let split: Vec<&str> = text.split(' ').collect();
        let name = split
            .len()
            .checked_sub(2)
            .and_then(|i| split.get(i))
            .ok_or_else(|| {
                Error::Index(format!("Index -1 out of bounds for length {}", split.len()))
            })?;
        let skill = Skills::skill_by_name(Some(&name.to_uppercase()));
        let xp = host.experience(player, skill)?;
        let level = host.static_level(player, skill)?;
        let next = Skills::experience_by_level(level.wrapping_add(1));
        text = text.replace('<', &((f64::from(next) - xp) as i32).to_string());
    } else if text.contains("level is") {
        let split: Vec<&str> = text.split(' ').collect();
        let name = split.get(1).ok_or_else(|| {
            Error::Index(format!("Index 1 out of bounds for length {}", split.len()))
        })?;
        let skill = Skills::skill_by_name(Some(&name.to_uppercase()));
        let level = host.static_level(player, skill)?;
        text = text.replace('<', &level.to_string());
    } else if text.contains("My current Slayer assignment is") {
        let amount = host.slayer_amount(player)?;
        let task = host
            .slayer_task_name(player)?
            .map(|name| rs09_game_core::java_lower::lowercase_root(&name))
            .unwrap_or_else(|| "None".into());
        if amount > 0 {
            text = text.replace("complete", &format!("{amount} {task}"));
        }
    }
    Ok(text)
}

/// The data map each single-selection phrase fills `<` from, by the text
/// it contains, in the original's order of checks (after the item check).
pub const SINGLE_MAPS: &[(&[&str], i32)] = &[
    (&["I'd like the loan duration to be"], 1642),
    (
        &[
            "Let's go to Agility course:",
            "Try training your Agility at:",
        ],
        1505,
    ),
    (&["Try training on"], 1524),
    (&["Try ranging"], 1525),
    (&["flat-pack"], 1490),
    (
        &["I'm cooking", "Would you please cook me", "Try cooking"],
        1492,
    ),
    (
        &["I am crafting", "Try crafting", "Would you please craft me"],
        1493,
    ),
    (&["I'm growing crop", "Try growing crop"], 1494),
    (&["I'm burning logs"], 1495),
    (&["Try burning logs at"], 1544),
    (
        &["I'm fishing", "Would you please fish me", "Try fishing for"],
        1491,
    ),
    (
        &[
            "I'm fletching",
            "Try fletching",
            "Would you please fetch me",
        ],
        1496,
    ),
    (
        &[
            "I'm mixing potion",
            "Would you please mix me a potion",
            "Try mixing potion",
        ],
        1526,
    ),
    (&["Where can I get the secondary ingredient"], 1516),
    (&["Would you please hunt me", "Try hunting"], 1543),
    (&["I'm casting spell", "Would you please cast"], 1527),
    (&["I am on spell book"], 1528),
    (&["I'm mining ore"], 1529),
    (&["I'm using a pick"], 1531),
    (&["Try mining at"], 1533),
    (&["Use your prayer"], 1534),
    (&["I'm going to craft rune"], 1535),
    (&["Try crafting runes at"], 1536),
    (&["You should use the Slayer master in"], 2139),
    (&["Do you have spare Slayer equipment"], 1538),
    (&["I like the familiar", "I can summon up to"], 1539),
    (&["Good charm droppers are"], 1499),
    (&["Try thieving from"], 1540),
    (&["I'm using a woodcutting axe made"], 1532),
    (&["Try training Woodcutting at"], 1500),
    (&["Nice level in"], 1517),
];

/// The phrases whose `<` is an item name (when the phrase has a `<`).
const ITEM_PHRASES: [&str; 6] = [
    "item",
    "Can I buy your",
    "What is the best world to buy",
    "What is the best world to sell",
    "Would you like to borrow",
    "Could I please borrow",
];

/// `getSingleQC(index, selection)`.
pub fn single<H: Host + ?Sized>(host: &mut H, index: i32, selection: i32) -> Result<String> {
    let text = phrase(host, index)?;
    if ITEM_PHRASES.iter().any(|p| text.contains(p)) && text.contains('<') {
        let name = host.item_name(selection)?.ok_or(Error::Null)?;
        return Ok(text.replace('<', &name));
    }
    for (phrases, map) in SINGLE_MAPS {
        if phrases.iter().any(|p| text.contains(p)) {
            let value = from_map(host, *map, selection)?;
            return Ok(text.replace('<', &value));
        }
    }
    Ok(text)
}

/// `getDoubleQC(index, a, b)`: directions and smithing.
pub fn double<H: Host + ?Sized>(host: &mut H, index: i32, a: i32, b: i32) -> Result<String> {
    let mut text = phrase(host, index)?;
    let maps = if text.contains("That is < of <") {
        Some((1502, 1504))
    } else if ["I am smithing", "Try smithing", "Would you please smith me"]
        .iter()
        .any(|p| text.contains(p))
    {
        Some((1530, 1498))
    } else {
        None
    };
    if let Some((first, second)) = maps {
        let value = from_map(host, first, a)?;
        text = replace_first(&text, "<", &value);
        let value = from_map(host, second, b)?;
        text = replace_first(&text, "<", &value);
    }
    Ok(text)
}

/// `QCRepository.sendQC(player, multiplier, offset, type, a, b, forClan)`.
pub fn send<H: Host + ?Sized>(host: &mut H, player: &H::Player, chat: &QuickChat) -> Result<()> {
    let index = index(chat.offset, chat.multiplier);
    host.set_attribute(player, "qc_offset", chat.offset)?;
    let text = match chat.kind {
        PacketType::Single => single(host, index, chat.index_a)?,
        PacketType::Double => double(host, index, chat.index_a, chat.index_b)?,
        PacketType::Standard => standard(host, player, index)?,
        PacketType::Unhandled => String::new(),
    };
    if chat.for_clan {
        let sender = host.name(player);
        let owner = host.clan_owner(player)?.ok_or(Error::Null)?;
        let clan_name = rs09_game_core::java_lower::lowercase_root(&owner).replace(' ', "_");
        let rank = rights_ordinal(host, player)?;
        host.publish(Event::ClanMessage {
            sender,
            clan_name,
            message: text,
            rank,
        })
    } else {
        let chat_icon = host.chat_icon(player)?;
        let message = ChatMessage {
            player: player.clone(),
            num_chars: text.encode_utf16().count() as i32,
            text,
            effects: 0,
            chat_icon,
            is_quick_chat: true,
        };
        host.submit(ChatPulse {
            player: player.clone(),
            message,
        })
    }
}

/// The submitted pulse's `pulse()`: the mask, then finished.
pub fn run_pulse<H: Host + ?Sized>(host: &mut H, pulse: &ChatPulse<H::Player>) -> Result<bool> {
    host.register_chat(&pulse.player, &pulse.message)?;
    Ok(true)
}

/// The `QuickChat` branch of `PacketProcessor.process` with the
/// `Decoders530` decoder in front: the decoder's log lines, then `sendQC`.
/// A payload the decoder fails on is the `DecodingError` text.
pub fn handle_packet<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    payload: &[u8],
) -> std::result::Result<Result<()>, String> {
    let (logs, decoded) = decode(payload);
    for (level, line) in logs {
        if let Err(error) = host.log(DECODER_ORIGIN, level, &line) {
            return Ok(Err(error));
        }
    }
    match decoded {
        Ok(chat) => Ok(send(host, player, &chat)),
        Err(error) => Err(format!(
            "Error decoding opcode 167/QUICKCHAT: {}",
            error.java_string()
        )),
    }
}

/// `QuickChatPacketHandler.decode(player, opcode, buffer)`: the same
/// decoding, the dev-mode dump of the buffer, then `sendQC`.
pub fn handler<H: Host + ?Sized>(host: &mut H, player: &H::Player, payload: &[u8]) -> Result<()> {
    const ORIGIN: &str = "core.net.packet.in.QuickChatPacketHandler";
    let (logs, decoded) = decode(payload);
    for (level, line) in logs {
        host.log(ORIGIN, level, &line)?;
    }
    let chat = decoded?;
    if host.dev_mode() {
        host.log(
            ORIGIN,
            Level::Fine,
            "Begin QuickChat Packet Buffer Dump---------",
        )?;
        host.log(
            ORIGIN,
            Level::Fine,
            &format!(
                "Packet Type: {} Chat Type: {}",
                chat.kind.name(),
                if chat.for_clan { "Clan" } else { "Public" }
            ),
        )?;
        for b in payload {
            host.log(ORIGIN, Level::Fine, &(*b as i8).to_string())?;
        }
        host.log(
            ORIGIN,
            Level::Fine,
            "End QuickChat Packet Buffer Dump-----------",
        )?;
    }
    send(host, player, &chat)
}
