//! `DisconnectionQueue.kt` and what drives it: `Player.clear`,
//! `Player.finishClear` with `sendLogoutEvents`, `Player.allowRemoval`,
//! `PacketDispatch.sendLogout`, and `IoSession.disconnect` with `getPlayer`.
//!
//! A library: nothing installs it. The queue and its counters are native;
//! what a leaving player's systems do is a required service of
//! [`Host`], called in the original order. No lock is held while a service
//! runs: a service may add to, remove from or walk the queue it was called
//! from, as `IoSession.disconnect` does through `Player.clear`.
use super::repository::{Error, Host, JavaHashMap, LogLevel, Repository, Result, SessionObject};
use crate::definitions::item::Shared;

/// The queue is looked at when `GameWorld.ticks % 3 == 0`, and a waiting
/// entry's counter grows by this much each time.
pub const UPDATE_INTERVAL: i32 = 3;
/// `secondsToTicks(30)`: the wait of a player whose `isAfkLogout` is true.
pub const AFK_LOGOUT_TICKS: i32 = seconds_to_ticks(30);
/// `secondsToTicks(5 * 60)`: every other player's.
pub const LOGOUT_TICKS: i32 = seconds_to_ticks(5 * 60);
/// The name the save task gives its thread.
pub const SAVE_THREAD_NAME: &str = "PlayerSave SQL";

/// `TickUtils.secondsToTicks`.
pub const fn seconds_to_ticks(seconds: i32) -> i32 {
    seconds.wrapping_mul(1000) / 600
}

struct Entry<E> {
    player: E,
    clear: bool,
}

struct State<E> {
    queue: JavaHashMap<Option<String>, Entry<E>>,
    timers: JavaHashMap<Option<String>, i32>,
}

/// The save `finish` hands to `TaskExecutor.executeSQL`.
pub struct SaveTask<E> {
    pub player: E,
}

impl<E: Clone + PartialEq> SaveTask<E> {
    /// The body of the task: name the thread, then save.
    pub fn run<H: Host<E>>(&self, host: &mut H, queue: &DisconnectionQueue<E>) -> Result<bool> {
        host.set_thread_name(SAVE_THREAD_NAME);
        queue.save(host, &self.player, true)
    }
}

/// `DisconnectionQueue`. A shared handle: clones are the same queue.
pub struct DisconnectionQueue<E> {
    state: Shared<State<E>>,
}

impl<E> Clone for DisconnectionQueue<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<E> Default for DisconnectionQueue<E> {
    fn default() -> Self {
        Self {
            state: Shared::new(State {
                queue: JavaHashMap::new(),
                timers: JavaHashMap::new(),
            }),
        }
    }
}

fn shown(text: &Option<String>) -> &str {
    text.as_deref().unwrap_or("null")
}

/// `catch (t: Throwable) { t.printStackTrace() }`.
fn catch_throwable<E, H: Host<E>>(host: &mut H, result: Result<()>) -> Result<()> {
    match result {
        Err(error) if error.is_throwable() => {
            host.print_stack_trace(&error);
            Ok(())
        }
        other => other,
    }
}

/// `catch (e: Exception) { e.printStackTrace() }`.
fn catch_exception<E, H: Host<E>>(host: &mut H, result: Result<()>) -> Result<()> {
    match result {
        Err(error) if error.is_exception() => {
            host.print_stack_trace(&error);
            Ok(())
        }
        other => other,
    }
}

impl<E: Clone + PartialEq> DisconnectionQueue<E> {
    /// `update()`: nothing unless the queue has an entry and the tick counter
    /// is a multiple of three. Then every entry present at that moment is
    /// finished if its player may be removed; otherwise its counter grows,
    /// and at the limit the player is cleared without the checks, the logout
    /// packet, the clan or the save.
    pub fn update<H: Host<E>>(&self, host: &mut H, repository: &Repository<E>) -> Result<()> {
        if self.is_empty() || host.ticks() % UPDATE_INTERVAL != 0 {
            return Ok(());
        }
        let entries: Vec<(Option<String>, E)> = {
            let state = self.state.read();
            state
                .queue
                .iter()
                .map(|(name, entry)| (name.clone(), entry.player.clone()))
                .collect()
        };
        for (name, player) in entries {
            if self.finish(host, repository, &player, false)? {
                // Only the entry goes; the counter stays for the name.
                self.state.write().queue.remove(&name);
                continue;
            }
            {
                let mut state = self.state.write();
                let waited = state.timers.get(&name).copied().unwrap_or(0);
                state
                    .timers
                    .put(name.clone(), waited.wrapping_add(UPDATE_INTERVAL));
            }
            let needed = if host.afk_logout(&player) == Some(true) {
                AFK_LOGOUT_TICKS
            } else {
                LOGOUT_TICKS
            };
            let waited = self.state.read().timers.get(&name).copied();
            if waited.unwrap_or(i32::MAX) >= needed {
                // try { finishClear() } finally { removePlayer(); remove(key) }
                let cleared = finish_clear(host, &player);
                repository.remove_player(host, &player)?;
                self.remove(name.as_deref());
                cleared?;
                host.log(
                    LogLevel::Warn,
                    &format!(
                        "Force-clearing {} after 15 minutes of being in the disconnection queue!",
                        shown(&name)
                    ),
                )?;
            }
        }
        Ok(())
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.state.read().queue.is_empty()
    }

