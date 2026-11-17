//! The social packets in and out of `PacketProcessor`: the ten social
//! decoders of `Decoders530` and its dispatch, the social branches of
//! `PacketProcessor.process`, `splitChatMessage`, `GlobalChat` and the public
//! `ChatMessage` up to the registration of the chat mask (whose encoding is
//! world/update_masks and the appearance writer's concern).
use super::management::Event;
use super::names::{decrypt_player_chat, format_display_name, long_to_string};
use super::{
    Error, Host, Result, Social, Value, chat_icon, is_blank, is_muted, kotlin_split, rank_at,
    rights,
};
use rs09_game_core::java_lower::lowercase_root;
use std::hash::Hash;

/// `PacketProcessor.BASE_CHAT_CUTOFF`.
pub const BASE_CHAT_CUTOFF: i32 = 81;
/// `GlobalChat.ATTR_GLOBAL_MUTE`.
pub const ATTR_GLOBAL_MUTE: &str = "/save:globalmute";

/// A decoded social packet (`Packet` subclasses), or what the dispatch makes
/// of an unknown opcode or a failed decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded {
    JoinClan {
        clan_name: String,
    },
    SetClanRank {
        username: String,
        rank: i32,
    },
    KickFromClan {
        username: String,
    },
    AddFriend {
        username: String,
    },
    RemoveFriend {
        username: String,
    },
    AddIgnore {
        username: String,
    },
    RemoveIgnore {
        username: String,
    },
    PrivateMessage {
        username: String,
        message: String,
    },
    ChatSetting {
        public: i32,
        private: i32,
        trade: i32,
    },
    ChatMessage {
        effects: i32,
        message: String,
    },
    /// `Packet.DecodingError`: the first line of its message (the stack
    /// frames that follow in the original are not part of it here).
    DecodingError(String),
    /// `Packet.UnhandledOp`: not one of the social opcodes.
    UnhandledOp,
}

/// The reads of `IoBuffer` the social decoders make.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl Reader<'_> {
    /// `get()`: a signed byte.
    fn get(&mut self) -> Result<i32> {
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or(Error::BufferUnderflow)?;
        self.position += 1;
        Ok(i32::from(byte as i8))
    }
    /// `getA()`.
    fn get_a(&mut self) -> Result<i32> {
        Ok((self.get()? & 0xff) - 128)
    }
    /// `getLong()`.
    fn get_long(&mut self) -> Result<i64> {
        let end = self.position + 8;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(Error::BufferUnderflow)?;
        self.position = end;
        Ok(i64::from_be_bytes(bytes.try_into().expect("eight bytes")))
    }
    /// `getSmart()`: the peeked byte is signed and compared with its own
    /// unsigned value, which it never exceeds, so the pinned server always
    /// reads one unsigned byte.
    fn get_smart(&mut self) -> Result<i32> {
        // The peek is an absolute get: past the end it is an IndexOutOfBoundsException.
        if self.position >= self.bytes.len() {
            return Err(Error::IndexOutOfBounds);
        }
        Ok(self.get()? & 0xff)
    }
    /// `StringUtils.decryptPlayerChat(buffer, total)`.
    fn chat(&mut self, total: i32) -> String {
        decrypt_player_chat(self.bytes, &mut self.position, total)
    }
}

