//! `GeneralBotCreator.kt`: its three constructors, `BotScriptPulse`,
//! `TransitionPulse` and the `GBCTick` tick listener.
//!
//! A `BotScriptPulse` runs every tick. It waits out its random delay, stops
//! a movement pulse whose bot has not moved for more than five ticks, closes
//! what blocks a paused interaction, and then, while the bot has neither a
//! pulse nor an active interaction, lets at most 75 bots a tick run their
//! script, each after a one-in-ten roll for a pause of 20 to 49 ticks that a
//! high-priority bot and an `Idler` skip.
use super::{AiPlayer, Error, Host, Location, Pulse, Result, Script, loc, username_key};
use crate::definitions::item::Shared;

/// The fields of a `BotScriptPulse`.
pub struct ScriptPulseState<H: Host> {
    /// `botScript`.
    pub bot_script: Script<H>,
    /// `isPlayer`.
    pub is_player: bool,
    /// `ticks`.
    pub ticks: i32,
    /// `randomDelay`.
    pub random_delay: i32,
    /// `lastBotLocation`.
    pub last_bot_location: Location,
    /// `lastBotMoveTicks`.
    pub last_bot_move_ticks: i32,
}

/// A `BotScriptPulse` object: a shared handle (`AIRepository.PulseRepository`
/// and the `botting:script` attribute hold the one the pulser runs).
pub struct ScriptPulse<H: Host>(Shared<ScriptPulseState<H>>);
impl<H: Host> Clone for ScriptPulse<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<H: Host> ScriptPulse<H> {
    /// `BotScriptPulse(botScript, isPlayer)`: `Pulse(1)`, then in member
    /// order `ticks`, the `init` block (`botScript.init(isPlayer)`),
    /// `randomDelay`, a copy of the bot's location and the world's ticks.
    pub fn new(host: &mut H, bot_script: &Script<H>, is_player: bool) -> Result<Self> {
        bot_script.init(host, is_player)?;
        let bot = bot_script.bot().ok_or(Error::Null)?;
        let last_bot_location = loc(host, &bot)?.transform(0, 0, 0);
        let last_bot_move_ticks = host.world_ticks()?;
        Ok(Self(Shared::new(ScriptPulseState {
            bot_script: bot_script.clone(),
            is_player,
            ticks: 0,
            random_delay: 0,
            last_bot_location,
            last_bot_move_ticks,
        })))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn script(&self) -> Script<H> {
        self.0.read().bot_script.clone()
    }
    pub fn is_player(&self) -> bool {
        self.0.read().is_player
    }
    pub fn ticks(&self) -> i32 {
        self.0.read().ticks
    }
    pub fn random_delay(&self) -> i32 {
        self.0.read().random_delay
    }
    /// `randomDelay = delay`.
    pub fn set_random_delay(&self, delay: i32) {
        self.0.write().random_delay = delay;
    }
    pub fn last_bot_location(&self) -> Location {
        self.0.read().last_bot_location
    }
    pub fn last_bot_move_ticks(&self) -> i32 {
        self.0.read().last_bot_move_ticks
    }

    /// `pulse()`.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        let script = self.script();
        // `(botScript.bot as? AIPlayer)?.isHighPriority == true`
        let is_priority = script
            .bot()
            .and_then(|bot| host.ai_player(&bot))
            .is_some_and(|ai| ai.is_high_priority());
        if self.random_delay() > 0 {
            let mut state = self.0.write();
            state.random_delay = state.random_delay.wrapping_sub(1);
            return Ok(false);
        }
        let bot = script.bot().ok_or(Error::Null)?;
        if host.pulse_manager_has_pulse_running(&bot)? {
            let current = host.pulse_manager_current(&bot)?;
            if current.is_some_and(|pulse| host.is_movement_pulse(&pulse)) {
                if host.location(&bot) != Some(self.last_bot_location()) {
                    let copy = loc(host, &bot)?.transform(0, 0, 0);
                    self.0.write().last_bot_location = copy;
                    let ticks = host.world_ticks()?;
                    self.0.write().last_bot_move_ticks = ticks;
                }
                if Some(self.last_bot_location()) == host.location(&bot)
                    && host.world_ticks()?.wrapping_sub(self.last_bot_move_ticks()) > 5
                {
                    let current = host.pulse_manager_current(&bot)?.ok_or(Error::Null)?;
                    host.pulse_stop(&current)?;
                }
            }
        }
        // A paused interaction with a chatbox or interface open never
        // advances: close them, unless the script asked to keep dialogues.
        if host.active_script(&bot)?.is_some()
            && host.has_modal_open(&bot)?
            && script.end_dialogue()
        {
            host.interface_close_chatbox(&bot)?;
            host.interface_open_chatbox(&bot, 137)?;
            host.interface_close(&bot)?;
            host.dialogue_close(&bot)?;
        }
        if !host.pulse_manager_has_pulse_running(&bot)? && host.active_script(&bot)?.is_none() {
            if !script.running() {
                return Ok(true);
            }
            // Only 75 bots may run their script each tick.
            let triggered = host.statics().write(|s| {
                let before = s.bot_pulses_triggered_this_tick;
                s.bot_pulses_triggered_this_tick = before.wrapping_add(1);
                before
            });
            if triggered >= 75 {
                return Ok(false);
            }
            if !is_priority && host.roll(10)? && !host.script_is_idler(&script.class()) {
                let pause = host.random_between(20, 50)?;
                let mut state = self.0.write();
                state.random_delay = state.random_delay.wrapping_add(pause);
                return Ok(false);
            }
            host.script_tick(&script)?;
        }
        Ok(false)
    }

