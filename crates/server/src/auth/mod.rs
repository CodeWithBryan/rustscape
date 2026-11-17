//! Native account rules and storage, with explicit clocks at original operation boundaries.
pub mod account;
pub mod admission;
pub mod factory;
pub mod password;
pub mod sql;
pub mod storage;
use account::{Account, UserAccountInfo};
use password::PasswordCodec;
use rs09_protocol::login::AuthResponse;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use storage::{AccountStorage, lock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthError {
    InvalidHash,
    InvalidCost,
    InvalidSalt,
    Entropy,
    Storage,
    IllegalState,
    UninitializedBaseline,
}
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AuthError {}
pub type Result<T> = std::result::Result<T, AuthError>;
pub type Clock = dyn Fn() -> i64 + Send + Sync;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Development { default_admin: bool },
    Production,
}
pub struct Authenticator {
    pub mode: Mode,
    pub storage: Arc<dyn AccountStorage>,
    passwords: Arc<dyn PasswordCodec>,
    configured_admin: Option<Arc<AtomicBool>>,
}
pub struct AuthOutcome {
    pub response: AuthResponse,
    pub account: Option<Account>,
}
impl Authenticator {
    pub fn new(
        mode: Mode,
        storage: Arc<dyn AccountStorage>,
        passwords: Arc<dyn PasswordCodec>,
    ) -> Self {
        Self {
            mode,
            storage,
            passwords,
            configured_admin: None,
        }
    }
    pub(crate) fn follow_admin_setting(mut self, setting: Arc<AtomicBool>) -> Self {
        self.configured_admin = Some(setting);
        self
    }
    pub fn can_create_account(&self, info: &Account) -> Result<bool> {
        let name = lock(info).username.clone();
        Ok(!self.storage.username_taken(&name)?)
    }
    pub fn create_account(&self, info: &Account, now: i64) -> Result<bool> {
        self.create_account_with_clock(info, &move || now)
    }
    pub fn create_account_with_clock(&self, info: &Account, clock: &Clock) -> Result<bool> {
        match self.mode {
            Mode::Development { default_admin } => {
                {
                    let mut info = lock(info);
                    info.username = info.username.to_lowercase();
                    if self
                        .configured_admin
                        .as_ref()
                        .map_or(default_admin, |setting| setting.load(Ordering::Relaxed))
                    {
                        info.rights = 2;
                    }
                }
                self.storage.store(Arc::clone(info))?;
                Ok(true)
            }
            Mode::Production => {
                let result = (|| -> Result<()> {
                    let plain = lock(info).password.clone();
                    let hashed = self.passwords.hash(&plain)?;
                    {
                        let mut info = lock(info);
                        info.password = hashed;
                        info.join_date = account::Timestamp::new(clock());
                    }
                    self.storage.store(Arc::clone(info))
                })();
                Ok(result.is_ok())
            }
        }
    }
    pub fn check_login(&self, username: &str, password: &str, now: i64) -> Result<AuthOutcome> {
        self.check_login_with_clock(username, password, &move || now)
    }
    pub fn check_login_with_clock(
        &self,
        username: &str,
        password: &str,
        clock: &Clock,
    ) -> Result<AuthOutcome> {
        let lower = username.to_lowercase();
        match self.mode {
            Mode::Development { .. } => {
                if !self.storage.username_taken(&lower)? {
                    let account = UserAccountInfo::create_default(clock()).shared();
                    lock(&account).username = username.to_owned();
                    self.create_account_with_clock(&account, clock)?;
                } else {
                    self.storage.get_account_info_with_clock(&lower, clock)?;
                }
                Ok(AuthOutcome {
                    response: AuthResponse::Success,
                    account: Some(self.storage.get_account_info_with_clock(username, clock)?),
                })
            }
            Mode::Production => {
                let parsed = (|| -> Result<AuthOutcome> {
                    if !self.storage.username_taken(&lower)? {
                        return Ok(AuthOutcome {
                            response: AuthResponse::InvalidCredentials,
                            account: None,
                        });
                    }
                    let account = self.storage.get_account_info_with_clock(&lower, clock)?;
                    let stored = lock(&account).password.clone();
                    if !self.passwords.verify(password, &stored)? || stored.is_empty() {
                        return Ok(AuthOutcome {
                            response: AuthResponse::InvalidCredentials,
                            account: None,
                        });
                    }
                    let fields = lock(&account);
                    let response = if fields.ban_end_time > clock() {
                        AuthResponse::AccountDisabled
                    } else if fields.online {
                        AuthResponse::AlreadyOnline
                    } else {
                        AuthResponse::Success
                    };
                    drop(fields);
                    Ok(AuthOutcome {
                        response,
                        account: if response == AuthResponse::Success {
                            Some(account)
                        } else {
                            None
                        },
                    })
                })();
                Ok(parsed.unwrap_or(AuthOutcome {
                    response: AuthResponse::CouldNotLogin,
                    account: None,
                }))
            }
        }
    }
    pub fn check_password(&self, stored: &str, password: &str) -> Result<bool> {
        match self.mode {
            Mode::Development { .. } => Ok(password == stored),
            Mode::Production => self.passwords.verify(password, stored),
        }
    }
    pub fn update_password(&self, username: &str, password: &str, now: i64) -> Result<()> {
        self.update_password_with_clock(username, password, &move || now)
    }
    pub fn update_password_with_clock(
        &self,
        username: &str,
        password: &str,
        clock: &Clock,
    ) -> Result<()> {
        let account = self.storage.get_account_info_with_clock(username, clock)?;
        let password = match self.mode {
            Mode::Development { .. } => password.to_owned(),
            Mode::Production => self.passwords.hash(password)?,
        };
        lock(&account).password = password;
        self.storage.update(account)
    }
}
