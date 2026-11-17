//! The system manager: `SystemManager` (the state and its transitions),
//! `SystemUpdate` (the update countdown pulse), `SystemTermination` (the
//! shutdown sequence that saves and clears every player), the JVM
//! shutdown hook `SystemShutdownHook` and `SystemConfig`.
//!
//! | Original | Here |
//! | --- | --- |
//! | `SystemManager.flag`, `isActive`/`isUpdating`/`isPrivate`/`isTerminated`, `state` | [`Manager`] |
//! | `SystemUpdate` (setCountdown, schedule, notifyPlayers, pulse, cancel, the backup flag) | [`Update`], [`Manager::update`] |
//! | `SystemTermination.terminate`, `save` | [`Manager::terminate`], [`Manager::save`] |
//! | `SystemShutdownHook.run` | [`Manager::shutdown_hook`] |
//! | `SystemConfig` | [`Config`] |
//!
//! The manager is a world value (the original's statics). The updater's
//! pulse fields are a native `pulses::Pulse` handle so the world's pulse
//! queue can hold it; [`Manager::update`] is `Pulse.update` for it (its
//! checks are null) and [`Manager::pulse`] the virtual `pulse()` a world
//! dispatches to by that handle's identity. Every call into another
//! system (the tick worker, the pulse runner, the repository, packets,
//! networking, bots, listeners, stores, PlayerMonitor, the disconnection
//! queue, the file system, the clock) is a required [`Host`] method with
//! no default. The `::update`, `::cancelupdate` and `::shutdown` command
//! bodies (world/commands/system.rs) reach this module through the
//! commands host. See docs/command-sets.md.
use crate::definitions::item::Shared;
use crate::pulses::{Class, Handle, Pulse};
use crate::world::commands::{Error, Result};

/// `SystemUpdate.DEFAULT_COUNTDOWN`.
pub const DEFAULT_COUNTDOWN: i32 = 100;
/// `SystemUpdate.BACKUP_TICK`.
pub const BACKUP_TICK: i32 = 10;
/// The classes the logs name.
pub const TERMINATION: &str = "core.game.system.SystemTermination";
pub const SHUTDOWN_HOOK: &str = "core.game.system.SystemShutdownHook";
/// The updater's class.
pub const UPDATE_CLASS: &str = "core.game.system.SystemUpdate";

/// `SystemState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Active,
    Updating,
    Private,
    Terminated,
}
impl State {
    pub fn name(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Updating => "UPDATING",
            Self::Private => "PRIVATE",
            Self::Terminated => "TERMINATED",
        }
    }
}

/// A world store `GameWorld.worldPersists` holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Persist {
    /// The host's name for it.
    pub name: String,
    /// `wld instanceof ServerStore`.
    pub server_store: bool,
}

