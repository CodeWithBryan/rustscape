//! FriendsList, IgnoreList, ClanChat and StockMarketManager, with the server
//! reducers from the pinned RT4 `Protocol.readPacketInternal`.
//!
//! This holds client presentation state only. The server remains the authority
//! on membership; client requests leave through the original packets.
use super::vm::ChatLine;
use crate::player_interaction::username;
use rs09_protocol::{
    game::{
        Frame,
        events::{ClanMember as WireMember, Offer, Sender, ServerEvent},
    },
    login::{decode_name, encode_name},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Friend {
    pub key: i64,
    /// Title-cased display name.
    pub name: String,
    pub world: i32,
    pub world_name: String,
    pub rank: i32,
    pub same_game: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ignore {
    pub key: i64,
    /// Base-37 text without title casing, as IgnoreList stores it.
    pub name: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanMember {
    pub key: i64,
    /// Base-37 text without title casing; scripts title-case on read.
    pub name: String,
    pub world: i32,
    pub rank: i32,
    pub world_name: String,
}

/// Which original `transmitAt` stamp a reducer refreshed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Transmit {
    pub friends: bool,
    pub clan: bool,
    pub stock: bool,
}

/// The local player and session facts the original reads from statics.
#[derive(Clone, Copy, Debug)]
pub struct Local<'a> {
    pub key: i64,
    /// Title-cased `PlayerList.self.username`.
    pub name: &'a str,
    pub world: i32,
    pub members: bool,
    pub tutorial: bool,
    /// `playerUnderage && !parentalChatConsent || mapQuickChat`.
    pub restricted: bool,
}

pub struct Social {
    pub friend_state: i32,
    pub friends: Vec<Friend>,
    pub ignores: Vec<Ignore>,
    pub clan_name: Option<String>,
    pub clan_owner: Option<String>,
    /// `ClanChat.members[..size]`. The array itself survives leaving a channel.
    pub clan_members: Vec<ClanMember>,
    clan_array: bool,
    pub clan_min_kick: i32,
    pub clan_rank: i32,
    recent: [i64; 100],
    counter: usize,
    pub offers: [Offer; 6],
}
impl Default for Social {
    fn default() -> Self {
        Self {
            friend_state: 0,
            friends: vec![],
            ignores: vec![],
            clan_name: None,
            clan_owner: None,
            clan_members: vec![],
            clan_array: false,
            clan_min_kick: 0,
            clan_rank: 0,
            recent: [0; 100],
            counter: 0,
            offers: [EMPTY_OFFER; 6],
        }
    }
}
impl Social {
    /// What `client.resetGameSessionState` clears at every login:
    /// `FriendsList.state` and `size`, `ClanChat.size`, `members` and `name`,
    /// and the six Grand Exchange offers. The ignore list, the clan owner,
    /// rank and kick threshold and the recent-message ring are left alone.
    pub fn reset_session(&mut self) {
        self.friend_state = 0;
        self.friends.clear();
        self.clan_members.clear();
        self.clan_array = false;
        self.clan_name = None;
        self.offers = [EMPTY_OFFER; 6];
    }
}
const EMPTY_OFFER: Offer = Offer {
    status: 0,
    item: 0,
    price: 0,
    count: 0,
    completed_count: 0,
    completed_gold: 0,
};

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}
/// `Base37.decode37`: None for the values the original rejects.
fn decode(key: i64) -> Option<String> {
    (key > 0 && key < 6_582_952_005_840_035_281 && key % 37 != 0).then(|| decode_name(key))
}
fn title(key: i64) -> String {
    username(key).unwrap_or_default()
}
/// `JagString.toTitleCase` of base-37 text: underscores become spaces and the
/// first letter after each is capitalised.
pub fn title_case(name: &str) -> String {
    let mut capitalize = true;
    name.chars()
        .map(|c| {
            if c == '_' {
                capitalize = true;
                ' '
            } else {
                let out = if capitalize && c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                capitalize = false;
                out
            }
        })
        .collect()
}
/// `JagString.encode37` of a Latin-1 script string.
pub fn encode(name: &str) -> i64 {
    let bytes: Vec<u8> = name.chars().map(|c| c as u32 as u8).collect();
    encode_name(&bytes)
}
/// `JagString.equalsIgnoreCase`: ASCII and Latin-1 capitals fold, except the
/// multiplication sign. Underscores and spaces stay distinct, as in the original.
fn eq_ignore_case(a: &str, b: &str) -> bool {
    fn fold(c: char) -> char {
        match c as u32 {
            65..=90 | 192..=214 | 216..=222 => char::from_u32(c as u32 + 32).unwrap_or(c),
            _ => c,
        }
    }
    a.chars().count() == b.chars().count()
        && a.chars().zip(b.chars()).all(|(x, y)| fold(x) == fold(y))
}
/// Scripts pass names that may carry a moderator crown.
fn without_crown(name: &str) -> &str {
    if name.starts_with("<img=0>") || name.starts_with("<img=1>") {
        &name[7..]
    } else {
        name
    }
}
fn crowned(rights: i32, staff_three: bool, name: String) -> String {
    match rights {
        2 => format!("<img=1>{name}"),
        3 if staff_three => format!("<img=1>{name}"),
        1 => format!("<img=0>{name}"),
        _ => name,
    }
}
fn system(text: String) -> ChatLine {
    ChatLine {
        kind: 0,
        text,
        name: String::new(),
        clan: String::new(),
        phrase: -1,
    }
}
/// `Font.escape(decodeHuffmanMessage(..).encodeMessage())`, or the original
/// fallback word when the body cannot be decoded.
pub(super) fn message(body: &[u8]) -> String {
    let Ok(raw) = rs09_protocol::game::chat::public_text(body) else {
        return "Cabbage".into();
    };
    let mut out = String::with_capacity(raw.len());
    for &b in &raw {
        match b {
            b'<' => out.push_str("<lt>"),
            b'>' => out.push_str("<gt>"),
            b => out.push(char::from(b)),
        }
    }
    out
}

