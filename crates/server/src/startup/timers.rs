//! The engine timers the class scan registers that this port has, and the
//! automatic installation `Entity.init` runs (`TimerRegistry.addAutoTimers`).
//!
//! The original's `ClassScanner.loadTimers` instantiates every non-abstract
//! `RSTimer` subclass the scan found (35 classes, 34 registered) and passes it
//! to `TimerRegistry.registerTimer`; the four with `isAuto` (in registration
//! order `GrandExchangeTimer`, `AntiMacro`, `SkillRestore`, `HolidayRandoms`)
//! are then instantiated and registered on every entity in `Entity.init`.
//! Three of them have a native prototype here, with the members a
//! registration on an NPC runs:
//!
//! - `GrandExchangeTimer` (`RSTimer(500, "GE periodic poll", isSoft, isAuto)`):
//!   no registration hook. Its `run` (players only) polls the Grand Exchange:
//!   a required service, refused.
//! - `AntiMacro` (`PersistTimer(0, "antimacro", isAuto)`): `onRegister`
//!   removes the timer from an entity that is not a player (or is an
//!   artificial player), then, while `runInterval` is 0, `setNextExecution`:
//!   `runInterval = RandomFunction.random(3000, 9001)` (one
//!   `RANDOM.nextInt(6001)`) and `nextExecution = ticks + runInterval`. That
//!   draw is the one shared draw every NPC makes in the original's
//!   `NPCSpawner` phase (`fixtures/startup-draws`). Its player branch (admin
//!   pause) and `run` (random-event content) are refused.
//! - `HolidayRandoms` (`PersistTimer(0, "holiday", isAuto)`): `onRegister`
//!   removes the timer from a non-player, an artificial player or when
//!   `HOLIDAY_EVENT_RANDOMS` is off, and returns. Its player branch is
//!   refused. Its constructor reads the clock four times (`LocalDate.now()`);
//!   clock reads are not modelled (docs/startup.md).
//!
//! `SkillRestore` is not here: its `onRegister` hooks two entity events
//! (`Entity.hook`), and entity event hooks have no native port. An entity
//! therefore gets three of the original's four automatic timers; the start-up
//! reports the missing one as a gap.
use super::generators::WorldGenerators;
use crate::definitions::{item::Shared, item_config::json::Json};
use crate::timers::{
    Context, Error, Handle, Manager, Result, Timer,
    registry::{Factories, Level, Registry},
};

/// A timer with a native prototype.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    GrandExchange,
    AntiMacro,
    HolidayRandoms,
}
/// `AntiMacro.MIN_DELAY_TICKS` and `MAX_DELAY_TICKS`.
pub const ANTI_MACRO_DELAY: (i32, i32) = (3000, 9000);

impl Kind {
    /// The class's simple name, as `registerTimer` logs it.
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::GrandExchange => "GrandExchangeTimer",
            Self::AntiMacro => "AntiMacro",
            Self::HolidayRandoms => "HolidayRandoms",
        }
    }
    fn of(timer: &Timer) -> Option<Self> {
        [Self::GrandExchange, Self::AntiMacro, Self::HolidayRandoms]
            .into_iter()
            .find(|k| k.class_name() == timer.class_name)
    }
    /// A new instance: the constructor's fields (`retrieveInstance`,
    /// `createInstance()` of the class).
    pub fn instance(self) -> Handle {
        let mut timer = match self {
            Self::GrandExchange => Timer::new(
                500,
                "GE periodic poll".into(),
                self.class_name().into(),
                false,
            ),
            Self::AntiMacro => Timer::new(0, "antimacro".into(), self.class_name().into(), true),
            Self::HolidayRandoms => Timer::new(0, "holiday".into(), self.class_name().into(), true),
        };
        timer.is_soft = self == Self::GrandExchange;
        timer.is_auto = true;
        Shared::new(timer)
    }
}

/// The entity a timer is registered on, as far as the three hooks read it.
pub struct Entity<'a> {
    pub player: bool,
    pub artificial: bool,
    /// `GameWorld.getTicks()`.
    pub ticks: i32,
    /// `ServerConstants.HOLIDAY_EVENT_RANDOMS`.
    pub holiday_event_randoms: bool,
    pub generators: &'a mut WorldGenerators,
    /// `registerTimer`'s log lines (`[TimerRegistry] ...`), level and text.
    pub log: &'a mut Vec<(Level, String)>,
}

fn refused(what: &str) -> Error {
    Error::Binding(format!("{what} is not ported"))
}

/// `RandomFunction.random(a, b)`: `min(a, b) + random(|b - a|)`, and
/// `random(n)` is 0 for `n <= 0`, else `RANDOM.nextInt(n)`.
fn random_between(generators: &mut WorldGenerators, a: i32, b: i32) -> i32 {
    let n = b.wrapping_sub(a).wrapping_abs();
    let r = if n <= 0 {
        0
    } else {
        generators.shared_next_int(n).expect("positive bound")
    };
    a.min(b).wrapping_add(r)
}

