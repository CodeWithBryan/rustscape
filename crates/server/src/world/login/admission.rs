//! The network-thread half of a login: `LoginReadEvent.read` after the block
//! is decoded and the account authenticated, `Login.proceedWith`,
//! `checkAccountLimit`, `proceedWithAcceptableLogin`, `sendMSEvents` and
//! `LoginParser.initialize` up to the submission of its world pulse.
use super::{Error, Host, RECONNECT_LOGIN_OP, Result, Write, ip, pulse::LoginPulse, username};
use crate::player::details::{PlayerDetails, Rights, UidInfo};
use crate::world::{disconnection, repository::Repository};
use rs09_protocol::login::AuthResponse;

/// How an admission ended, for the caller's records. The bytes the original
/// wrote are written through [`Host::write_bytes`] either way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admitted<P> {
    /// A reply other than Success was written and nothing else happened.
    Refused(AuthResponse),
    /// The player is in the repository and its login pulse was submitted
    /// (`validateRequest` passed). The reply is written when the pulse runs.
    Submitted(P),
    /// The player was added to the repository but `validateRequest` stopped
    /// the login (inactive session, or a reply it flagged).
    Stopped(P),
    /// `proceedWithAcceptableLogin` caught an `Exception` and disconnected the
    /// session.
    Disconnected(P),
}

/// `LoginReadEvent.read` from `Login.proceedWith` on: the original's
/// `catch (Exception)` around it prints and writes UnexpectedError.
pub fn read<P: Clone + Eq + std::hash::Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    session: &H::Session,
    details: PlayerDetails,
    opcode: i32,
) -> Result<Admitted<P>> {
    match proceed_with(host, repository, session, details, opcode) {
        Err(error) if error.is_exception() => {
            host.print_stack(&error);
            super::write_response(
                host,
                repository,
                session,
                AuthResponse::UnexpectedError,
                Write::Queued,
            )?;
            Ok(Admitted::Refused(AuthResponse::UnexpectedError))
        }
        other => other,
    }
}

/// `Login.proceedWith(session, details, opcode)`.
pub fn proceed_with<P: Clone + Eq + std::hash::Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    session: &H::Session,
    mut details: PlayerDetails,
    opcode: i32,
) -> Result<Admitted<P>> {
    let uid = crate::auth::storage::lock(&details.account).uid;
    if repository.contains_uid(uid) {
        super::write_response(
            host,
            repository,
            session,
            AuthResponse::AlreadyOnline,
            Write::Queued,
        )?;
        return Ok(Admitted::Refused(AuthResponse::AlreadyOnline));
    }
    details.session_address = Some(host.session_address(session));
    let address = ip(&details);
    let deprecated = Some("DEPRECATED".to_owned());
    let info = UidInfo::new(
        address.clone(),
        deprecated.clone(),
        deprecated.clone(),
        deprecated,
    )
    .map_err(|e| Error::Exception(format!("UIDInfo: {e:?}")))?;
    details
        .uid
        .translate(&info)
        .map_err(|e| Error::Exception(format!("UIDInfo.translate: {e:?}")))?;

    let flagged = host.flagged_ips()?;
    if flagged
        .iter()
        .any(|entry| Some(entry.as_str()) == address.as_deref())
    {
        let name = crate::auth::storage::lock(&details.account)
            .username
            .clone();
        let message = format!(
            "Login from flagged IP {}",
            address.as_deref().unwrap_or("null")
        );
        host.post_player_alert(&name, &message)?;
    }

    let player = host.create_player(details)?;
    let address = host.with_details(&player, |d| ip(d))?;
    host.log_ip(&player, address.as_deref())?;
    if can_bypass_account_limit_check(host, &player)? {
        proceed_with_acceptable_login(host, repository, session, player, opcode)
    } else {
        let name = username(host, &player)?;
        if check_account_limit(host, address.as_deref(), &name)? {
            proceed_with_acceptable_login(host, repository, session, player, opcode)
        } else {
            super::write_response(
                host,
                repository,
                session,
                AuthResponse::LoginLimitExceeded,
                Write::Queued,
            )?;
            Ok(Admitted::Refused(AuthResponse::LoginLimitExceeded))
        }
    }
}

/// `canBypassAccountLimitCheck`: administrators and player moderators.
fn can_bypass_account_limit_check<P, H: Host<P>>(host: &mut H, player: &P) -> Result<bool> {
    let rights = host
        .with_details(player, |details| details.rights())?
        .map_err(|_| Error::Exception("ArrayIndexOutOfBoundsException: rights".into()))?;
    Ok(rights == Rights::Administrator || rights == Rights::PlayerModerator)
}