/// `Decoders530.process(player, opcode, buffer)` for the social decoders.
pub fn decode(opcode: i32, payload: &[u8]) -> Decoded {
    let name = match opcode {
        104 => "CLAN_JOIN",
        188 => "CLAN_SETRANK",
        162 => "CLAN_KICK",
        120 => "ADD_FRIEND",
        57 => "REMOVE_FRIEND",
        34 => "ADD_IGNORE",
        213 => "REMOVE_IGNORE",
        201 => "PRIVATE_MESSAGE",
        157 => "CHAT_SETTINGS",
        237 => "CHAT_MESSAGE",
        _ => return Decoded::UnhandledOp,
    };
    let mut reader = Reader {
        bytes: payload,
        position: 0,
    };
    let decoded = (|| -> Result<Decoded> {
        Ok(match opcode {
            104 => Decoded::JoinClan {
                clan_name: long_to_string(reader.get_long()?),
            },
            188 => {
                let rank = reader.get_a()?;
                let username = long_to_string(reader.get_long()?);
                Decoded::SetClanRank { username, rank }
            }
            162 => Decoded::KickFromClan {
                username: long_to_string(reader.get_long()?),
            },
            120 => Decoded::AddFriend {
                username: long_to_string(reader.get_long()?),
            },
            57 => Decoded::RemoveFriend {
                username: long_to_string(reader.get_long()?),
            },
            34 => Decoded::AddIgnore {
                username: long_to_string(reader.get_long()?),
            },
            213 => Decoded::RemoveIgnore {
                username: long_to_string(reader.get_long()?),
            },
            201 => {
                let username = long_to_string(reader.get_long()?);
                let total = reader.get()? & 0xff;
                let message = reader.chat(total);
                Decoded::PrivateMessage { username, message }
            }
            157 => {
                let public = reader.get()?;
                let private = reader.get()?;
                let trade = reader.get()?;
                Decoded::ChatSetting {
                    public,
                    private,
                    trade,
                }
            }
            _ => {
                let prefix = reader.get()?.clamp(0, 11);
                let suffix = reader.get()?.clamp(0, 5);
                let count = reader.get_smart()?;
                let message = reader.chat(count);
                Decoded::ChatMessage {
                    effects: (prefix << 8) | suffix,
                    message,
                }
            }
        })
    })();
    decoded.unwrap_or_else(|error| {
        Decoded::DecodingError(format!(
            "Error decoding opcode {opcode}/{name}: {}",
            error.java_string()
        ))
    })
}

/// `ChatMessage`: the public chat context, its chat icon read at construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage<P> {
    pub player: P,
    pub text: String,
    pub effects: i32,
    pub num_chars: i32,
    pub chat_icon: i32,
    pub is_quick_chat: bool,
}
impl<P: Clone> ChatMessage<P> {
    /// `new ChatMessage(player, text, effects, numChars)`.
    pub fn new<H: Host<P>>(
        host: &mut H,
        player: &P,
        text: &str,
        effects: i32,
        num_chars: i32,
    ) -> Result<Self> {
        let chat_icon = chat_icon(host, player)?;
        Ok(Self {
            player: player.clone(),
            text: text.to_owned(),
            effects,
            num_chars,
            chat_icon,
            is_quick_chat: false,
        })
    }
}

fn units(text: &str) -> i32 {
    text.encode_utf16().count() as i32
}

/// `PacketProcessor.splitChatMessage(message, clanLength, rankPresent)`:
/// whole words up to the cutoff (81 less the clan length and 9 for a rank),
/// then the rest, in UTF-16 units.
pub fn split_chat_message(message: &str, clan_length: i32, rank_present: bool) -> Vec<String> {
    let all: Vec<u16> = message.encode_utf16().collect();
    let cutoff =
        BASE_CHAT_CUTOFF.wrapping_sub(clan_length.wrapping_add(if rank_present { 9 } else { 0 }));
    let mut counter: i32 = 0;
    for token in kotlin_split(message, " ") {
        let length = units(token);
        if counter.wrapping_add(length) > cutoff {
            break;
        }
        counter = counter.wrapping_add(length).wrapping_add(1);
    }
    let first = (counter as usize).min(all.len());
    let mut messages = vec![String::from_utf16_lossy(&all[..first])];
    if (counter as usize) < all.len() {
        messages.push(String::from_utf16_lossy(&all[counter as usize..]));
    }
    messages
}