impl Social {
    /// The duplicate check precedes the ignore check, and a staff sender
    /// (rights above 1) bypasses only the ignore/restriction branch.
    fn blocked(&self, sender: Sender, local: Local<'_>, restriction_applies: bool) -> bool {
        if self.recent.contains(&sender.message_id) {
            return true;
        }
        if sender.rights <= 1 {
            if restriction_applies && local.restricted {
                return true;
            }
            return self.ignores.iter().any(|i| i.key == sender.name);
        }
        false
    }
    fn remember(&mut self, sender: Sender) {
        self.recent[self.counter] = sender.message_id;
        self.counter = (self.counter + 1) % 100;
    }

    /// Whether a quick chat message passes the duplicate, ignore and tutorial
    /// checks. Unlike typed chat, the chat restriction does not apply, and an
    /// echo of the player's own message is always shown.
    pub fn quick_chat_wanted(&self, event: &ServerEvent, local: Local<'_>) -> bool {
        match event {
            ServerEvent::PrivateQuickChat { sender, .. }
            | ServerEvent::ClanQuickChat { sender, .. } => {
                !self.blocked(*sender, local, false) && !local.tutorial
            }
            ServerEvent::PrivateQuickChatEcho { .. } => true,
            _ => false,
        }
    }
    /// The chat line for an accepted quick chat message whose phrase text has
    /// been produced. Kinds are the original's 18, 19 and 20.
    pub fn quick_chat(&mut self, event: &ServerEvent, text: String) -> Option<ChatLine> {
        match event {
            ServerEvent::PrivateQuickChat { sender, phrase, .. } => {
                self.remember(*sender);
                Some(ChatLine {
                    kind: 18,
                    text,
                    name: crowned(sender.rights, false, title(sender.name)),
                    clan: String::new(),
                    phrase: *phrase,
                })
            }
            ServerEvent::PrivateQuickChatEcho { name, phrase, .. } => Some(ChatLine {
                kind: 19,
                text,
                name: title(*name),
                clan: String::new(),
                phrase: *phrase,
            }),
            ServerEvent::ClanQuickChat {
                sender,
                clan,
                phrase,
                ..
            } => {
                self.remember(*sender);
                Some(ChatLine {
                    kind: 20,
                    text,
                    name: crowned(sender.rights, true, title(sender.name)),
                    clan: title(*clan),
                    phrase: *phrase,
                })
            }
            _ => None,
        }
    }