    /// `finish(entry, force)`.
    fn finish<H: Host<E>>(
        &self,
        host: &mut H,
        repository: &Repository<E>,
        player: &E,
        force: bool,
    ) -> Result<bool> {
        if !force && !allow_removal(host, player)? {
            return Ok(false);
        }
        send_logout(host, repository, player)?;
        let cleared = finish_clear(host, player);
        catch_throwable(host, cleared)?;
        repository.remove_player(host, player)?;
        let left = leave_clan(host, player);
        catch_exception(host, left)?;
        if host.is_artificial(player) {
            return Ok(true);
        }
        if !force {
            host.execute_sql(SaveTask {
                player: player.clone(),
            })?;
        } else {
            self.save(host, player, false)?;
        }
        let username = host.details_username(player);
        host.log(
            LogLevel::Info,
            &format!("Player cleared. Removed {}.", shown(&username)),
        )?;
        Ok(true)
    }

    /// `get(name)`.
    pub fn get(&self, name: Option<&str>) -> Option<E> {
        let state = self.state.read();
        state
            .queue
            .get(&name.map(str::to_owned))
            .map(|entry| entry.player.clone())
    }

    /// `clear()`: every entry finished by force, then the entries dropped.
    /// The counters stay.
    pub fn clear<H: Host<E>>(&self, host: &mut H, repository: &Repository<E>) -> Result<()> {
        let players: Vec<E> = {
            let state = self.state.read();
            state
                .queue
                .iter()
                .map(|(_, entry)| entry.player.clone())
                .collect()
        };
        for player in players {
            self.finish(host, repository, &player, true)?;
        }
        self.state.write().queue.clear();
        Ok(())
    }

    /// `add(player, clear)`: nothing if the player's name is queued already.
    /// The name is read three times; a null name is a NullPointerException,
    /// so the queue never holds one.
    pub fn add<H: Host<E>>(&self, host: &mut H, player: &E, clear: bool) -> Result<()> {
        // Kotlin checks the platform-typed name where it is used as a key.
        let name = host.name(player).ok_or(Error::NullPointer)?;
        if self.state.read().queue.contains_key(&Some(name)) {
            return Ok(());
        }
        let name = host.name(player).ok_or(Error::NullPointer)?;
        self.state.write().queue.put(
            Some(name),
            Entry {
                player: player.clone(),
                clear,
            },
        );
        let name = host.name(player);
        host.log(
            LogLevel::Info,
            &format!("Queueing {} for disconnection.", shown(&name)),
        )
    }

    /// `contains(name)`.
    pub fn contains(&self, name: Option<&str>) -> bool {
        self.state
            .read()
            .queue
            .contains_key(&name.map(str::to_owned))
    }

    /// `remove(name)`: the entry and the counter.
    pub fn remove(&self, name: Option<&str>) {
        let name = name.map(str::to_owned);
        let mut state = self.state.write();
        state.queue.remove(&name);
        state.timers.remove(&name);
    }

    /// `save(player, sql)`: a failed save is printed and forgotten, and the
    /// answer is always false.
    pub fn save<H: Host<E>>(&self, host: &mut H, player: &E, _sql: bool) -> Result<bool> {
        let saved = host.save_immediately(player);
        catch_throwable(host, saved)?;
        Ok(false)
    }

    /// The entries in the order `update` and `clear` walk them: name, player
    /// and the `isClear` flag, which the original stores and never reads.
    pub fn entries(&self) -> Vec<(Option<String>, E, bool)> {
        let state = self.state.read();
        state
            .queue
            .iter()
            .map(|(name, entry)| (name.clone(), entry.player.clone(), entry.clear))
            .collect()
    }

    /// The counters in the order of their map.
    pub fn timers(&self) -> Vec<(Option<String>, i32)> {
        let state = self.state.read();
        state
            .timers
            .iter()
            .map(|(name, waited)| (name.clone(), *waited))
            .collect()
    }
}

/// The clan part of `finish`.
fn leave_clan<E, H: Host<E>>(host: &mut H, player: &E) -> Result<()> {
    if host.clan(player)?.is_some() {
        let clan = host.clan(player)?.ok_or(Error::NullPointer)?;
        host.clan_leave(&clan, player, false)?;
    }
    Ok(())
}

