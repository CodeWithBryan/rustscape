//! Auth.configure provider selection and reconfiguration ordering.
use super::{
    Authenticator, Mode,
    password::PasswordCodec,
    sql::SqlStorage,
    storage::{AccountStorage, InMemoryStorage},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct Settings {
    pub use_auth: bool,
    pub persist_accounts: bool,
    pub default_admin: bool,
    pub database: DatabaseSettings,
}
impl Default for Settings {
    fn default() -> Self {
        // ServerConstants initializers, before ServerConfigParser loads a TOML.
        Self {
            use_auth: false,
            persist_accounts: false,
            default_admin: true,
            database: DatabaseSettings::default(),
        }
    }
}
/// Contains credentials. The separate DATABASE_PORT is unused by Auth.configure.
#[derive(Clone, Default)]
pub struct DatabaseSettings {
    pub address: Option<String>,
    pub name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub port: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingSetting {
    Address,
    Name,
    Username,
    Password,
}
impl std::fmt::Display for MissingSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "missing SQL authentication setting: {self:?}")
    }
}
impl std::error::Error for MissingSetting {}

#[derive(Clone)]
pub enum Provider {
    Memory(Arc<InMemoryStorage>),
    Sql(Arc<SqlStorage>),
}
impl Provider {
    pub fn storage(&self) -> Arc<dyn AccountStorage> {
        match self {
            Self::Memory(storage) => storage.clone(),
            Self::Sql(storage) => storage.clone(),
        }
    }
}
pub struct Registry {
    provider: Option<Provider>,
    authenticator: Option<Arc<Authenticator>>,
    admin: Arc<AtomicBool>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            provider: None,
            authenticator: None,
            admin: Arc::new(AtomicBool::new(true)),
        }
    }
}
impl Registry {
    pub fn provider(&self) -> Option<&Provider> {
        self.provider.as_ref()
    }
    pub fn authenticator(&self) -> Option<&Arc<Authenticator>> {
        self.authenticator.as_ref()
    }
    pub fn configure(
        &mut self,
        settings: &Settings,
        passwords: Arc<dyn PasswordCodec>,
        online: impl Fn(&str) -> bool + Send + Sync + 'static,
    ) -> Result<(), MissingSetting> {
        self.admin.store(settings.default_admin, Ordering::Relaxed);
        self.provider = Some(if settings.persist_accounts {
            Provider::Sql(Arc::new(SqlStorage::unconfigured(online)))
        } else {
            Provider::Memory(Arc::new(InMemoryStorage::default()))
        });
        if settings.use_auth
            && let Some(Provider::Sql(sql)) = &mut self.provider
        {
            let fields = &settings.database;
            // Argument evaluation uses !! left to right. Assignment of the
            // global authenticator occurs only after configureFor succeeds.
            let address = fields.address.as_deref().ok_or(MissingSetting::Address)?;
            let name = fields.name.as_deref().ok_or(MissingSetting::Name)?;
            let username = fields.username.as_deref().ok_or(MissingSetting::Username)?;
            let password = fields.password.as_deref().ok_or(MissingSetting::Password)?;
            Arc::get_mut(sql)
                .expect("new provider has no shared handles")
                .configure_reference(address, name, username, password);
        }
        let mode = if settings.use_auth {
            Mode::Production
        } else {
            Mode::Development {
                default_admin: settings.default_admin,
            }
        };
        self.authenticator = Some(Arc::new(
            Authenticator::new(mode, self.provider.as_ref().unwrap().storage(), passwords)
                .follow_admin_setting(Arc::clone(&self.admin)),
        ));
        Ok(())
    }
}