/// What the manager needs from the world. No defaults: methods without a
/// `Result` are field or constant reads; every other one is a call into a
/// system this module does not port.
pub trait Host {
    type Player: Clone;
    /// `GameWorld.getMajorUpdateWorker().getStarted()`.
    fn worker_started(&mut self) -> bool;
    /// `setStarted(started)`.
    fn set_worker_started(&mut self, started: bool) -> Result<()>;
    fn worker_start(&mut self) -> Result<()>;
    fn worker_stop(&mut self) -> Result<()>;
    /// `Repository.getPlayers()`, in its walk order.
    fn players(&mut self) -> Vec<Self::Player>;
    /// `player.getPacketDispatch().sendSystemUpdate(time)`.
    fn send_system_update(&mut self, player: &Self::Player, time: i32) -> Result<()>;
    /// `GameWorld.getPulser().submit(updater)`.
    fn submit_update(&mut self, pulse: &Handle) -> Result<()>;
    /// `schedule`'s own thread when the tick worker is not running: it
    /// calls [`Manager::update`] every 600 ms while the updater runs.
    fn start_update_thread(&mut self, pulse: &Handle) -> Result<()>;
    /// `core.api.log(origin, level, message)` at INFO.
    fn log(&mut self, origin: &str, message: &str) -> Result<()>;
    /// `ServerConstants.DATA_PATH`.
    fn data_path(&mut self) -> Option<String>;
    /// `Server.setRunning(running)`.
    fn set_server_running(&mut self, running: bool) -> Result<()>;
    /// `AIRepository.clearAllBots()`.
    fn clear_all_bots(&mut self) -> Result<()>;
    /// `Server.getReactor().terminate()`.
    fn terminate_reactor(&mut self) -> Result<()>;
    /// `Server.getWebSocketServer() != null`.
    fn websocket_present(&mut self) -> bool;
    fn stop_websocket(&mut self, timeout: i32, reason: &str) -> Result<()>;
    /// `Server.setWebSocketServer(null)`.
    fn clear_websocket(&mut self) -> Result<()>;
    fn is_artificial(&mut self, player: &Self::Player) -> Result<bool>;
    /// `player.getDetails().save()`.
    fn save_details(&mut self, player: &Self::Player) -> Result<()>;
    /// `player.clear()`.
    fn clear_player(&mut self, player: &Self::Player) -> Result<()>;
    /// `GameWorld.getShutdownListeners()`, by name.
    fn shutdown_listeners(&mut self) -> Vec<String>;
    fn shutdown_listener(&mut self, listener: &str) -> Result<()>;
    /// `PlayerMonitor.flushRemainingEventsImmediately()`.
    fn monitor_flush(&mut self) -> Result<()>;
    /// `GameWorld.getWorldPersists()`.
    fn world_persists(&mut self) -> Vec<Persist>;
    fn persist_save(&mut self, store: &Persist) -> Result<()>;
    /// `new File(directory).getAbsolutePath()`.
    fn absolute_path(&mut self, directory: &str) -> String;
    fn is_directory(&mut self, directory: &str) -> bool;
    fn mkdirs(&mut self, directory: &str);
    /// `System.currentTimeMillis()`.
    fn now(&mut self) -> i64;
    /// `Repository.getDisconnectionQueue().isEmpty()`, `update()`, `clear()`.
    fn queue_empty(&mut self) -> Result<bool>;
    fn queue_update(&mut self) -> Result<()>;
    fn queue_clear(&mut self) -> Result<()>;
    /// `Thread.sleep(millis)`.
    fn sleep(&mut self, millis: i64);
    /// `e.printStackTrace()`.
    fn print_stack_trace(&mut self, error: &Error);
}

/// `SystemUpdate`: its pulse fields and the backup flag.
#[derive(Clone)]
pub struct Update {
    pub pulse: Handle,
    pub create_backup: bool,
}
impl Update {
    pub fn new() -> Self {
        Self {
            pulse: Shared::new(Pulse::new(
                DEFAULT_COUNTDOWN,
                None,
                Class::Other,
                UPDATE_CLASS.into(),
            )),
            create_backup: false,
        }
    }
    pub fn delay(&self) -> i32 {
        self.pulse.read().delay
    }
    pub fn ticks_passed(&self) -> i32 {
        self.pulse.read().ticks_passed
    }
    pub fn is_running(&self) -> bool {
        self.pulse.read().running
    }
    /// `setCountdown(ticks)`.
    pub fn set_countdown(&mut self, mut ticks: i32) {
        if self.create_backup {
            if ticks < BACKUP_TICK {
                ticks = BACKUP_TICK;
            }
            ticks -= BACKUP_TICK;
        }
        self.pulse.write().delay = ticks;
    }
}
impl Default for Update {
    fn default() -> Self {
        Self::new()
    }
}

