use super::{
    Clock, Result,
    account::{Account, UserAccountInfo, java_string_hash},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub trait AccountStorage: Send + Sync {
    fn username_taken(&self, username: &str) -> Result<bool>;
    fn get_account_info(&self, username: &str, now: i64) -> Result<Account>;
    fn get_account_info_with_clock(&self, username: &str, clock: &Clock) -> Result<Account> {
        self.get_account_info(username, clock())
    }
    fn store(&self, info: Account) -> Result<()>;
    fn update(&self, info: Account) -> Result<()>;
    fn remove(&self, info: &Account) -> Result<()>;
    fn online_friends(&self, username: &str) -> Result<Vec<String>>;
    fn usernames_with_ip(&self, ip: &str) -> Result<Vec<Option<String>>>;
}
#[derive(Default)]
pub struct InMemoryStorage {
    storage: Mutex<HashMap<String, Account>>,
}
impl InMemoryStorage {
    /// Clones handles under the map lock; inspect values after releasing it.
    /// A stable world-phase boundary is required for a consistent deep snapshot.
    pub fn snapshot_handles(&self) -> std::collections::BTreeMap<String, Account> {
        lock(&self.storage)
            .iter()
            .map(|(key, value)| (key.clone(), Arc::clone(value)))
            .collect()
    }
}
impl AccountStorage for InMemoryStorage {
    fn username_taken(&self, username: &str) -> Result<bool> {
        Ok(lock(&self.storage).contains_key(username))
    }
    fn get_account_info(&self, username: &str, now: i64) -> Result<Account> {
        self.get_account_info_with_clock(username, &move || now)
    }
    fn get_account_info_with_clock(&self, username: &str, clock: &Clock) -> Result<Account> {
        Ok(Arc::clone(
            lock(&self.storage)
                .entry(username.to_owned())
                .or_insert_with(|| {
                    let mut info = UserAccountInfo::create_default(clock());
                    info.uid = java_string_hash(username);
                    info.shared()
                }),
        ))
    }
    fn store(&self, info: Account) -> Result<()> {
        let name = {
            let mut fields = lock(&info);
            fields.uid = java_string_hash(&fields.username);
            fields.username.clone()
        };
        lock(&self.storage).insert(name, info);
        Ok(())
    }
    fn update(&self, info: Account) -> Result<()> {
        let name = lock(&info).username.clone();
        lock(&self.storage).insert(name, info);
        Ok(())
    }
    fn remove(&self, info: &Account) -> Result<()> {
        let name = lock(info).username.clone();
        lock(&self.storage).remove(&name);
        Ok(())
    }
    fn online_friends(&self, _username: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    fn usernames_with_ip(&self, _ip: &str) -> Result<Vec<Option<String>>> {
        Ok(Vec::new())
    }
}
