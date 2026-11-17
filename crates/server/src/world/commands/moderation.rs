//! `ModerationCommandSet`: kick, ban, ipban, mute, jail, modcr, csvmodcr and
//! its `getattribute` (which `PlayerAttributeCommandSet` registers over).
use super::deferred::{Job, Pulse};
use super::kotlin::{TimeUnit, is_digit_token, to_int_or_null};
use super::{Error, Host, Result, arg, notify, reject, rights};
use crate::auth::storage::lock;
use crate::player::details::Rights;
use rs09_game_core::java_lower::lowercase_root;

/// `MAX_JAIL_TIME`, in seconds.
pub const MAX_JAIL_TIME: i32 = 1800;
/// The jail cells in Varrock.
pub const JAIL: [i32; 3] = [3228, 3407, 0];

/// The duration loop of ban, ipban and mute: digits gather into a token
/// that is never cleared; every other character picks the unit and
/// converts the token so far.
fn duration<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    text: &str,
) -> Result<(String, i64, TimeUnit)> {
    let mut token = String::new();
    let mut millis = 0i64;
    let mut unit = TimeUnit::Nanoseconds;
    for ch in text.chars() {
        if is_digit_token(ch) {
            token.push(ch);
        } else {
            let value = to_int_or_null(&token).unwrap_or(-1);
            if value == -1 {
                reject::<_, ()>(host, player, &[&format!("Invalid duration: {token}")])?;
            }
            unit = match ch {
                'd' => TimeUnit::Days,
                's' => TimeUnit::Seconds,
                'm' => TimeUnit::Minutes,
                'h' => TimeUnit::Hours,
                _ => TimeUnit::Seconds,
            };
            millis = unit.to_millis(i64::from(value));
        }
    }
    Ok((token, millis, unit))
}

/// `::kick <player>`.
pub fn kick<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let name = arg(args, 1)?.to_owned();
    match host.player_by_name(Some(&name))? {
        Some(target) => {
            host.clear(&target)?;
            let username = host.username(&target);
            notify(host, player, &format!("Player {username} was kicked."))
        }
        None => reject(host, player, &["ERROR REMOVING PLAYER."]),
    }
}

/// `::ban <name> <duration>`.
pub fn ban<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let name = arg(args, 1)?.to_owned();
    if !host.username_taken(&name)? {
        reject::<_, ()>(host, player, &[&format!("Invalid username: {name}")])?;
    }
    let target = host.player_by_name(Some(&name))?;
    let text = arg(args, 2)?.to_owned();
    let (token, millis, unit) = duration(host, player, &text)?;
    if let Some(target) = &target {
        let account = host.account(target);
        let end = host.now().wrapping_add(millis);
        lock(&account).ban_end_time = end;
        host.clear(target)?;
    }
    host.submit_pulse(Pulse::BanStore {
        name: Some(name.clone()),
        millis,
    })?;
    notify(
        host,
        player,
        &format!("Banned user {name} for {token} {}.", unit.lower_name()),
    )
}

/// `::ipban <ip> <duration>`.
pub fn ipban<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let ip = arg(args, 1)?.to_owned();
    let text = arg(args, 2)?.to_owned();
    let (token, millis, unit) = duration(host, player, &text)?;
    let names = host.usernames_with_ip(&ip)?;
    if names.is_empty() {
        reject::<_, ()>(host, player, &[&format!("No accounts found on IP {ip}")])?;
    }
    for name in names {
        let target = host.player_by_name(name.as_deref())?;
        if let Some(target) = &target {
            let account = host.account(target);
            let end = host.now().wrapping_add(millis);
            lock(&account).ban_end_time = end;
            host.clear(target)?;
        }
        host.submit_pulse(Pulse::IpBanStore { name, millis })?;
    }
    host.store_add_to_list("flagged-ips", "ips", &ip)?;
    notify(
        host,
        player,
        &format!(
            "Banned all accounts on {ip} for {token} {}.",
            unit.lower_name()
        ),
    )
}

/// `::mute <name> <duration>`.
pub fn mute<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let name = arg(args, 1)?.to_owned();
    if !host.username_taken(&name)? {
        reject::<_, ()>(host, player, &[&format!("Invalid username: {name}")])?;
    }
    let target = host.player_by_name(Some(&name))?;
    let text = arg(args, 2)?.to_owned();
    let (token, millis, unit) = duration(host, player, &text)?;
    match &target {
        Some(target) => {
            let account = host.account(target);
            let end = host.now().wrapping_add(millis);
            lock(&account).mute_end_time = end;
        }
        None => host.submit_pulse(Pulse::MuteStore {
            name: name.clone(),
            millis,
        })?,
    }
    notify(
        host,
        player,
        &format!("Muted user {name} for {token} {}.", unit.lower_name()),
    )
}