/// `SystemConfig`. `parse()` is empty in the original and `parseConfig`
/// is private and never called, so the maps stay empty.
#[derive(Clone, Debug, Default)]
pub struct Config {
    beta_users: Vec<String>,
}
impl Config {
    /// `validLogin(player)`.
    pub fn valid_login(&self) -> bool {
        true
    }
    /// `isDoubleExp()`: `getConfig("dxp", null)` is null.
    pub fn is_double_exp(&self) -> bool {
        false
    }
    /// `isBetaUser(name)`.
    pub fn is_beta_user(&self, name: &str) -> bool {
        self.beta_users.iter().any(|n| n == name)
    }
    /// `getConfigs().size()` and `getBetaUsers().size()`.
    pub fn sizes(&self) -> (usize, usize) {
        (0, self.beta_users.len())
    }
    /// `split(data, regex)`. The regex is matched literally; one holding a
    /// metacharacter is refused (no caller passes one).
    pub fn split(&self, data: &str, regex: &str) -> Result<Vec<String>> {
        if !data.contains(regex) {
            return Ok(vec![data.to_owned()]);
        }
        if regex.is_empty() || regex.chars().any(|c| ".$|()[{^?*+\\".contains(c)) {
            return Err(Error::Binding(Some(format!(
                "not ported: SystemConfig.split with the regular expression {regex:?}"
            ))));
        }
        let trimmed = data.trim_matches(|c: char| c <= ' ');
        let mut parts: Vec<String> = trimmed.split(regex).map(str::to_owned).collect();
        if !trimmed.contains(regex) {
            return Ok(vec![trimmed.to_owned()]);
        }
        while parts.last().is_some_and(String::is_empty) {
            parts.pop();
        }
        Ok(parts)
    }
}

