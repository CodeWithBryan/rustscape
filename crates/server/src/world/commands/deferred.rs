//! What the commands leave to run later: the anonymous `Pulse`s they
//! submit to `GameWorld.Pulser`, the `runTask` body, the input-dialogue
//! handlers and the coroutine `csvmodcr` launches. Each is a typed value
//! holding what the original's object captured; the world runs it through
//! [`run_pulse`], [`run_task`], [`run_input`] or [`run_job`] when its own
//! scheduler says so. [`Pulse::label`] is the original anonymous class's
//! name, for records only.
use super::kotlin::kotlin_trim;
use super::moderation::JAIL;
use super::{Body, Host, InputType, Loc, Registry, Result, notify, run};
use crate::auth::storage::lock;
use rs09_game_core::java_lower::lowercase_root;

/// A submitted pulse.
#[derive(Clone, Debug, PartialEq)]
pub enum Pulse<P> {
    /// `ban`'s `Pulse(2)`: the stored account gets the ban end too.
    BanStore { name: Option<String>, millis: i64 },
    /// `ipban`'s `Pulse(2)` for one name of the IP.
    IpBanStore { name: Option<String>, millis: i64 },
    /// `mute`'s `Pulse(2)` for a player who is not online.
    MuteStore { name: String, millis: i64 },
    /// `jail`'s `Pulse(3)`: the player and location it captured, and the
    /// release time it computed when it was made.
    Jail {
        other: P,
        original: Option<Loc>,
        release: i64,
    },
}
impl<P> Pulse<P> {
    /// `Pulse.getDelay()` as constructed.
    pub fn delay(&self) -> i32 {
        match self {
            Self::Jail { .. } => 3,
            _ => 2,
        }
    }
    /// The original's anonymous class.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BanStore { .. } => {
                "core.game.system.command.sets.ModerationCommandSet$defineCommands$2$1"
            }
            Self::IpBanStore { .. } => {
                "core.game.system.command.sets.ModerationCommandSet$defineCommands$3$1"
            }
            Self::MuteStore { .. } => {
                "core.game.system.command.sets.ModerationCommandSet$defineCommands$4$1"
            }
            Self::Jail { .. } => {
                "core.game.system.command.sets.ModerationCommandSet$defineCommands$5$1"
            }
        }
    }
}

/// `pulse.pulse()`: true when it is finished.
pub fn run_pulse<H: Host + ?Sized>(host: &mut H, pulse: &Pulse<H::Player>) -> Result<bool> {
    match pulse {
        Pulse::BanStore { name, millis } | Pulse::IpBanStore { name, millis } => {
            let info = host.account_info(name.as_deref())?;
            let end = host.now().wrapping_add(*millis);
            lock(&info).ban_end_time = end;
            host.account_update(&info)?;
            Ok(true)
        }
        Pulse::MuteStore { name, millis } => {
            let info = host.account_info(Some(name))?;
            let end = host.now().wrapping_add(*millis);
            lock(&info).mute_end_time = end;
            host.account_update(&info)?;
            Ok(true)
        }
        Pulse::Jail {
            other,
            original,
            release,
        } => {
            let done = host.now() >= *release;
            if host.location(other)? != Some(JAIL) {
                host.set_teleport_location(other, Some(JAIL))?;
            }
            if done {
                host.set_teleport_location(other, *original)?;
            }
            Ok(done)
        }
    }
}

/// A `runTask` body.
#[derive(Clone, Debug, PartialEq)]
pub enum Task<P> {
    /// `resetpassword`: check the current password, then ask for the new one.
    CheckPassword { player: P, password: String },
}

/// The task's `invoke()` (the pulse around it counts the repeats).
pub fn run_task<H: Host + ?Sized>(host: &mut H, task: &Task<H::Player>) -> Result<()> {
    match task {
        Task::CheckPassword { player, password } => {
            if host.check_password(player, password)? {
                host.input_dialogue(
                    player,
                    InputType::StringShort,
                    "Enter New Password:",
                    InputHandler::NewPassword {
                        player: player.clone(),
                        password: password.clone(),
                    },
                )
            } else {
                host.dialogue(player, "Fail: Wrong Password.")
            }
        }
    }
}

/// An input dialogue's handler.
#[derive(Clone, Debug, PartialEq)]
pub enum InputHandler<P> {
    /// "Enter Current Password:".
    CurrentPassword { player: P },
    /// "Enter New Password:", with the current password it captured.
    NewPassword { player: P, password: String },
}

/// The value an input dialogue hands its handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputValue {
    Text(String),
    Int(i32),
}
impl InputValue {
    /// `value.toString()`.
    pub fn java_string(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Int(value) => value.to_string(),
        }
    }
}

/// The handler's body for `value`.
pub fn run_input<H: Host + ?Sized>(
    host: &mut H,
    handler: &InputHandler<H::Player>,
    value: &InputValue,
) -> Result<()> {
    match handler {
        InputHandler::CurrentPassword { player } => host.run_task(
            player,
            0,
            1,
            Task::CheckPassword {
                player: player.clone(),
                password: value.java_string(),
            },
        ),
        InputHandler::NewPassword { player, password } => {
            let new = value.java_string();
            let length = new.encode_utf16().count();
            if *password == new {
                host.dialogue(player, "Failed: Passwords Match")
            } else if !(5..=20).contains(&length) {
                host.dialogue(player, "Failed: Password Too Long Or Too Short")
            } else {
                let username = lock(&host.account(player)).username.clone();
                if new == username {
                    host.dialogue(player, "Failed: Password Is Username")
                } else {
                    let username = lock(&host.account(player)).username.clone();
                    host.update_password(&username, &new)?;
                    host.dialogue(player, "Success: Password Updated!")
                }
            }
        }
    }
}

/// A launched coroutine.
#[derive(Clone, Debug, PartialEq)]
pub enum Job<P> {
    /// `csvmodcr`'s file walk.
    CsvModCr { player: P, filename: String },
}

/// The coroutine's body: the file under `ServerConstants.DATA_PATH`, each
/// line not starting with `#` as `name,amount` handed to the registry's
/// `modcr` body (without the rights check), which may reject.
pub fn run_job<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    job: &Job<H::Player>,
) -> Result<()> {
    match job {
        Job::CsvModCr { player, filename } => {
            let data = host.data_path();
            let Some(lines) = host.file_lines(data.as_deref(), filename)? else {
                notify(host, player, &format!("Unable to locate file {filename}."))?;
                let data = host.data_path();
                notify(
                    host,
                    player,
                    &format!(
                        "Note: The file must be in {}",
                        data.as_deref().unwrap_or("null")
                    ),
                )?;
                return Ok(());
            };
            for line in lines {
                if line.starts_with('#') {
                    continue;
                }
                let tokens: Vec<&str> = line.split(',').collect();
                let username = kotlin_trim(&lowercase_root(tokens[0])).replace(' ', "_");
                let amount = kotlin_trim(super::kotlin::split_get(&tokens, 1)?).to_owned();
                if let Some(command) = registry.get("modcr") {
                    let body: Body = command.body;
                    run(host, body, player, &[String::new(), username, amount])?;
                }
            }
            Ok(())
        }
    }
}
