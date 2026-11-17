//! Original communication data, account/legacy-column parsing and serialization.
//! Social actions, packets, clan repositories and loot share are in world::social.
use super::{Error, Result};
use crate::auth::account::UserAccountInfo;
use crate::world::repository::JavaHashMap;
use rs09_game_core::java_string::parse_i32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
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
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn value(self) -> i32 {
        match self {
            Self::None => -1,
            Self::Administrator => 127,
            _ => self.ordinal() - 1,
        }
    }
    pub fn info(self) -> &'static str {
        [
            "Anyone",
            "Any friends",
            "Recruit+",
            "Corporal+",
            "Sergeant+",
            "Lieutenant+",
            "Captain+",
            "General+",
            "Only me",
            "No-one",
        ][self as usize]
    }
    pub fn name(self) -> &'static str {
        [
            "NONE",
            "FRIEND",
            "RECRUIT",
            "CORPORAL",
            "SERGEANT",
            "LIEUTENANT",
            "CAPTAIN",
            "GENERAL",
            "OWNER",
            "ADMINISTRATOR",
        ][self as usize]
    }
    pub fn from_ordinal(value: i32) -> Result<Self> {
        Self::ALL.get(value as usize).copied().ok_or(Error::Index)
    }
}
impl std::fmt::Display for ClanRank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rank=[{}], Info=[{}]", self.name(), self.info())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contact {
    pub username: Option<String>,
    pub world_id: i32,
    pub rank: Option<ClanRank>,
}
impl Contact {
    pub fn new(username: Option<String>) -> Self {
        Self {
            username,
            world_id: 0,
            rank: Some(ClanRank::Friend),
        }
    }
}
/// The contact `HashMap<String, Contact>`: put, get, remove, clear and the
/// walk keep the JDK 11 table (world::repository::JavaHashMap).
#[derive(Default)]
pub struct Contacts {
    order: JavaHashMap<Option<String>, usize>,
    values: Vec<Option<Contact>>,
}
impl Contacts {
    pub fn insert(&mut self, key: String, value: Contact) {
        let key = Some(key);
        if let Some(&index) = self.order.get(&key) {
            self.values[index] = Some(value);
        } else {
            self.values.push(Some(value));
            self.order.put(key, self.values.len() - 1);
        }
    }
    pub fn get(&self, key: &str) -> Option<&Contact> {
        let index = *self.order.get(&Some(key.to_owned()))?;
        self.values[index].as_ref()
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Contact> {
        let index = *self.order.get(&Some(key.to_owned()))?;
        self.values[index].as_mut()
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.order.contains_key(&Some(key.to_owned()))
    }
    pub fn remove(&mut self, key: &str) -> Option<Contact> {
        let index = self.order.remove(&Some(key.to_owned()))?;
        self.values[index].take()
    }
    /// `clear()`: the table keeps its length, as the JDK's does.
    pub fn clear(&mut self) {
        self.order.clear();
        self.values.clear();
    }
    pub fn len(&self) -> usize {
        self.order.len() as usize
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Contact)> {
        self.order.iter().map(|(key, &index)| {
            (
                key.as_deref().expect("contact keys are not null"),
                self.values[index].as_ref().expect("live contact"),
            )
        })
    }
}
pub const MAX_LIST_SIZE: usize = 200;
pub struct CommunicationInfo {
    pub contacts: Contacts,
    pub blocked: Vec<Option<String>>,
    pub clan_name: Option<String>,
    pub current_clan: Option<String>,
    pub requirements: [Option<ClanRank>; 4],
    pub loot_share: bool,
}
impl Default for CommunicationInfo {
    fn default() -> Self {
        Self {
            contacts: Contacts::default(),
            blocked: Vec::new(),
            clan_name: Some(String::new()),
            current_clan: Some(String::new()),
            requirements: [
                Some(ClanRank::Friend),
                Some(ClanRank::None),
                Some(ClanRank::Owner),
                Some(ClanRank::Administrator),
            ],
            loot_share: false,
        }
    }
}
/// Java String.split with a literal, one-character delimiter and limit zero.
fn split(text: &str, delimiter: char) -> impl Iterator<Item = &str> {
    let trimmed = text.trim_end_matches(delimiter);
    let all_delimiters = !text.is_empty() && trimmed.is_empty();
    trimmed.split(delimiter).filter(move |_| !all_delimiters)
}
fn parse_contact_entries(text: &str, contacts: &mut Contacts) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    for entry in split(text, '~') {
        let stripped = entry.replace(['{', '}'], "");
        let mut tokens = split(&stripped, ',');
        let Some(username) = tokens.next() else {
            continue;
        };
        let ordinal = parse_i32(tokens.next().ok_or(Error::Index)?).ok_or(Error::Number)?;
        let rank = ClanRank::from_ordinal(ordinal)?;
        let mut contact = Contact::new(Some(username.to_owned()));
        contact.rank = Some(rank);
        contacts.insert(username.to_owned(), contact);
    }
    Ok(())
}
pub fn parse_contacts(text: &str) -> Result<Contacts> {
    let mut contacts = Contacts::default();
    parse_contact_entries(text, &mut contacts)?;
    Ok(contacts)
}
pub fn parse_clan_requirements(text: &str) -> Result<[Option<ClanRank>; 4]> {
    let mut requirements = [None; 4];
    for (index, token) in split(text, ',').enumerate() {
        let ordinal = parse_i32(token).ok_or(Error::Number)?;
        let Ok(rank) = ClanRank::from_ordinal(ordinal) else {
            continue;
        };
        if index < 4 {
            requirements[index] = Some(if index == 2 && !(3..=8).contains(&ordinal) {
                ClanRank::Administrator
            } else {
                rank
            });
        }
    }
    Ok(requirements)
}
/// A legacy SQLTable cell preserves Java's reference-identity empty-string test.
/// Database connectivity is provided by auth::sql, independently of this adapter.
#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub text: Option<String>,
    pub interned_empty: bool,
}
pub trait Columns {
    fn read(&mut self, name: &str) -> Result<Cell>;
    fn write(&mut self, name: &str, value: Option<String>) -> Result<()>;
}
impl CommunicationInfo {
    pub fn parse_account(&mut self, account: &UserAccountInfo) -> Result<()> {
        self.blocked
            .extend(split(&account.blocked, ',').map(|value| Some(value.to_owned())));
        self.clan_name = Some(account.clan_name.clone());
        self.current_clan = Some(account.current_clan.clone());
        self.requirements = parse_clan_requirements(&account.clan_reqs)?;
        self.contacts = parse_contacts(&account.contacts)?;
        Ok(())
    }
    pub fn parse_columns(&mut self, table: &mut dyn Columns) -> Result<()> {
        if table.read("contacts")?.text.is_some() {
            let text = table.read("contacts")?.text.ok_or(Error::Null)?;
            parse_contact_entries(&text, &mut self.contacts)?;
        }
        if table.read("blocked")?.text.is_some() {
            let text = table.read("blocked")?.text.ok_or(Error::Null)?;
            if !text.is_empty() {
                self.blocked
                    .extend(split(&text, ',').map(|s| Some(s.to_owned())));
            }
        }
        self.clan_name = table.read("clanName")?.text;
        self.current_clan = table.read("currentClan")?.text;
        let cell = table.read("clanReqs")?;
        if cell.interned_empty && cell.text.as_deref() == Some("") {
            return Ok(());
        }
        for (index, token) in split(cell.text.as_deref().ok_or(Error::Null)?, ',').enumerate() {
            let ordinal = parse_i32(token).ok_or(Error::Number)?;
            let Ok(rank) = ClanRank::from_ordinal(ordinal) else {
                continue;
            };
            if index < 4 && (index != 2 || (3..=8).contains(&ordinal)) {
                self.requirements[index] = Some(rank);
            }
        }
        Ok(())
    }
    pub fn current_clan(&self) -> &str {
        self.current_clan.as_deref().unwrap_or("")
    }
    pub fn contact_string(&self) -> Result<String> {
        self.contacts_string(false)
    }
    fn contacts_string(&self, keys: bool) -> Result<String> {
        let mut output = String::new();
        for (index, (key, contact)) in self.contacts.iter().enumerate() {
            let rank = contact.rank.ok_or(Error::Null)?;
            if index > 0 {
                output.push('~');
            }
            output.push('{');
            output.push_str(if keys {
                key
            } else {
                contact.username.as_deref().unwrap_or("null")
            });
            output.push(',');
            output.push(char::from(b'0' + rank as u8));
            output.push('}');
        }
        Ok(output)
    }
    pub fn blocked_string(&self) -> String {
        let mut output = String::new();
        for (index, value) in self.blocked.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(value.as_deref().unwrap_or("null"));
        }
        output
    }
    pub fn clan_requirement_string(&self) -> Result<String> {
        let mut output = String::with_capacity(7);
        for (index, rank) in self.requirements.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push(char::from(b'0' + rank.ok_or(Error::Null)? as u8));
        }
        Ok(output)
    }
    pub fn save_columns(&self, table: &mut dyn Columns) -> Result<()> {
        let blocked = self.blocked_string();
        let contacts = self.contacts_string(true)?;
        table.write("blocked", Some(blocked))?;
        table.write("contacts", Some(contacts))?;
        table.write("clanName", self.clan_name.clone())?;
        table.write("currentClan", self.current_clan.clone())?;
        table.write("clanReqs", Some(self.clan_requirement_string()?))
    }
}