/// `SystemManager` with its updater and config.
#[derive(Clone)]
pub struct Manager {
    state: State,
    pub updater: Update,
    pub config: Config,
}
impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}
impl Manager {
    /// The class's initial values: TERMINATED, a fresh updater.
    pub fn new() -> Self {
        Self {
            state: State::Terminated,
            updater: Update::new(),
            config: Config::default(),
        }
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn is_active(&self) -> bool {
        self.state != State::Terminated
    }
    pub fn is_updating(&self) -> bool {
        self.state == State::Updating
    }
    pub fn is_private(&self) -> bool {
        self.state == State::Private
    }
    pub fn is_terminated(&self) -> bool {
        self.state == State::Terminated
    }

    /// `SystemManager.flag(state)`: the state is set before its handling.
    pub fn flag<H: Host + ?Sized>(&mut self, host: &mut H, state: State) -> Result<()> {
        if self.state == state {
            return Ok(());
        }
        self.state = state;
        match state {
            State::Active | State::Private => {
                host.set_worker_started(false)?;
                host.worker_start()
            }
            State::Updating => self.schedule(host),
            State::Terminated => self.terminate(host),
        }
    }

    /// `SystemUpdate.schedule()`.
    pub fn schedule<H: Host + ?Sized>(&mut self, host: &mut H) -> Result<()> {
        {
            let mut pulse = self.updater.pulse.write();
            pulse.ticks_passed = 0;
            pulse.running = true;
        }
        if host.worker_started() {
            self.notify_players(host);
            return host.submit_update(&self.updater.pulse);
        }
        host.start_update_thread(&self.updater.pulse)
    }

    /// `SystemUpdate.notifyPlayers()`: failures are printed.
    pub fn notify_players<H: Host + ?Sized>(&mut self, host: &mut H) {
        let time = self.updater.delay()
            + if self.updater.create_backup {
                BACKUP_TICK
            } else {
                0
            };
        for player in host.players() {
            if let Err(error) = host.send_system_update(&player, time) {
                host.print_stack_trace(&error);
                return;
            }
        }
    }

    /// `SystemUpdate.cancel()`.
    pub fn cancel<H: Host + ?Sized>(&mut self, host: &mut H) -> Result<()> {
        self.updater.pulse.write().running = false;
        self.flag(host, State::Active)
    }

    /// `SystemUpdate.pulse()`.
    pub fn pulse<H: Host + ?Sized>(&mut self, host: &mut H) -> Result<bool> {
        if self.updater.delay() >= BACKUP_TICK && self.updater.create_backup {
            let saved = match host.data_path() {
                None => Err(Error::Null),
                Some(path) => self.save(host, &path),
            };
            if let Err(error) = saved {
                host.print_stack_trace(&error);
            }
            self.updater.pulse.write().delay = BACKUP_TICK - 1;
            return Ok(false);
        }
        self.flag(host, State::Terminated)?;
        Ok(true)
    }

    /// `Pulse.update()` for the updater (its checks are null): true when
    /// it is finished.
    pub fn update<H: Host + ?Sized>(&mut self, host: &mut H) -> bool {
        if !self.updater.is_running() {
            return true;
        }
        let due = {
            let mut pulse = self.updater.pulse.write();
            pulse.ticks_passed = pulse.ticks_passed.wrapping_add(1);
            pulse.ticks_passed >= pulse.delay
        };
        if !due {
            return false;
        }
        self.updater.pulse.write().ticks_passed = 0;
        match self.pulse(host) {
            Ok(true) => {
                self.updater.pulse.write().running = false;
                true
            }
            Ok(false) => !self.updater.is_running(),
            Err(error) => {
                host.print_stack_trace(&error);
                self.updater.pulse.write().running = false;
                true
            }
        }
    }

    /// `SystemTermination.terminate()`: everything between the first and
    /// the last log line is one try whose failure is printed.
    pub fn terminate<H: Host + ?Sized>(&mut self, host: &mut H) -> Result<()> {
        host.log(
            TERMINATION,
            "Initializing termination sequence - do not shutdown!",
        )?;
        if let Err(error) = self.terminate_body(host) {
            host.print_stack_trace(&error);
        }
        host.log(TERMINATION, "Server successfully terminated!")
    }

    fn terminate_body<H: Host + ?Sized>(&mut self, host: &mut H) -> Result<()> {
        host.log(TERMINATION, "Shutting down networking...")?;
        host.set_server_running(false)?;
        host.log(TERMINATION, "Stopping all bots...")?;
        host.clear_all_bots()?;
        host.terminate_reactor()?;
        if host.websocket_present() {
            host.stop_websocket(1000, "Server shutting down")?;
            host.clear_websocket()?;
        }
        host.log(TERMINATION, "Stopping all pulses...")?;
        host.worker_stop()?;
        for player in host.players() {
            let saved = (|| {
                if !host.is_artificial(&player)? {
                    host.save_details(&player)?;
                    host.clear_player(&player)?;
                }
                Ok(())
            })();
            if let Err(error) = saved {
                host.print_stack_trace(&error);
            }
        }
        for listener in host.shutdown_listeners() {
            host.shutdown_listener(&listener)?;
        }
        host.monitor_flush()?;
        let mut store = None;
        for world in host.world_persists() {
            if world.server_store {
                store = Some(world);
            } else {
                host.persist_save(&world)?;
            }
        }
        // ServerStore always saves last.
        if let Some(store) = store {
            host.persist_save(&store)?;
        }
        if let Some(path) = host.data_path() {
            self.save(host, &path)?;
        }
        Ok(())
    }

    /// `SystemTermination.save(directory)`.
    pub fn save<H: Host + ?Sized>(&mut self, host: &mut H, directory: &str) -> Result<()> {
        let absolute = host.absolute_path(directory);
        host.log(TERMINATION, &format!("Saving data [dir={absolute}]..."))?;
        if !host.is_directory(directory) {
            host.mkdirs(directory);
        }
        host.terminate_reactor()?;
        let start = host.now();
        while !host.queue_empty()? && host.now().wrapping_sub(start) < 5000 {
            host.queue_update()?;
            host.sleep(100);
        }
        host.queue_update()?;
        host.queue_clear()
    }

    /// `SystemShutdownHook.run()`.
    pub fn shutdown_hook<H: Host + ?Sized>(&mut self, host: &mut H) -> Result<()> {
        if self.is_terminated() {
            return Ok(());
        }
        host.log(SHUTDOWN_HOOK, "Terminating...")?;
        self.flag(host, State::Terminated)
    }
}