    /// Returns None for events this reducer does not own.
    pub fn apply(
        &mut self,
        event: &ServerEvent,
        local: Local<'_>,
    ) -> Option<(Vec<ChatLine>, Transmit)> {
        let mut lines = vec![];
        let mut transmit = Transmit::default();
        match event {
            ServerEvent::PrivateMessage { sender, body } => {
                if !self.blocked(*sender, local, true) && !local.tutorial {
                    self.remember(*sender);
                    let text = message(body);
                    let name = title(sender.name);
                    lines.push(ChatLine {
                        kind: if matches!(sender.rights, 1..=3) { 7 } else { 3 },
                        text,
                        name: crowned(sender.rights, true, name),
                        clan: String::new(),
                        phrase: -1,
                    });
                }
            }
            ServerEvent::PrivateEcho { name, body } => lines.push(ChatLine {
                kind: 6,
                text: message(body),
                name: title(*name),
                clan: String::new(),
                phrase: -1,
            }),
            ServerEvent::ClanMessage { sender, clan, body } => {
                if !self.blocked(*sender, local, true) && !local.tutorial {
                    self.remember(*sender);
                    lines.push(ChatLine {
                        kind: 9,
                        text: message(body),
                        name: crowned(sender.rights, true, title(sender.name)),
                        clan: title(*clan),
                        phrase: -1,
                    });
                }
            }
            ServerEvent::ClanLeft => {
                self.clan_owner = None;
                self.clan_name = None;
                self.clan_members.clear();
                self.clan_array = false;
                transmit.clan = true;
            }
            ServerEvent::ClanChannel {
                owner,
                name,
                min_kick,
                members,
            } => {
                transmit.clan = true;
                self.clan_name = decode(*name);
                self.clan_owner = decode(*owner);
                self.clan_min_kick = *min_kick;
                if let Some(members) = members {
                    self.clan_members = members.iter().map(member).collect();
                    for m in &self.clan_members {
                        if m.key == local.key {
                            self.clan_rank = m.rank;
                        }
                    }
                    // Stable bubble sort on the raw name, as the original performs.
                    self.clan_members.sort_by(|a, b| compare(&a.name, &b.name));
                    self.clan_array = true;
                }
            }
            ServerEvent::ClanMemberLeft { name, world } => {
                if self.clan_members.is_empty() {
                    return Some((lines, transmit));
                }
                if let Some(i) = self
                    .clan_members
                    .iter()
                    .position(|m| m.key == *name && m.world == *world)
                {
                    self.clan_members.remove(i);
                }
                transmit.clan = true;
            }
            ServerEvent::ClanMemberJoined(wire) => {
                let new = member(wire);
                let mut insert = 0;
                for n in (0..self.clan_members.len()).rev() {
                    match compare(&self.clan_members[n].name, &new.name) {
                        std::cmp::Ordering::Equal => {
                            let old = &mut self.clan_members[n];
                            old.world = new.world;
                            old.rank = new.rank;
                            old.world_name = new.world_name;
                            if new.key == local.key {
                                self.clan_rank = new.rank;
                            }
                            transmit.clan = true;
                            return Some((lines, transmit));
                        }
                        std::cmp::Ordering::Less => {
                            insert = n + 1;
                            break;
                        }
                        std::cmp::Ordering::Greater => {}
                    }
                }
                // A full 100-entry array drops the member without a transmit.
                if self.clan_members.len() >= 100 {
                    return Some((lines, transmit));
                }
                if new.key == local.key {
                    self.clan_rank = new.rank;
                }
                self.clan_members.insert(insert, new);
                self.clan_array = true;
                transmit.clan = true;
            }
            ServerEvent::Friend {
                name,
                world,
                rank,
                same_game,
                world_name,
            } => {
                let display = title(*name);
                let world_name = latin1(world_name);
                if let Some(friend) = self.friends.iter_mut().find(|f| f.key == *name) {
                    if friend.world != *world {
                        friend.world = *world;
                        if *world > 0 {
                            lines.push(friend_notice(format!("{display} has logged in.")));
                        }
                        if *world == 0 {
                            lines.push(friend_notice(format!("{display} has logged out.")));
                        }
                    }
                    friend.world_name = world_name;
                    friend.rank = *rank;
                    friend.same_game = *same_game;
                } else if self.friends.len() < 200 {
                    self.friends.push(Friend {
                        key: *name,
                        name: display,
                        world: *world,
                        world_name,
                        rank: *rank,
                        same_game: *same_game,
                    });
                }
                transmit.friends = true;
                self.sort_friends(local.world);
            }
            ServerEvent::FriendListState(state) => {
                self.friend_state = *state;
                transmit.friends = true;
            }
            ServerEvent::IgnoreList(names) => {
                self.ignores = names
                    .iter()
                    .take(100)
                    .map(|&key| Ignore {
                        key,
                        name: decode(key).unwrap_or_default(),
                    })
                    .collect();
                transmit.friends = true;
            }
            ServerEvent::ExchangeOffer { slot, offer } => {
                // Slots outside the six-entry array throw in the original.
                let entry = self.offers.get_mut(usize::try_from(*slot).ok()?)?;
                *entry = offer.unwrap_or(EMPTY_OFFER);
                transmit.stock = true;
            }
            _ => return None,
        }
        Some((lines, transmit))
    }

    /// The original bubble pass: same-world friends first, offline friends last.
    fn sort_friends(&mut self, world: i32) {
        let mut count = self.friends.len();
        while count > 0 {
            count -= 1;
            let mut sorted = true;
            for i in 0..count {
                let (a, b) = (self.friends[i].world, self.friends[i + 1].world);
                if (a != world && b == world) || (a == 0 && b != 0) {
                    sorted = false;
                    self.friends.swap(i, i + 1);
                }
            }
            if sorted {
                break;
            }
        }
    }