/// `core.tools.colorize(line)`: the six letter codes, then `%` and six hex
/// digits, become color tags; `</col>` and a space close the line.
pub fn colorize(line: &str) -> String {
    let replaced = line
        .replace("%R", "<col=ff0000>")
        .replace("%O", "<col=ff6600>")
        .replace("%Y", "<col=ffff00>")
        .replace("%G", "<col=66ff33>")
        .replace("%B", "<col=3366ff>")
        .replace("%P", "<col=cc00ff>");
    let chars: Vec<char> = replaced.chars().collect();
    let mut out = String::with_capacity(replaced.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%'
            && i + 7 <= chars.len()
            && chars[i + 1..i + 7].iter().all(char::is_ascii_hexdigit)
        {
            out.push_str("<col=");
            out.extend(&chars[i + 1..i + 7]);
            out.push('>');
            i += 7;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out + "</col>" + " "
}

/// `GlobalChat.prepare(sender, message, isResizable, rights)`.
pub fn prepare_global(sender: &str, message: &str, resizable: bool, rights: i32) -> String {
    let base = if resizable { "%f1b04c" } else { "%7512ff" };
    let bracket = if resizable { "%ffffff" } else { "%000000" };
    let icon = if rights > 0 {
        format!("<img={}>", rights - 1)
    } else {
        String::new()
    };
    colorize(&format!(
        "{bracket}[{base}G{bracket}] {icon}{sender}: {base}{message}"
    ))
}

/// `getAttribute(player, ATTR_GLOBAL_MUTE, false)`.
fn global_muted<P, H: Host<P>>(host: &mut H, player: &P) -> Result<bool> {
    match host.get_attribute(player, ATTR_GLOBAL_MUTE)? {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(value),
        Some(_) => Err(Error::ClassCast),
    }
}

impl<P: Clone + Eq + Hash> Social<P> {
    /// The social branches of `PacketProcessor.process`. The decoder's
    /// failures and unknown opcodes are not processed here (the original logs
    /// them as unprocessed packets).
    pub fn process<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        packet: &Decoded,
    ) -> Result<()> {
        match packet {
            Decoded::AddFriend { username } => self.add(host, player, username),
            Decoded::RemoveFriend { username } => self.remove(host, player, username, false),
            Decoded::AddIgnore { username } => self.block(host, player, username),
            Decoded::RemoveIgnore { username } => self.remove(host, player, username, true),
            Decoded::JoinClan { clan_name } => {
                let current = host.with_details(player, |d| {
                    d.communication.current_clan.clone().unwrap_or_default()
                });
                if clan_name.is_empty() && !current.is_empty() {
                    let username = host.name(player);
                    self.publish(Event::LeaveClanRequest {
                        clan_name: current,
                        username,
                    });
                    return Ok(());
                }
                host.api_message(player, "Attempting to join channel....:clan:")?;
                let username = host.name(player);
                self.publish(Event::JoinClanRequest {
                    clan_name: clan_name.clone(),
                    username,
                });
                Ok(())
            }
            Decoded::SetClanRank { username, rank } => {
                let rank = rank_at(i64::from(*rank) + 1)?;
                self.update_clan_rank(host, player, username, Some(rank))
            }
            Decoded::KickFromClan { username } => {
                let Some(clan) = self.clan_of(player) else {
                    return Ok(());
                };
                let Some(target) = host.player_by_name(username) else {
                    return Ok(());
                };
                self.clan_kick(host, clan, player, &target)
            }
            Decoded::ChatMessage { effects, message } => {
                self.public_chat(host, player, *effects, message)
            }
            Decoded::ChatSetting {
                public,
                private,
                trade,
            } => {
                self.send_chat_setting(host, player, *public, *private, *trade)?;
                host.update_chat_settings(player, *public, *private, *trade)
            }
            Decoded::PrivateMessage { username, message } => {
                if is_muted(host, player) {
                    host.player_message(player, "You have been muted due to breaking a rule.")?;
                } else {
                    self.send_message(host, player, username, message)?;
                }
                host.log_private_chat(player, username, message)
            }
            Decoded::DecodingError(_) | Decoded::UnhandledOp => Ok(()),
        }
    }

    /// The `ChatMessage` branch: muted players are told so; with global chat
    /// on, `//` goes to everyone (unless the sender muted global chat), each
    /// part logged when not blank; `/` with a clan publishes each non-blank
    /// part as a clan message; anything else is logged and registered as the
    /// player's chat mask, its length in UTF-16 units.
    fn public_chat<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        effects: i32,
        message: &str,
    ) -> Result<()> {
        if is_muted(host, player) {
            return host.player_message(player, "You have been muted due to breaking a rule.");
        }
        if host.global_chat_enabled() && message.starts_with("//") {
            if global_muted(host, player)? {
                return Ok(());
            }
            let length = units(&host.name(player)).wrapping_add(3);
            for part in split_chat_message(&message[2..], length, false) {
                if !is_blank(&part) {
                    host.log_chat(player, "global", &part)?;
                }
                let username = format_display_name(&host.name(player));
                let icon = chat_icon(host, player)?;
                self.global_process(host, &username, &part, icon)?;
            }
            return Ok(());
        } else if message.starts_with('/') && self.clan_of(player).is_some() {
            let clan = self.clan_of(player).expect("checked");
            let length = units(&self.clans[clan.0].name).wrapping_add(units(&host.name(player)));
            let rank_present = rights(host, player)?.ordinal() != 0;
            for part in split_chat_message(&message[1..], length, rank_present) {
                if is_blank(&part) {
                    continue;
                }
                let sender = host.name(player);
                let clan = self.clan_of(player).ok_or(Error::NullPointer)?;
                let clan_name = lowercase_root(&self.clans[clan.0].owner).replace(' ', "_");
                let rank = chat_icon(host, player)?;
                host.log_chat(player, "clan", &part)?;
                self.publish(Event::ClanMessage {
                    sender,
                    clan_name,
                    message: part,
                    rank,
                });
            }
            return Ok(());
        }
        host.log_chat(player, "public", message)?;
        let context = ChatMessage::new(host, player, message, effects, units(message))?;
        host.register_chat(player, &context, false)?;
        Ok(())
    }

    /// `GlobalChat.process(sender, message, rights)`: every listed player who
    /// has not muted global chat gets the message prepared for its window mode.
    pub fn global_process<H: Host<P>>(
        &mut self,
        host: &mut H,
        sender: &str,
        message: &str,
        rights: i32,
    ) -> Result<()> {
        let standard = prepare_global(sender, message, false, rights);
        let resizable = prepare_global(sender, message, true, rights);
        let mut listeners = Vec::new();
        for player in host.players() {
            if !global_muted(host, &player)? {
                listeners.push(player);
            }
        }
        for player in listeners {
            if host.is_resizable(&player) {
                host.api_message(&player, &resizable)?;
            } else {
                host.api_message(&player, &standard)?;
            }
        }
        Ok(())
    }

    /// `GlobalChat.defineCommands()`: the `muteglobal` command.
    pub fn define_global_commands<H: Host<P>>(&mut self, host: &mut H) -> Result<()> {
        host.define_command(
            "muteglobal",
            "STANDARD",
            "",
            "Toggles global chat on or off.",
        )
    }

    /// The `muteglobal` command: the saved attribute flips, and the player is
    /// told the state it had before.
    pub fn mute_global<H: Host<P>>(&mut self, host: &mut H, player: &P) -> Result<()> {
        let original = global_muted(host, player)?;
        host.set_attribute(player, ATTR_GLOBAL_MUTE, Value::Bool(!original))?;
        host.api_message(
            player,
            &format!(
                "Global chat is now {}.",
                if original { "ON" } else { "OFF" }
            ),
        )
    }
}
