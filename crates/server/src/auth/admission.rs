//! LoginReadEvent's parser/authentication decision, before communication/world entry.
use super::{Authenticator, Clock, account::Account};
use rs09_protocol::login::{AuthResponse, LoginInfo, LoginOutcome};

pub struct Authenticated {
    pub login: LoginInfo,
    pub account: Account,
}
#[allow(
    clippy::large_enum_variant,
    reason = "Keep the moved login metadata inline without another allocation per admission."
)]
pub enum Decision {
    Reply(AuthResponse),
    Ready(Authenticated),
}
pub fn check(parsed: LoginOutcome, auth: &Authenticator, clock: &Clock) -> Decision {
    let info = match (parsed.response, parsed.info) {
        (AuthResponse::Success, Some(info)) => info,
        (response, _) => return Decision::Reply(response),
    };
    let outcome = match auth.check_login_with_clock(&info.username, &info.password, clock) {
        Ok(outcome) => outcome,
        Err(_) => return Decision::Reply(AuthResponse::UnexpectedError),
    };
    match (outcome.response, outcome.account) {
        (AuthResponse::Success, Some(account)) => Decision::Ready(Authenticated {
            login: info,
            account,
        }),
        (response, _) => Decision::Reply(response),
    }
}