/// `::jail <seconds> <name...>`.
pub fn jail<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 3 {
        reject::<_, ()>(host, player, &["Usage: ::jail <seconds> <player>"])?;
    }
    let seconds = to_int_or_null(arg(args, 1)?);
    let Some(seconds) = seconds else {
        reject::<_, ()>(host, player, &["<seconds> Must be a valid integer!"])?;
        return Err(Error::Null);
    };
    if seconds > MAX_JAIL_TIME {
        reject::<_, ()>(
            host,
            player,
            &[&format!("Maximum jail time is {MAX_JAIL_TIME} seconds.")],
        )?;
    }
    let name = args[2..].join("_");
    let other = host.player_by_name(Some(&name))?;
    if other.is_none() {
        reject::<_, ()>(
            host,
            player,
            &[&format!("Can not find {name} in the player list!")],
        )?;
    }
    if let Some(other) = &other
        && rights(host, other)? == Rights::Administrator
    {
        let username = host.username(player);
        reject::<_, ()>(
            host,
            player,
            &[&format!(
                "You cannot jail {name}, they are a god. Nice try though {username}!"
            )],
        )?;
    }
    let other = other.ok_or(Error::Null)?;
    let other_name = host.username(&other);
    notify(
        host,
        player,
        &format!("Jailing {other_name} for {seconds} seconds."),
    )?;
    let username = host.username(player);
    notify(
        host,
        &other,
        &format!("{username} has jailed you for {seconds} seconds."),
    )?;
    let original = host.location(&other)?;
    let release = host
        .now()
        .wrapping_add(TimeUnit::Seconds.to_millis(i64::from(seconds)));
    host.submit_pulse(Pulse::Jail {
        other,
        original,
        release,
    })
}

/// `::modcr <name> <amount>`.
pub fn modcr<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let username = lowercase_root(args.get(1).map_or("", String::as_str));
    let amount = args
        .get(2)
        .and_then(|a| to_int_or_null(a))
        .unwrap_or(i32::MIN);
    if username.is_empty() {
        return reject(host, player, &["Usage: ::modcr user_name amount"]);
    }
    if amount == i32::MIN {
        return reject(host, player, &["Usage ::modcr user_name amount"]);
    }
    let online = host.player_by_name(Some(&username))?;
    let info = match &online {
        None => host.account_info(Some(&username))?,
        Some(p) => host.account(p),
    };
    if lock(&info).is_default() {
        return reject(
            host,
            player,
            &[&format!(
                "The user you specified ({username}) does not exist."
            )],
        );
    }
    let credits = {
        let mut fields = lock(&info);
        fields.credits = fields.credits.wrapping_add(amount);
        fields.credits
    };
    match &online {
        None => host.account_update(&info)?,
        Some(p) => host.api_message(
            p,
            &format!(
                "You have been {} {} credit(s).",
                if amount > 0 { "granted" } else { "penalized" },
                amount.wrapping_abs()
            ),
        )?,
    }
    notify(
        host,
        player,
        &format!(
            "Updated {username}'s credits to {credits} by {} {}.",
            if amount > 0 { "adding" } else { "removing" },
            amount.wrapping_abs()
        ),
    )
}

/// `::csvmodcr <file>`: the work runs in the launched coroutine,
/// [`super::deferred::run_job`].
pub fn csvmodcr<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let filename = args.get(1).cloned().unwrap_or_default();
    if filename.is_empty() {
        return reject(host, player, &["Usage: ::csvmodcr filename"]);
    }
    host.launch(Job::CsvModCr {
        player: player.clone(),
        filename,
    })
}

/// `ModerationCommandSet`'s `::getattribute <name> <attribute>`.
pub fn get_attribute<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    let username = args.get(1).cloned().unwrap_or_default();
    let attribute = args.get(2).cloned().unwrap_or_default();
    if username.is_empty() || attribute.is_empty() {
        return reject(host, player, &["Usage: ::getattribute username attribute"]);
    }
    let Some(target) = host.player_by_name(Some(&username))? else {
        return reject(
            host,
            player,
            &[&format!("Unable to find player {username}.")],
        );
    };
    let Some(value) = host.attribute(&target, &attribute)? else {
        return reject(
            host,
            player,
            &[&format!(
                "Unable to find attribute {attribute} for player {username}."
            )],
        );
    };
    notify(
        host,
        player,
        &format!(
            "Attribute {attribute} for player {username} is {}.",
            value.java_string()
        ),
    )
}