    pub fn is_friend(&self, name: &str, local: Local<'_>) -> bool {
        let name = without_crown(name);
        self.friends.iter().any(|f| eq_ignore_case(&f.name, name))
            || eq_ignore_case(name, local.name)
    }
    pub fn friend_index(&self, name: &str) -> i32 {
        let name = without_crown(name);
        self.friends
            .iter()
            .position(|f| eq_ignore_case(&f.name, name))
            .map_or(-1, |i| i as i32)
    }
    pub fn is_ignored(&self, name: &str) -> bool {
        let name = without_crown(name);
        self.ignores.iter().any(|i| eq_ignore_case(&i.name, name))
    }
    pub fn friend(&self, index: i32) -> Option<&Friend> {
        if self.friend_state != 2 {
            return None;
        }
        self.friends.get(usize::try_from(index).ok()?)
    }
    pub fn clan_member(&self, index: i32) -> Option<&ClanMember> {
        self.clan_name.as_ref()?;
        self.clan_members.get(usize::try_from(index).ok()?)
    }
    /// CS2 3624 checks the member array, not the channel name.
    pub fn clan_member_is(&self, index: i32, name: &str) -> bool {
        self.clan_array
            && usize::try_from(index)
                .ok()
                .and_then(|i| self.clan_members.get(i))
                .is_some_and(|m| eq_ignore_case(&m.name, name))
    }

    /// `FriendsList.add`: a message for each refusal, otherwise the local
    /// entry and FRIENDLIST_ADD.
    pub fn add_friend(&mut self, key: i64, local: Local<'_>) -> (Option<ChatLine>, Option<Frame>) {
        if key == 0 {
            return (None, None);
        }
        if (self.friends.len() >= 100 && !local.members) || self.friends.len() >= 200 {
            return (
                Some(system(
                    "Your friend list is full. Max of 100 for free users, and 200 for members."
                        .into(),
                )),
                None,
            );
        }
        let name = title(key);
        if self.friends.iter().any(|f| f.key == key) {
            return (
                Some(system(format!("{name} is already on your friend list."))),
                None,
            );
        }
        if self.ignores.iter().any(|i| i.key == key) {
            return (
                Some(system(format!(
                    "Please remove {name} from your ignore list first."
                ))),
                None,
            );
        }
        if name == local.name {
            return (
                Some(system(
                    "You can't add yourself to your own friend list.".into(),
                )),
                None,
            );
        }
        self.friends.push(Friend {
            key,
            name,
            world: 0,
            world_name: String::new(),
            rank: 0,
            same_game: false,
        });
        (None, Some(name_frame(120, key)))
    }
    pub fn remove_friend(&mut self, key: i64) -> Option<Frame> {
        if key == 0 {
            return None;
        }
        let i = self.friends.iter().position(|f| f.key == key)?;
        self.friends.remove(i);
        Some(name_frame(57, key))
    }
    pub fn add_ignore(&mut self, key: i64, local: Local<'_>) -> (Option<ChatLine>, Option<Frame>) {
        if key == 0 {
            return (None, None);
        }
        if self.ignores.len() >= 100 {
            return (
                Some(system("Your ignore list is full. Max of 100 users.".into())),
                None,
            );
        }
        let name = title(key);
        if self.ignores.iter().any(|i| i.key == key) {
            return (
                Some(system(format!("{name} is already on your ignore list."))),
                None,
            );
        }
        if self.friends.iter().any(|f| f.key == key) {
            return (
                Some(system(format!(
                    "Please remove {name} from your friend list first."
                ))),
                None,
            );
        }
        if name == local.name {
            return (
                Some(system(
                    "You can't add yourself to your own ignore list.".into(),
                )),
                None,
            );
        }
        self.ignores.push(Ignore {
            key,
            name: decode(key).unwrap_or_default(),
        });
        (None, Some(name_frame(34, key)))
    }
    pub fn remove_ignore(&mut self, key: i64) -> Option<Frame> {
        if key == 0 {
            return None;
        }
        let i = self.ignores.iter().position(|f| f.key == key)?;
        self.ignores.remove(i);
        Some(name_frame(213, key))
    }
    /// `ClanChat.kick` scans the whole member array for the encoded name.
    pub fn kick(&self, name: &str) -> Option<Frame> {
        if !self.clan_array {
            return None;
        }
        let key = encode(name);
        if key == 0 {
            return None;
        }
        self.clan_members
            .iter()
            .find(|m| m.key == key)
            .map(|m| name_frame(162, m.key))
    }
}

