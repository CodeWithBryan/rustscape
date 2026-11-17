//! The commands' account and authenticator calls joined to the native
//! `crate::auth`: a world answers [`super::Host::username_taken`],
//! [`super::Host::account_info`], [`super::Host::account_update`],
//! [`super::Host::usernames_with_ip`], [`super::Host::check_password`] and
//! [`super::Host::update_password`] through [`Accounts`], so bans, mutes,
//! credits and passwords land in the same account records login reads.
use super::{Error, Result};
use crate::auth::account::Account;
use crate::auth::storage::lock;
use crate::auth::{AuthError, Authenticator, Clock};

/// A failure of the native account layer, as the service's exception.
fn failed(error: AuthError) -> Error {
    Error::Binding(Some(format!("account storage: {error}")))
}

/// `GameWorld.accountStorage` and `GameWorld.authenticator`, natively.
pub struct Accounts<'a> {
    pub authenticator: &'a Authenticator,
    /// `System.currentTimeMillis()` where the storage reads it.
    pub clock: &'a Clock,
}
impl Accounts<'_> {
    /// `checkUsernameTaken(name)`.
    pub fn username_taken(&self, name: &str) -> Result<bool> {
        self.authenticator
            .storage
            .username_taken(name)
            .map_err(failed)
    }
    /// `getAccountInfo(name)`; the Kotlin storage refuses a null name.
    pub fn account_info(&self, name: Option<&str>) -> Result<Account> {
        let name = name.ok_or(Error::Null)?;
        self.authenticator
            .storage
            .get_account_info_with_clock(name, self.clock)
            .map_err(failed)
    }
    /// `update(info)`.
    pub fn update(&self, info: &Account) -> Result<()> {
        self.authenticator
            .storage
            .update(std::sync::Arc::clone(info))
            .map_err(failed)
    }
    /// `getUsernamesWithIP(ip)`.
    pub fn usernames_with_ip(&self, ip: &str) -> Result<Vec<Option<String>>> {
        self.authenticator
            .storage
            .usernames_with_ip(ip)
            .map_err(failed)
    }
    /// `checkPassword(player, password)` against `player.details.password`.
    pub fn check_password(&self, account: &Account, password: &str) -> Result<bool> {
        let stored = lock(account).password.clone();
        self.authenticator
            .check_password(&stored, password)
            .map_err(failed)
    }
    /// `updatePassword(username, password)`.
    pub fn update_password(&self, username: &str, password: &str) -> Result<()> {
        self.authenticator
            .update_password_with_clock(username, password, self.clock)
            .map_err(failed)
    }
}
