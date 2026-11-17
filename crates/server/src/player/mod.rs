//! Player metadata before world/profile initialization; no world-entry implementation.
pub mod appearance;
pub mod appearance_writer;
pub mod attributes;
pub mod bank_pin;
pub mod communication;
pub mod core_data;
pub mod details;
pub mod ironman;
pub mod item_ownership;
pub mod owned;
pub mod persisted;
pub mod save;
pub mod saved_data;
pub mod settings;
pub mod skills;
pub mod variables;
use crate::auth::{Clock, admission::Authenticated};
use details::{ClientInfo, PlayerDetails};
use rs09_protocol::login::LoginInfo;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Number,
    Index,
    Null,
    Storage(crate::auth::AuthError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub struct Prepared {
    pub login: LoginInfo,
    pub details: PlayerDetails,
    pub client: ClientInfo,
}
/// The data-setup suffix of LoginReadEvent, stopping before Login.proceedWith.
pub fn prepare(authenticated: Authenticated, clock: Arc<Clock>) -> Result<Prepared> {
    let Authenticated { login, account } = authenticated;
    let mut details = PlayerDetails::new(&login.username, clock);
    details.account = account;
    details
        .communication
        .parse_account(&crate::auth::storage::lock(&details.account))?;
    let client = ClientInfo {
        display_mode: login.display_mode,
        window_mode: login.window_mode,
        screen_width: login.screen_width,
        screen_height: login.screen_height,
    };
    Ok(Prepared {
        login,
        details,
        client,
    })
}
