//! UserAccountInfo field order, Java hash codes, and mutable Timestamp snapshot semantics.
use super::storage::lock;
use super::{AuthError, Result};
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

pub fn java_string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |value, unit| {
        value.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampValue {
    pub base_millis: i64,
    pub nanos: u32,
}
#[derive(Clone)]
pub struct Timestamp(Arc<Mutex<TimestampValue>>);
impl Timestamp {
    pub fn new(millis: i64) -> Self {
        let mut base = (millis / 1000) * 1000;
        let mut nanos = (millis % 1000) * 1_000_000;
        if nanos < 0 {
            nanos += 1_000_000_000;
            base = base.wrapping_sub(1000);
        }
        Self(Arc::new(Mutex::new(TimestampValue {
            base_millis: base,
            nanos: nanos as u32,
        })))
    }
    pub fn value(&self) -> TimestampValue {
        *lock(&self.0)
    }
    pub fn millis(&self) -> i64 {
        let value = self.value();
        value
            .base_millis
            .wrapping_add(i64::from(value.nanos / 1_000_000))
    }
    pub fn set_time(&self, millis: i64) {
        *lock(&self.0) = Self::new(millis).value();
    }
    pub fn set_nanos(&self, nanos: u32) -> Result<()> {
        if nanos >= 1_000_000_000 {
            return Err(AuthError::Storage);
        }
        lock(&self.0).nanos = nanos;
        Ok(())
    }
}
impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.value() == other.value()
    }
}
impl Eq for Timestamp {}
impl std::fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value().fmt(f)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldValue {
    Text(String),
    Int(i32),
    Long(i64),
    Bool(bool),
    Timestamp(Timestamp),
}