fn friend_notice(text: String) -> ChatLine {
    ChatLine {
        kind: 5,
        text,
        name: String::new(),
        clan: String::new(),
        phrase: -1,
    }
}
fn member(wire: &WireMember) -> ClanMember {
    ClanMember {
        key: wire.name,
        name: decode(wire.name).unwrap_or_default(),
        world: wire.world,
        rank: wire.rank,
        world_name: latin1(&wire.world_name),
    }
}
/// `JagString.compareTo`: unsigned bytes in order, then the shorter string
/// first. This is not the collating `JagString.compare` that scripts call.
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars().map(|c| c as u32).cmp(b.chars().map(|c| c as u32))
}
fn name_frame(opcode: u8, key: i64) -> Frame {
    Frame {
        opcode,
        payload: key.to_be_bytes().to_vec(),
    }
}
/// FRIEND_SETRANK: p1add(rank), p8(encode37(name)).
pub fn set_rank(name: &str, rank: i32) -> Frame {
    let mut payload = vec![rank.wrapping_add(128) as u8];
    payload.extend(encode(name).to_be_bytes());
    Frame {
        opcode: 188,
        payload,
    }
}
/// CLAN_JOINCHAT_LEAVECHAT; a zero name leaves and is sent only by `leave`.
pub fn join_clan(name: &str) -> Option<Frame> {
    let key = encode(name);
    (key != 0).then(|| name_frame(104, key))
}
pub fn leave_clan() -> Frame {
    name_frame(104, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::{Direction, Size, chat::WORDPACK};

    const ME: Local<'static> = Local {
        key: 0,
        name: "Zezima",
        world: 2,
        members: false,
        tutorial: false,
        restricted: false,
    };
    fn me() -> Local<'static> {
        Local {
            key: encode("zezima"),
            ..ME
        }
    }
    fn body(text: &[u8]) -> Vec<u8> {
        let mut out = vec![text.len() as u8];
        out.extend(WORDPACK.encode(text).unwrap());
        out
    }
    fn sender(name: &str, id: i64, rights: i32) -> Sender {
        Sender {
            name: encode(name),
            message_id: id,
            rights,
        }
    }
    fn friend(name: &str, world: i32) -> ServerEvent {
        ServerEvent::Friend {
            name: encode(name),
            world,
            rank: 0,
            same_game: true,
            world_name: if world > 0 {
                format!("World {world}").into_bytes()
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn private_messages_suppress_duplicates_ignores_and_restrictions() {
        let mut s = Social::default();
        let event = ServerEvent::PrivateMessage {
            sender: sender("bob_the cat", 77, 0),
            body: body(b"hello <there>"),
        };
        let (lines, transmit) = s.apply(&event, me()).unwrap();
        assert_eq!(transmit, Transmit::default());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].kind, 3);
        assert_eq!(lines[0].name, "Bob The Cat");
        assert_eq!(lines[0].text, "Hello <lt>there<gt>");
        // The same message ID is recorded and dropped the second time.
        assert!(s.apply(&event, me()).unwrap().0.is_empty());

        let ignored = ServerEvent::PrivateMessage {
            sender: sender("bob_the cat", 78, 1),
            body: body(b"again"),
        };
        s.ignores.push(Ignore {
            key: encode("bob_the cat"),
            name: "bob_the_cat".into(),
        });
        assert!(s.apply(&ignored, me()).unwrap().0.is_empty());
        // Rights above one bypass the ignore list and carry the gold crown.
        let staff = ServerEvent::PrivateMessage {
            sender: sender("bob_the cat", 78, 2),
            body: body(b"again"),
        };
        let (lines, _) = s.apply(&staff, me()).unwrap();
        assert_eq!(
            (lines[0].kind, lines[0].name.as_str()),
            (7, "<img=1>Bob The Cat")
        );

        let restricted = Local {
            restricted: true,
            ..me()
        };
        let other = ServerEvent::PrivateMessage {
            sender: sender("carol", 90, 0),
            body: body(b"hi"),
        };
        assert!(s.apply(&other, restricted).unwrap().0.is_empty());
        let tutorial = Local {
            tutorial: true,
            ..me()
        };
        assert!(s.apply(&other, tutorial).unwrap().0.is_empty());
        // Neither refusal recorded the ID, so the message is still deliverable.
        assert_eq!(s.apply(&other, me()).unwrap().0.len(), 1);
    }

    #[test]
    fn recent_message_ring_holds_one_hundred_ids() {
        let mut s = Social::default();
        for id in 1..=101 {
            let event = ServerEvent::PrivateMessage {
                sender: sender("carol", id, 0),
                body: body(b"x"),
            };
            assert_eq!(s.apply(&event, me()).unwrap().0.len(), 1, "{id}");
        }
        let replay = |s: &mut Social, id| {
            s.apply(
                &ServerEvent::PrivateMessage {
                    sender: sender("carol", id, 0),
                    body: body(b"x"),
                },
                me(),
            )
            .unwrap()
            .0
            .len()
        };
        // ID 1 was overwritten by the 101st message; ID 2 is still remembered.
        assert_eq!(replay(&mut s, 2), 0);
        assert_eq!(replay(&mut s, 1), 1);
    }

    #[test]
    fn echo_and_clan_lines_and_undecodable_body() {
        let mut s = Social::default();
        let (lines, _) = s
            .apply(
                &ServerEvent::PrivateEcho {
                    name: encode("carol"),
                    body: body(b"to you"),
                },
                me(),
            )
            .unwrap();
        assert_eq!(
            (
                lines[0].kind,
                lines[0].name.as_str(),
                lines[0].text.as_str()
            ),
            (6, "Carol", "To you")
        );
        let (lines, _) = s
            .apply(
                &ServerEvent::ClanMessage {
                    sender: sender("carol", 5, 1),
                    clan: encode("the_clan"),
                    body: body(b"welcome"),
                },
                me(),
            )
            .unwrap();
        assert_eq!(
            (
                lines[0].kind,
                lines[0].name.as_str(),
                lines[0].clan.as_str(),
                lines[0].text.as_str()
            ),
            (9, "<img=0>Carol", "The Clan", "Welcome")
        );
        let (lines, _) = s
            .apply(
                &ServerEvent::PrivateEcho {
                    name: encode("carol"),
                    body: vec![],
                },
                me(),
            )
            .unwrap();
        assert_eq!(lines[0].text, "Cabbage");
    }

    #[test]
    fn friend_updates_announce_world_changes_and_keep_the_original_order() {
        let mut s = Social::default();
        for (name, world) in [("alice", 0), ("bob", 5), ("carol", 2), ("dave", 0)] {
            let (lines, transmit) = s.apply(&friend(name, world), me()).unwrap();
            assert!(lines.is_empty(), "a new friend is not announced");
            assert!(transmit.friends);
        }
        // Same world first, then other worlds, then offline; ties keep their order.
        let order = |s: &Social| s.friends.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        assert_eq!(order(&s), ["Carol", "Bob", "Alice", "Dave"]);
        let (lines, _) = s.apply(&friend("dave", 2), me()).unwrap();
        assert_eq!(lines[0].kind, 5);
        assert_eq!(lines[0].text, "Dave has logged in.");
        assert_eq!(order(&s), ["Carol", "Dave", "Bob", "Alice"]);
        let (lines, _) = s.apply(&friend("carol", 0), me()).unwrap();
        assert_eq!(lines[0].text, "Carol has logged out.");
        assert_eq!(order(&s), ["Dave", "Bob", "Carol", "Alice"]);
        // An unchanged world updates fields without a notice.
        let (lines, _) = s.apply(&friend("bob", 5), me()).unwrap();
        assert!(lines.is_empty());
        assert_eq!(s.friends[1].world_name, "World 5");
    }

    #[test]
    fn friend_queries_require_the_loaded_state() {
        let mut s = Social::default();
        s.apply(&friend("bob", 5), me()).unwrap();
        assert!(s.friend(0).is_none());
        s.apply(&ServerEvent::FriendListState(2), me()).unwrap();
        assert_eq!(s.friend(0).unwrap().name, "Bob");
        assert!(s.friend(1).is_none() && s.friend(-1).is_none());
        assert!(s.is_friend("<img=1>BOB", me()));
        assert!(s.is_friend("zezima", me()), "the local player counts");
        assert!(!s.is_friend("carol", me()));
        assert_eq!(s.friend_index("<img=0>bob"), 0);
        assert_eq!(s.friend_index("carol"), -1);
    }

    #[test]
    fn friend_and_ignore_requests_follow_the_original_refusals() {
        let mut s = Social::default();
        let key = encode("bob");
        assert_eq!(s.add_friend(0, me()), (None, None));
        let (line, frame) = s.add_friend(key, me());
        assert!(line.is_none());
        let frame = frame.unwrap();
        assert_eq!(
            (frame.opcode, frame.payload.as_slice()),
            (120, &key.to_be_bytes()[..])
        );
        let text = |r: (Option<ChatLine>, Option<Frame>)| {
            assert!(r.1.is_none());
            r.0.unwrap().text
        };
        assert_eq!(
            text(s.add_friend(key, me())),
            "Bob is already on your friend list."
        );
        assert_eq!(
            text(s.add_ignore(key, me())),
            "Please remove Bob from your friend list first."
        );
        assert_eq!(
            text(s.add_friend(me().key, me())),
            "You can't add yourself to your own friend list."
        );
        assert_eq!(
            text(s.add_ignore(me().key, me())),
            "You can't add yourself to your own ignore list."
        );
        let carol = encode("carol");
        assert_eq!(s.add_ignore(carol, me()).1.unwrap().opcode, 34);
        assert_eq!(s.ignores[0].name, "carol");
        assert!(s.is_ignored("<img=0>Carol"));
        assert_eq!(
            text(s.add_ignore(carol, me())),
            "Carol is already on your ignore list."
        );
        assert_eq!(
            text(s.add_friend(carol, me())),
            "Please remove Carol from your ignore list first."
        );
        assert_eq!(s.remove_ignore(carol).unwrap().opcode, 213);
        assert!(s.remove_ignore(carol).is_none());
        assert_eq!(s.remove_friend(key).unwrap().opcode, 57);
        assert!(s.remove_friend(key).is_none() && s.remove_friend(0).is_none());

        for i in 0..100 {
            s.friends.push(Friend {
                key: 1000 + i,
                name: String::new(),
                world: 0,
                world_name: String::new(),
                rank: 0,
                same_game: false,
            });
        }
        assert!(text(s.add_friend(key, me())).starts_with("Your friend list is full."));
        let member = Local {
            members: true,
            ..me()
        };
        assert!(s.add_friend(key, member).1.is_some());
    }

    #[test]
    fn clan_channel_members_sort_update_and_leave() {
        let mut s = Social::default();
        let wire = |name: &str, world, rank| WireMember {
            name: encode(name),
            world,
            rank,
            world_name: format!("W{world}").into_bytes(),
        };
        let local = me();
        let (_, transmit) = s
            .apply(
                &ServerEvent::ClanChannel {
                    owner: encode("owner"),
                    name: encode("my_clan"),
                    min_kick: 3,
                    members: Some(vec![
                        wire("mallory", 1, 0),
                        wire("zezima", 2, 6),
                        wire("alice", 3, 1),
                    ]),
                },
                local,
            )
            .unwrap();
        assert!(transmit.clan);
        let names = |s: &Social| {
            s.clan_members
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&s), ["alice", "mallory", "zezima"]);
        assert_eq!((s.clan_rank, s.clan_min_kick), (6, 3));
        assert_eq!(s.clan_name.as_deref(), Some("my_clan"));
        assert!(s.clan_member_is(2, "Zezima") && !s.clan_member_is(0, "Zezima"));

        // The 255 size keeps the existing members and only refreshes the header.
        s.apply(
            &ServerEvent::ClanChannel {
                owner: encode("owner"),
                name: encode("renamed"),
                min_kick: 4,
                members: None,
            },
            local,
        )
        .unwrap();
        assert_eq!((names(&s).len(), s.clan_min_kick), (3, 4));

        s.apply(&ServerEvent::ClanMemberJoined(wire("bob", 9, 2)), local)
            .unwrap();
        assert_eq!(names(&s), ["alice", "bob", "mallory", "zezima"]);
        // An existing name is updated in place.
        s.apply(&ServerEvent::ClanMemberJoined(wire("zezima", 7, 5)), local)
            .unwrap();
        assert_eq!((s.clan_members[3].world, s.clan_rank), (7, 5));
        assert_eq!(names(&s).len(), 4);
        // Removal needs both the name and the world to match.
        s.apply(
            &ServerEvent::ClanMemberLeft {
                name: encode("bob"),
                world: 1,
            },
            local,
        )
        .unwrap();
        assert_eq!(names(&s).len(), 4);
        s.apply(
            &ServerEvent::ClanMemberLeft {
                name: encode("bob"),
                world: 9,
            },
            local,
        )
        .unwrap();
        assert_eq!(names(&s), ["alice", "mallory", "zezima"]);

        let kick = s.kick("Mallory").unwrap();
        assert_eq!(
            (kick.opcode, kick.payload),
            (162, encode("mallory").to_be_bytes().to_vec())
        );
        assert!(s.kick("nobody").is_none());
        s.apply(&ServerEvent::ClanLeft, local).unwrap();
        assert!(s.clan_name.is_none() && s.clan_members.is_empty());
        assert!(s.kick("Mallory").is_none() && s.clan_member(0).is_none());
    }

    #[test]
    fn clan_members_order_by_raw_bytes_not_by_collation() {
        // Bytewise, digits (48..57) sort before '_' (95), which sorts before
        // letters (97..). The collating comparison used by scripts differs.
        assert_eq!(compare("a_b", "ab"), std::cmp::Ordering::Less);
        assert_eq!(compare("a1", "a_"), std::cmp::Ordering::Less);
        assert_eq!(compare("ab", "abc"), std::cmp::Ordering::Less);
        assert_eq!(compare("abc", "abc"), std::cmp::Ordering::Equal);
        assert_eq!(compare("b", "abc"), std::cmp::Ordering::Greater);
        let mut s = Social::default();
        let members = ["zed", "a_b", "ab", "a1", "a"]
            .map(|name| WireMember {
                name: encode(name),
                world: 1,
                rank: 0,
                world_name: vec![],
            })
            .to_vec();
        s.apply(
            &ServerEvent::ClanChannel {
                owner: encode("owner"),
                name: encode("clan"),
                min_kick: 0,
                members: Some(members),
            },
            me(),
        )
        .unwrap();
        let names: Vec<&str> = s.clan_members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["a", "a1", "a_b", "ab", "zed"]);
    }

    #[test]
    fn quick_chat_lines_skip_the_restriction_but_keep_ignores_and_duplicates() {
        let mut s = Social::default();
        let restricted = Local {
            restricted: true,
            ..me()
        };
        let private = |id, rights| ServerEvent::PrivateQuickChat {
            sender: sender("bob", id, rights),
            phrase: 40,
            body: vec![],
        };
        // The typed-chat restriction does not block quick chat.
        assert!(s.quick_chat_wanted(&private(1, 0), restricted));
        let line = s.quick_chat(&private(1, 0), "Hello.".into()).unwrap();
        assert_eq!(
            (
                line.kind,
                line.name.as_str(),
                line.text.as_str(),
                line.phrase
            ),
            (18, "Bob", "Hello.", 40)
        );
        // The message ID is now remembered.
        assert!(!s.quick_chat_wanted(&private(1, 0), me()));
        // Rights 2 carry the gold crown; rights 3 do not for private quick chat.
        assert_eq!(
            s.quick_chat(&private(2, 2), String::new()).unwrap().name,
            "<img=1>Bob"
        );
        assert_eq!(
            s.quick_chat(&private(3, 3), String::new()).unwrap().name,
            "Bob"
        );
        s.ignores.push(Ignore {
            key: encode("bob"),
            name: "bob".into(),
        });
        assert!(!s.quick_chat_wanted(&private(9, 1), me()));
        assert!(s.quick_chat_wanted(&private(9, 2), me()));
        let tutorial = Local {
            tutorial: true,
            ..me()
        };
        assert!(!s.quick_chat_wanted(&private(9, 2), tutorial));

        let echo = ServerEvent::PrivateQuickChatEcho {
            name: encode("carol"),
            phrase: 7,
            body: vec![],
        };
        assert!(s.quick_chat_wanted(&echo, tutorial));
        let line = s.quick_chat(&echo, "Bye.".into()).unwrap();
        assert_eq!(
            (line.kind, line.name.as_str(), line.phrase),
            (19, "Carol", 7)
        );

        let clan = ServerEvent::ClanQuickChat {
            sender: sender("dave", 50, 3),
            clan: encode("my_clan"),
            phrase: 8,
            body: vec![],
        };
        assert!(s.quick_chat_wanted(&clan, me()));
        let line = s.quick_chat(&clan, "Hi.".into()).unwrap();
        assert_eq!(
            (line.kind, line.name.as_str(), line.clan.as_str()),
            (20, "<img=1>Dave", "My Clan")
        );
        assert!(!s.quick_chat_wanted(&ServerEvent::ResetAnimations, me()));
    }

    #[test]
    fn exchange_offers_and_other_events() {
        let mut s = Social::default();
        let offer = Offer {
            status: 0x0a,
            item: 4151,
            price: 9,
            count: 8,
            completed_count: 7,
            completed_gold: 6,
        };
        let (_, transmit) = s
            .apply(
                &ServerEvent::ExchangeOffer {
                    slot: 5,
                    offer: Some(offer),
                },
                me(),
            )
            .unwrap();
        assert!(transmit.stock);
        assert_eq!(s.offers[5], offer);
        s.apply(
            &ServerEvent::ExchangeOffer {
                slot: 5,
                offer: None,
            },
            me(),
        )
        .unwrap();
        assert_eq!(s.offers[5], EMPTY_OFFER);
        assert!(
            s.apply(
                &ServerEvent::ExchangeOffer {
                    slot: 6,
                    offer: None
                },
                me()
            )
            .is_none()
        );
        assert!(s.apply(&ServerEvent::ResetAnimations, me()).is_none());
    }

    #[test]
    fn outgoing_frames_match_the_original_packet_lengths() {
        for (frame, opcode) in [
            (set_rank("Bob", 3), 188),
            (join_clan("Bob").unwrap(), 104),
            (leave_clan(), 104),
        ] {
            assert_eq!(frame.opcode, opcode);
            assert_eq!(
                Direction::ClientToServer.size(opcode),
                Some(Size::Fixed(frame.payload.len()))
            );
        }
        for opcode in [120, 57, 34, 213, 162] {
            assert_eq!(Direction::ClientToServer.size(opcode), Some(Size::Fixed(8)));
        }
        assert_eq!(set_rank("Bob", 3).payload[0], 131);
        assert!(join_clan("???").is_none());
    }
}