/// `checkAccountLimit(ipAddress, username)`. The exceptions file's block is
/// one service; its failure is printed and the configured limit kept.
pub fn check_account_limit<P, H: Host<P>>(
    host: &mut H,
    ip: Option<&str>,
    username: &str,
) -> Result<bool> {
    let constants = host.constants();
    let mut limit = constants.daily_account_limit;
    let path = format!(
        "{}account_limit_exceptions.conf",
        constants.config_path.as_deref().unwrap_or("null")
    );
    match host.account_limit_exception(&path, ip) {
        Ok(Some(value)) => limit = value as i32,
        Ok(None) => {}
        Err(error) if error.is_exception() => host.print_stack(&error),
        Err(error) => return Err(error),
    }
    let accounts = host.daily_accounts(ip)?;
    if accounts.iter().any(|name| name == username) {
        return Ok(true);
    }
    if accounts.len() as i64 >= i64::from(limit) {
        return Ok(false);
    }
    host.add_daily_account(ip, username)?;
    Ok(true)
}

/// `proceedWithAcceptableLogin(session, player, opcode)`.
fn proceed_with_acceptable_login<P: Clone + Eq + std::hash::Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    session: &H::Session,
    player: P,
    opcode: i32,
) -> Result<Admitted<P>> {
    repository.add_player(host, &player)?;
    let now = host.now_millis();
    host.set_last_ping(session, now);
    let attempt = (|| -> Result<bool> {
        let submitted = initialize(host, repository, &player, opcode == RECONNECT_LOGIN_OP)?;
        send_ms_events(host, &player);
        Ok(submitted)
    })();
    match attempt {
        Ok(true) => Ok(Admitted::Submitted(player)),
        Ok(false) => Ok(Admitted::Stopped(player)),
        Err(error) if error.is_exception() => {
            host.print_stack(&error);
            disconnection::session_disconnect(host, repository, session)?;
            Ok(Admitted::Disconnected(player))
        }
        Err(error) => Err(error),
    }
}

/// `Login.sendMSEvents(player.details)`: native social.
fn send_ms_events<P: Clone + Eq + std::hash::Hash, H: Host<P>>(host: &mut H, player: &P) {
    let (social, services) = host.social();
    social.login_events(services, player);
}

/// `LoginParser.initialize(player, reconnect)`. Answers whether the world
/// pulse was submitted.
pub fn initialize<P: Clone + Eq + std::hash::Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    player: &P,
    reconnect: bool,
) -> Result<bool> {
    let session = host
        .session(player)
        .ok_or_else(|| Error::Exception("NullPointerException: details.session".into()))?;
    if !validate_request(host, repository, player, &session)? {
        return Ok(false);
    }
    let parsed = (|| -> Result<H::Parser> {
        match host.parse_save(player)? {
            Some(parser) => Ok(parser),
            None => {
                let name = username(host, player)?;
                Err(Error::Exception(format!(
                    "IllegalStateException: Failed parsing save for: {name}"
                )))
            }
        }
    })();
    let parser = match parsed {
        Ok(parser) => Some(parser),
        Err(error) if error.is_exception() => {
            host.print_stack(&error);
            repository.remove_player(host, player)?;
            flag(
                host,
                repository,
                &session,
                AuthResponse::ErrorLoadingProfile,
            )?;
            None
        }
        Err(error) => return Err(error),
    };
    host.submit_login_pulse(LoginPulse {
        player: player.clone(),
        session,
        reconnect,
        parser,
    })?;
    Ok(true)
}

/// `validateRequest()`: an inactive session stops silently; an update in
/// progress and a ban are flagged.
fn validate_request<P: Clone + Eq + std::hash::Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    player: &P,
    session: &H::Session,
) -> Result<bool> {
    if !host.session_active(session) {
        return Ok(false);
    }
    if host.is_updating() {
        return flag(host, repository, session, AuthResponse::Updating);
    }
    if host.with_details(player, |details| details.is_banned())? {
        return flag(host, repository, session, AuthResponse::AccountDisabled);
    }
    Ok(true)
}

/// `LoginParser.flag(response)`: `details.session.write(response, true)`.
pub fn flag<P: Clone + Eq + std::hash::Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    session: &H::Session,
    response: AuthResponse,
) -> Result<bool> {
    super::write_response(host, repository, session, response, Write::Instant)?;
    Ok(response == AuthResponse::Success)
}
