use super::{Error, Result, communication::CommunicationInfo};
use crate::auth::{
    AuthError, Clock,
    account::{Account, UserAccountInfo},
    storage::{AccountStorage, lock},
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientInfo {
    pub display_mode: i32,
    pub window_mode: i32,
    pub screen_width: i32,
    pub screen_height: i32,
}
impl ClientInfo {
    pub fn is_high_detail(&self) -> bool {
        self.display_mode > 0
    }
    pub fn is_resizable(&self) -> bool {
        self.window_mode > 1
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Rights {
    RegularPlayer,
    PlayerModerator,
    Administrator,
}
impl Rights {
    pub fn from_ordinal(id: i32) -> Result<Self> {
        [
            Self::RegularPlayer,
            Self::PlayerModerator,
            Self::Administrator,
        ]
        .get(id as usize)
        .copied()
        .ok_or(Error::Index)
    }
    pub fn for_id(id: i32) -> Result<Self> {
        Self::from_ordinal(id.max(0))
    }
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UidInfo {
    pub ip: Option<String>,
    pub computer_name: Option<String>,
    pub mac: Option<String>,
    pub serial: Option<String>,
}
impl UidInfo {
    pub fn new(
        ip: Option<String>,
        computer_name: Option<String>,
        mac: Option<String>,
        serial: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            ip,
            computer_name,
            mac: Some(mac.ok_or(Error::Null)?.replace(':', "-")),
            serial,
        })
    }
    pub fn translate(&mut self, other: &Self) -> Result<()> {
        self.ip = other.ip.clone();
        self.computer_name = other.computer_name.clone();
        self.mac = Some(other.mac.as_deref().ok_or(Error::Null)?.replace(':', "-"));
        self.serial = other.serial.clone();
        Ok(())
    }
}
impl std::fmt::Display for UidInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[ip={}, compName={}, mac={}, serial={}]",
            self.ip.as_deref().unwrap_or("null"),
            self.computer_name.as_deref().unwrap_or("null"),
            self.mac.as_deref().unwrap_or("null"),
            self.serial.as_deref().unwrap_or("null")
        )
    }
}
pub struct PlayerDetails {
    pub account: Account,
    pub communication: CommunicationInfo,
    pub uid: UidInfo,
    /// A present session overrides UID IP, including a null session address.
    pub session_address: Option<Option<String>>,
    pub save_parsed: bool,
    clock: Arc<Clock>,
}
impl PlayerDetails {
    pub fn new(username: &str, clock: Arc<Clock>) -> Self {
        let mut account = UserAccountInfo::create_default(clock());
        account.username = username.to_owned();
        Self {
            account: account.shared(),
            communication: CommunicationInfo::default(),
            uid: UidInfo::default(),
            session_address: None,
            save_parsed: false,
            clock,
        }
    }
    pub fn is_banned(&self) -> bool {
        let until = lock(&self.account).ban_end_time;
        until > (self.clock)()
    }
    pub fn is_muted(&self) -> bool {
        let until = lock(&self.account).mute_end_time;
        until > (self.clock)()
    }
    pub fn is_permanent_mute(&self) -> bool {
        let until = lock(&self.account).mute_end_time;
        until.wrapping_sub((self.clock)()) / 86_400_000 > 1000
    }
    pub fn rights(&self) -> Result<Rights> {
        Rights::from_ordinal(lock(&self.account).rights)
    }
    pub fn set_rights(&self, rights: Rights) {
        lock(&self.account).rights = rights.ordinal();
    }
    pub fn ip_address(&self) -> Option<&str> {
        self.session_address
            .as_ref()
            .map_or(self.uid.ip.as_deref(), |address| address.as_deref())
    }
    /// Mutate the shared account in source order before updating storage.
    /// Only the original IllegalStateException category is ignored.
    pub fn save(&self, storage: &dyn AccountStorage) -> Result<()> {
        if !self.save_parsed || self.is_banned() {
            return Ok(());
        }
        {
            lock(&self.account).contacts = self.communication.contact_string()?;
        }
        {
            lock(&self.account).blocked = self.communication.blocked_string();
        }
        {
            lock(&self.account).clan_name =
                self.communication.clan_name.clone().ok_or(Error::Null)?;
        }
        {
            lock(&self.account).clan_reqs = self.communication.clan_requirement_string()?;
        }
        {
            lock(&self.account).current_clan = self.communication.current_clan().to_owned();
        }
        match storage.update(Arc::clone(&self.account)) {
            Ok(()) | Err(AuthError::IllegalState) => Ok(()),
            Err(error) => Err(Error::Storage(error)),
        }
    }
}