/// `Player.allowRemoval()`.
pub fn allow_removal<E, H: Host<E>>(host: &mut H, player: &E) -> Result<bool> {
    Ok(!(host.in_combat(player)?
        || host.lifepoints(player)? < 1
        || host.is_dead(player)?
        || host.is_teleporting(player)?
        || host.has_soft_script(player)?))
}

/// `PacketDispatch.sendLogout()`: queue the player, then send the packet.
pub fn send_logout<E: Clone + PartialEq, H: Host<E>>(
    host: &mut H,
    repository: &Repository<E>,
    player: &E,
) -> Result<()> {
    repository.disconnection_queue().add(host, player, false)?;
    host.logout_packet(player)
}

/// `Player.clear()`: an artificial player is cleared at once; any other is
/// queued again from scratch, and its account details are saved.
pub fn player_clear<E: Clone + PartialEq, H: Host<E>>(
    host: &mut H,
    repository: &Repository<E>,
    player: &E,
) -> Result<()> {
    if host.is_artificial(player) {
        return finish_clear(host, player);
    }
    let queue = repository.disconnection_queue();
    let name = host.name(player);
    queue.remove(name.as_deref());
    queue.add(host, player, true)?;
    host.details_save(player)
}

/// `Player.finishClear()`.
pub fn finish_clear<E, H: Host<E>>(host: &mut H, player: &E) -> Result<()> {
    if !host.is_artificial(player) {
        host.world_logout_listeners(player)?;
    }
    host.set_playing(player, false);
    host.walking_queue_reset(player)?;
    host.player_logout_listeners(player)?;
    if host.has_familiar(player)? {
        host.familiar_clear(player)?;
    }
    host.interface_close(player)?;
    host.interface_close_single_tab(player)?;
    host.entity_clear(player)?;
    host.zone_monitor_clear(player)?;
    host.house_leave(player)?;
    host.renderable_players_remove(player)?;
    send_logout_events(host, player)?;
    host.wealth_update(player, true)
}

/// `Player.sendLogoutEvents()`. The generated builders refuse a null string.
fn send_logout_events<E, H: Host<E>>(host: &mut H, player: &E) -> Result<()> {
    let name = host.name(player).ok_or(Error::NullPointer)?;
    host.publish_player_status(&name, 0, false)?;
    if host.clan(player)?.is_some() {
        let name = host.name(player).ok_or(Error::NullPointer)?;
        let world = host.world_id()?;
        let clan = host.clan(player)?.ok_or(Error::NullPointer)?;
        let owner = host.clan_owner(&clan)?.ok_or(Error::NullPointer)?;
        host.publish_clan_leave(&name, world, &owner)?;
    }
    Ok(())
}

/// `IoSession.getPlayer()`: an empty `object` is first filled from the name
/// map by the session's user name.
pub fn session_player<E: Clone + PartialEq, H: Host<E>>(
    host: &mut H,
    repository: &Repository<E>,
    session: &H::Session,
) -> Option<E> {
    match host.session_object(session) {
        SessionObject::Player(player) => Some(player),
        SessionObject::Other => None,
        SessionObject::Null => {
            let username = host.session_username(session);
            let found = repository.get_player_by_name(host, username.as_deref());
            host.set_session_object(session, found.clone());
            found
        }
    }
}

/// `getPlayer().clear()` after `getPlayer() != null`.
fn clear_session_player<E: Clone + PartialEq, H: Host<E>>(
    host: &mut H,
    repository: &Repository<E>,
    session: &H::Session,
) -> Result<()> {
    let player = session_player(host, repository, session).ok_or(Error::NullPointer)?;
    host.player_clear(repository, &player)
}

/// `IoSession.disconnect()`. An inactive session does nothing. Otherwise the
/// session goes inactive, its key and socket are closed, its player is
/// cleared (an `Exception` from that is printed) and `object` is emptied. An
/// `IOException` from the socket is printed, the player is still cleared,
/// this time without a catch, and `object` is left as it is.
pub fn session_disconnect<E: Clone + PartialEq, H: Host<E>>(
    host: &mut H,
    repository: &Repository<E>,
    session: &H::Session,
) -> Result<()> {
    match disconnect(host, repository, session) {
        Err(error) if error.is_io() => {
            host.print_stack_trace(&error);
            if session_player(host, repository, session).is_some() {
                clear_session_player(host, repository, session)?;
            }
            Ok(())
        }
        other => other,
    }
}

fn disconnect<E: Clone + PartialEq, H: Host<E>>(
    host: &mut H,
    repository: &Repository<E>,
    session: &H::Session,
) -> Result<()> {
    if !host.session_active(session) {
        return Ok(());
    }
    host.set_session_active(session, false);
    if host.session_has_key(session) {
        host.key_cancel(session)?;
        host.socket_close(session)?;
    }
    if session_player(host, repository, session).is_some() {
        let cleared = clear_session_player(host, repository, session);
        catch_exception(host, cleared)?;
    }
    host.set_session_object(session, None);
    Ok(())
}