    /// `stop()`: the tick counter set, one more `pulse()`, `Pulse.stop()`,
    /// and, while the server runs, the entry under the bot's lower-case user
    /// name removed from `PulseRepository`.
    pub fn stop(&self, host: &mut H) -> Result<()> {
        self.0.write().ticks = i32::MAX - 20;
        self.pulse(host)?;
        host.script_pulse_super_stop(self)?;
        if host.server_running() {
            let bot = self.script().bot().ok_or(Error::Null)?;
            let key = username_key(host, &bot)?;
            host.statics()
                .write(|s| s.pulse_repository.remove(&Some(key)));
        }
        Ok(())
    }
}

/// A `TransitionPulse` object.
pub struct TransitionPulse<H: Host> {
    /// `script`.
    pub script: Script<H>,
    /// The delay `Pulse(RandomFunction.random(60, 200))` drew.
    pub delay: i32,
}
impl<H: Host> Clone for TransitionPulse<H> {
    fn clone(&self) -> Self {
        Self {
            script: self.script.clone(),
            delay: self.delay,
        }
    }
}
impl<H: Host> TransitionPulse<H> {
    /// `TransitionPulse(script)`. Nothing in the pinned sources makes one.
    pub fn new(host: &mut H, script: &Script<H>) -> Result<Self> {
        Ok(Self {
            script: script.clone(),
            delay: host.random_between(60, 200)?,
        })
    }
    /// `pulse()`: a `BotScriptPulse` for a new instance of the script,
    /// entered in `PulseRepository`, then submitted.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        let instance = host.script_new_instance(&self.script)?;
        submit_registered(host, &instance)?;
        Ok(true)
    }
}

/// `GameWorld.Pulser.submit(BotScriptPulse(script).also {
/// AIRepository.PulseRepository[it.botScript.bot.username.lowercase()] = it })`.
fn submit_registered<H: Host>(host: &mut H, script: &Script<H>) -> Result<ScriptPulse<H>> {
    let pulse = ScriptPulse::new(host, script, false)?;
    let bot = script.bot().ok_or(Error::Null)?;
    let key = username_key(host, &bot)?;
    host.statics()
        .write(|s| s.pulse_repository.put(Some(key), pulse.clone()));
    host.pulser_submit(Pulse::Script(pulse.clone()))?;
    Ok(pulse)
}

/// `GeneralBotCreator(botScript, loc)`: a new `AIPlayer` at the location
/// runs the script. The pulse is not entered in `PulseRepository`.
pub fn create_at<H: Host>(
    host: &mut H,
    bot_script: &Script<H>,
    location: Option<Location>,
) -> Result<ScriptPulse<H>> {
    let bot = super::builder::create(host, location)?;
    bot_script.write(|s| s.bot = Some(bot.node()));
    let pulse = ScriptPulse::new(host, bot_script, false)?;
    host.pulser_submit(Pulse::Script(pulse.clone()))?;
    Ok(pulse)
}

/// `GeneralBotCreator(botScript, bot)`: an existing artificial player runs
/// the script, and the pulse is entered in `PulseRepository`.
pub fn create_for_bot<H: Host>(
    host: &mut H,
    bot_script: &Script<H>,
    bot: Option<&AiPlayer<H>>,
) -> Result<ScriptPulse<H>> {
    bot_script.write(|s| s.bot = bot.map(AiPlayer::node));
    submit_registered(host, bot_script)
}

/// `GeneralBotCreator(botScript, player, isPlayer)`: a player runs the
/// script; the pulse is the player's `botting:script` attribute.
pub fn create_for_player<H: Host>(
    host: &mut H,
    bot_script: &Script<H>,
    player: &H::Node,
    is_player: bool,
) -> Result<ScriptPulse<H>> {
    bot_script.write(|s| s.bot = Some(player.clone()));
    let pulse = ScriptPulse::new(host, bot_script, is_player)?;
    host.pulser_submit(Pulse::Script(pulse.clone()))?;
    host.set_attribute_true(player, "/save:not_on_highscores")?;
    host.set_script_attribute(player, &pulse)?;
    Ok(pulse)
}

/// `GBCTick.tick()`: the tick listener that resets the per-tick count.
pub fn gbc_tick<H: Host>(host: &mut H) {
    host.statics()
        .write(|s| s.bot_pulses_triggered_this_tick = 0);
}