#[derive(Clone)]
pub struct AccountFields {
    pub username: String,
    pub password: String,
    pub uid: i32,
    pub rights: i32,
    pub credits: i32,
    pub ip: String,
    pub last_used_ip: String,
    pub mute_end_time: i64,
    pub ban_end_time: i64,
    pub contacts: String,
    pub blocked: String,
    pub clan_name: String,
    pub current_clan: String,
    pub clan_reqs: String,
    pub time_played: i64,
    pub last_login: i64,
    pub online: bool,
    pub join_date: Timestamp,
}
pub type Account = Arc<Mutex<UserAccountInfo>>;
pub struct UserAccountInfo {
    pub fields: AccountFields,
    pub initial_values: Option<Vec<FieldValue>>,
}
impl Deref for UserAccountInfo {
    type Target = AccountFields;
    fn deref(&self) -> &Self::Target {
        &self.fields
    }
}
impl DerefMut for UserAccountInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields
    }
}
impl UserAccountInfo {
    pub fn new(fields: AccountFields) -> Self {
        Self {
            fields,
            initial_values: None,
        }
    }
    pub fn create_default(now: i64) -> Self {
        let mut account = Self::new(AccountFields {
            username: String::new(),
            password: String::new(),
            uid: 0,
            rights: 0,
            credits: 0,
            ip: String::new(),
            last_used_ip: String::new(),
            mute_end_time: 0,
            ban_end_time: 0,
            contacts: String::new(),
            blocked: String::new(),
            clan_name: String::new(),
            current_clan: String::new(),
            clan_reqs: "1,0,8,9".into(),
            time_played: 0,
            last_login: 0,
            online: false,
            join_date: Timestamp::new(now),
        });
        account.set_initial_reference_values();
        account
    }
    pub fn shared(self) -> Account {
        Arc::new(Mutex::new(self))
    }
    pub fn values(&self) -> Vec<FieldValue> {
        use FieldValue::*;
        vec![
            Text(self.username.clone()),
            Text(self.password.clone()),
            Int(self.uid),
            Int(self.rights),
            Int(self.credits),
            Text(self.ip.clone()),
            Text(self.last_used_ip.clone()),
            Long(self.mute_end_time),
            Long(self.ban_end_time),
            Text(self.contacts.clone()),
            Text(self.blocked.clone()),
            Text(self.clan_name.clone()),
            Text(self.current_clan.clone()),
            Text(self.clan_reqs.clone()),
            Long(self.time_played),
            Long(self.last_login),
            Bool(self.online),
            Timestamp(self.join_date.clone()),
        ]
    }
    pub fn set_initial_reference_values(&mut self) {
        self.initial_values = Some(self.values());
    }
    pub fn set_value(&mut self, index: usize, value: FieldValue) -> Result<()> {
        use FieldValue::*;
        match (index, value) {
            (0, Text(value)) => self.username = value,
            (1, Text(value)) => self.password = value,
            (2, Int(value)) => self.uid = value,
            (3, Int(value)) => self.rights = value,
            (4, Int(value)) => self.credits = value,
            (5, Text(value)) => self.ip = value,
            (6, Text(value)) => self.last_used_ip = value,
            (7, Long(value)) => self.mute_end_time = value,
            (8, Long(value)) => self.ban_end_time = value,
            (9, Text(value)) => self.contacts = value,
            (10, Text(value)) => self.blocked = value,
            (11, Text(value)) => self.clan_name = value,
            (12, Text(value)) => self.current_clan = value,
            (13, Text(value)) => self.clan_reqs = value,
            (14, Long(value)) => self.time_played = value,
            (15, Long(value)) => self.last_login = value,
            (16, Bool(value)) => self.online = value,
            (17, Timestamp(value)) => self.join_date = value,
            _ => return Err(AuthError::Storage),
        }
        Ok(())
    }
    pub fn changed_fields(&self) -> Result<(Vec<usize>, Vec<FieldValue>)> {
        let current = self.values();
        let initial = self
            .initial_values
            .as_ref()
            .ok_or(AuthError::UninitializedBaseline)?;
        if initial.len() != current.len() {
            return Err(AuthError::UninitializedBaseline);
        }
        let changed = current
            .iter()
            .zip(initial)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        Ok((changed, current))
    }
    pub fn is_default(&self) -> bool {
        static DEFAULT: std::sync::OnceLock<UserAccountInfo> = std::sync::OnceLock::new();
        self == DEFAULT.get_or_init(|| Self::create_default(0))
    }
    pub fn java_hash(&self) -> i32 {
        let long = |value: i64| (value as u64 ^ (value as u64 >> 32)) as i32;
        [
            java_string_hash(&self.password),
            self.uid,
            self.rights,
            self.credits,
            java_string_hash(&self.ip),
            java_string_hash(&self.last_used_ip),
            long(self.mute_end_time),
            long(self.ban_end_time),
            java_string_hash(&self.contacts),
            java_string_hash(&self.blocked),
            java_string_hash(&self.clan_name),
            java_string_hash(&self.current_clan),
            java_string_hash(&self.clan_reqs),
            long(self.time_played),
            long(self.last_login),
            if self.online { 1231 } else { 1237 },
        ]
        .into_iter()
        .fold(java_string_hash(&self.username), |hash, field| {
            hash.wrapping_mul(31).wrapping_add(field)
        })
    }
}
impl PartialEq for UserAccountInfo {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username
            && self.password == other.password
            && self.uid == other.uid
            && self.rights == other.rights
            && self.credits == other.credits
            && self.ip == other.ip
            && self.last_used_ip == other.last_used_ip
            && self.mute_end_time == other.mute_end_time
            && self.ban_end_time == other.ban_end_time
            && self.contacts == other.contacts
            && self.blocked == other.blocked
            && self.clan_name == other.clan_name
            && self.current_clan == other.current_clan
            && self.clan_reqs == other.clan_reqs
            && self.time_played == other.time_played
            && self.last_login == other.last_login
            && self.online == other.online
    }
}
impl Eq for UserAccountInfo {}
