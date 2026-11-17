//! The consumables' timer services answered from the native engine timers
//! (`timers::content`, docs/content-timers.md): a world whose players run a
//! `timers::Manager` with the registry of `content::register_prototypes`
//! implements the timer members of [`super::Host`] with these functions.
//!
//! - `spawnTimer(identifier, *args)` is `TimerRegistry.getTimerInstance`:
//!   the registry's prototype for the identifier, instantiated with the
//!   arguments (`None` for an identifier the registry does not hold);
//! - `registerTimer`, `removeTimer(identifier)`, `getTimer<Disease>` and
//!   `removeTimer<Disease>` are the manager's own members;
//! - `Disease.hitsLeft` is the timer's `State::Disease` field.
use super::TimerArg;
use crate::timers::{
    self, Handle, Manager,
    content::{self, Arg, Kind, Natives, State},
};

/// The boxed `Int`/`Boolean` arguments as the timer constructors receive them.
pub fn arguments<E>(args: &[TimerArg]) -> Vec<Arg<E>> {
    args.iter()
        .map(|a| match a {
            TimerArg::Int(v) => Arg::Int(*v),
            TimerArg::Bool(v) => Arg::Bool(*v),
        })
        .collect()
}

/// `spawnTimer(identifier, *args)`.
pub fn spawn_timer<H: content::Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    identifier: &str,
    args: &[TimerArg],
) -> timers::Result<Option<Handle>> {
    let registry = ctx.registry.clone();
    let args = arguments::<H::Entity>(args);
    registry.instance(identifier, &args, manager, &mut (), ctx)
}

/// `registerTimer(entity, timer)`.
pub fn register_timer<H: content::Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    timer: Option<&Handle>,
) -> timers::Result<()> {
    manager.register_optional(timer, &mut (), ctx)
}

/// `removeTimer(entity, identifier)`.
pub fn remove_timer<H: content::Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    identifier: &str,
) -> timers::Result<()> {
    manager.remove_identifier(identifier, &mut (), ctx)
}

/// `getTimer<Disease>(entity)`.
pub fn disease(manager: &Manager) -> timers::Result<Option<Handle>> {
    content::get(manager, Kind::Disease)
}

/// `removeTimer<Disease>(entity)`.
pub fn remove_disease<H: content::Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
) -> timers::Result<()> {
    manager.remove_where(|t| Kind::Disease.matches(t), &mut (), ctx)
}

/// `Disease.hitsLeft` (the field's initialiser on first use).
pub fn disease_hits_left<H: content::Host>(timer: &Handle) -> timers::Result<i32> {
    content::with_state::<H::Entity, H::RandomEvent, _>(timer, |s| match s {
        State::Disease { hits_left } => Ok(*hits_left),
        _ => Err(timers::Error::Type),
    })?
}

/// `Disease.hitsLeft = value`.
pub fn set_disease_hits_left<H: content::Host>(timer: &Handle, value: i32) -> timers::Result<()> {
    content::with_state::<H::Entity, H::RandomEvent, _>(timer, |s| match s {
        State::Disease { hits_left } => {
            *hits_left = value;
            Ok(())
        }
        _ => Err(timers::Error::Type),
    })?
}
