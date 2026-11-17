//! The members a `CombatSwingHandler` subclass outside the three base classes
//! overrides. In the original these are virtual methods: `CombatPulse`, the
//! impact pulse and the base classes call them on the instance and reach the
//! subclass body. Here a host answers, per handler instance, which bodies
//! replace the base ones (`Host::overrides`); the functions in [`super::handler`]
//! and [`super::pulse`] ask before running the body of the instance's base
//! class. Every member that is `None` is the base class's own body.
//!
//! The weapon special attacks (`world::special_attacks`) are the only
//! subclasses that fill this table so far.
use super::Host;
use crate::world::combat_swing::{BattleState, Result, Types};

type E<H> = <H as Types>::Entity;
type R<H> = <H as Host>::HandlerRef;

/// `swing(entity, victim, state)`: the ticks before impact, or `-1`.
pub type Swing<H> =
    fn(&mut H, &R<H>, Option<&E<H>>, Option<&E<H>>, &mut BattleState<H>) -> Result<i32>;
/// `visualize(entity, victim, state)`.
pub type Visualize<H> = fn(&mut H, &R<H>, &E<H>, Option<&E<H>>, &BattleState<H>) -> Result<()>;
/// `visualizeImpact` and `impact(entity, victim, state)`.
pub type StateMember<H> =
    fn(&mut H, &R<H>, Option<&E<H>>, Option<&E<H>>, &mut BattleState<H>) -> Result<()>;
/// `adjustBattleState(entity, victim, state)`.
pub type Adjust<H> = fn(&mut H, &R<H>, &E<H>, &E<H>, &mut BattleState<H>) -> Result<()>;
/// `getFormattedHit(attacker, victim, state, hit)`.
pub type FormattedHit<H> = fn(&mut H, &R<H>, &E<H>, &E<H>, &BattleState<H>, i32) -> Result<i32>;

/// The overriding bodies of one subclass.
pub struct Overrides<H: Host> {
    pub swing: Option<Swing<H>>,
    pub visualize: Option<Visualize<H>>,
    pub visualize_impact: Option<StateMember<H>>,
    pub impact: Option<StateMember<H>>,
    pub adjust_battle_state: Option<Adjust<H>>,
    pub formatted_hit: Option<FormattedHit<H>>,
}

impl<H: Host> Overrides<H> {
    /// A subclass that overrides nothing.
    pub const fn none() -> Self {
        Self {
            swing: None,
            visualize: None,
            visualize_impact: None,
            impact: None,
            adjust_battle_state: None,
            formatted_hit: None,
        }
    }
}

impl<H: Host> Clone for Overrides<H> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H: Host> Copy for Overrides<H> {}