/// The native timer classes behind `crate::timers`' services, for one entity.
pub struct Natives;
impl Context<Entity<'_>> for Natives {
    fn is_player(&self, state: &Entity<'_>) -> bool {
        state.player
    }
    fn modal_open(&mut self, _: &mut Entity<'_>) -> Result<bool> {
        Err(refused("Player.hasModalOpen for timers"))
    }
    fn script_delay(&mut self, _: &mut Entity<'_>) -> Result<i32> {
        Err(refused("scripts.delay for timers"))
    }
    fn ticks(&mut self, state: &mut Entity<'_>) -> Result<i32> {
        Ok(state.ticks)
    }
    fn entity_name(&mut self, _: &mut Entity<'_>) -> Result<Option<String>> {
        Err(refused("Entity.name for timers"))
    }
    fn before_register(&mut self, _: &Handle, _: &mut Manager, _: &mut Entity<'_>) -> Result<()> {
        Ok(())
    }
    fn on_register(
        &mut self,
        timer: &Handle,
        manager: &mut Manager,
        state: &mut Entity<'_>,
    ) -> Result<()> {
        let kind =
            Kind::of(&timer.read()).ok_or_else(|| refused("onRegister of a non-native timer"))?;
        match kind {
            Kind::GrandExchange => Ok(()),
            Kind::AntiMacro => {
                if !state.player || state.artificial {
                    manager.remove(Some(timer), state, self)?;
                }
                if state.player {
                    // `entity.rights == Rights.ADMINISTRATOR` pauses it.
                    return Err(refused("AntiMacro.onRegister for a player (rights)"));
                }
                if timer.read().run_interval == 0 {
                    let (min, max) = ANTI_MACRO_DELAY;
                    let interval = random_between(state.generators, min, max.wrapping_add(1));
                    let mut t = timer.write();
                    t.run_interval = interval;
                    t.next_execution = state.ticks.wrapping_add(interval);
                }
                Ok(())
            }
            Kind::HolidayRandoms => {
                if !state.player || state.artificial || !state.holiday_event_randoms {
                    manager.remove(Some(timer), state, self)?;
                    return Ok(());
                }
                Err(refused(
                    "HolidayRandoms.onRegister for a player (holiday messages)",
                ))
            }
        }
    }
    fn on_removal(&mut self, _: &Handle, _: &mut Manager, _: &mut Entity<'_>) -> Result<()> {
        // None of the three overrides onRemoval.
        Ok(())
    }
    fn run(&mut self, timer: &Handle, _: &mut Manager, _: &mut Entity<'_>) -> Result<bool> {
        Err(refused(&format!("{}.run", timer.read().class_name)))
    }
    fn initial_delay(
        &mut self,
        timer: &Handle,
        _: &mut Manager,
        _: &mut Entity<'_>,
    ) -> Result<i32> {
        // getInitialRunDelay(): none of the three overrides it.
        Ok(timer.read().run_interval)
    }
    fn save(
        &mut self,
        timer: &Handle,
        _: &mut Json,
        _: &mut Manager,
        _: &mut Entity<'_>,
    ) -> Result<()> {
        Err(refused(&format!("{}.save", timer.read().class_name)))
    }
    fn parse(
        &mut self,
        timer: &Handle,
        _: &Json,
        _: &mut Manager,
        _: &mut Entity<'_>,
    ) -> Result<()> {
        Err(refused(&format!("{}.parse", timer.read().class_name)))
    }
    fn resolve(&mut self, _: &str, _: &mut Manager, _: &mut Entity<'_>) -> Result<Option<Handle>> {
        Err(refused("TimerRegistry lookups outside the start-up"))
    }
    fn run_failure(&mut self, _: &str, _: Option<&str>, _: &mut Entity<'_>) -> Result<()> {
        Err(refused("timer failure logging"))
    }
    fn unknown_timer(&mut self, _: &str, _: &mut Entity<'_>) -> Result<()> {
        Err(refused("unknown timer logging"))
    }
    fn exception(&mut self, error: &Error, _: &mut Entity<'_>) -> Result<()> {
        Err(error.clone())
    }
}
impl Factories<Entity<'_>> for Natives {
    type Argument = ();
    fn instantiate(
        &mut self,
        prototype: &Handle,
        args: Option<&[()]>,
        _: &Registry,
        _: &mut Manager,
        _: &mut Entity<'_>,
    ) -> Result<Option<Handle>> {
        if args.is_some() {
            return Err(refused("timer arguments"));
        }
        let kind =
            Kind::of(&prototype.read()).ok_or_else(|| refused("a non-native timer class"))?;
        Ok(Some(kind.instance()))
    }
    fn registry_log(
        &mut self,
        level: Level,
        message: String,
        _: &Registry,
        _: &mut Manager,
        state: &mut Entity<'_>,
    ) -> Result<()> {
        state.log.push((level, message));
        Ok(())
    }
}
