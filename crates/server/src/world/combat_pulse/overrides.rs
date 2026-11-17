//! The members a `CombatSwingHandler` subclass outside the three base classes
//! overrides. In the original these are virtual methods: `CombatPulse`, the
//! impact pulse and the base classes call them on the instance and reach the
//! subclass body. Here a host answers, per handler instance, which bodies
//! replace the base ones (`Host::overrides`); the functions in [`super::handler`]
//! and [`super::pulse`] ask before running the body of the instance's base
//! class. Every member that is `None` is the base class's own body.
//!
//! The weapon special attacks (`world::special_attacks`) and the dragonfire
//! and switching handlers (`world::dragonfire`) fill this table.
use super::Host;
use crate::world::combat_swing::{ArmourSet, BattleState, InteractionType, Result, Types};

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
/// `canSwing(entity, victim)`.
pub type CanSwing<H> = fn(&mut H, &R<H>, &E<H>, &E<H>) -> Result<InteractionType>;
/// `addExperience(entity, victim, state)`.
pub type AddExperience<H> =
    fn(&mut H, &R<H>, Option<&E<H>>, Option<&E<H>>, Option<&BattleState<H>>) -> Result<()>;
/// `postSwing(entity, victim, state)`.
pub type PostSwing<H> =
    fn(&mut H, &R<H>, Option<&E<H>>, Option<&E<H>>, &BattleState<H>) -> Result<()>;
/// `getArmourSet(e)`.
pub type ArmourSetOf<H> = fn(&mut H, &R<H>, Option<&E<H>>) -> Result<Option<ArmourSet>>;
/// `calculateAccuracy(entity)`.
pub type Accuracy<H> = fn(&mut H, &R<H>, Option<&E<H>>) -> Result<i32>;
/// `calculateHit(entity, victim, modifier)`.
pub type Hit<H> = fn(&mut H, &R<H>, Option<&E<H>>, Option<&E<H>>, f64) -> Result<i32>;
/// `calculateDefence(victim, attacker)`.
pub type Defence<H> = fn(&mut H, &R<H>, Option<&E<H>>, Option<&E<H>>) -> Result<i32>;
/// `getSetMultiplier(e, skillId)`.
pub type SetMultiplier<H> = fn(&mut H, &R<H>, Option<&E<H>>, i32) -> Result<f64>;

/// The overriding bodies of one subclass.
pub struct Overrides<H: Host> {
    pub swing: Option<Swing<H>>,
    pub visualize: Option<Visualize<H>>,
    pub visualize_impact: Option<StateMember<H>>,
    pub impact: Option<StateMember<H>>,
    pub adjust_battle_state: Option<Adjust<H>>,
    pub formatted_hit: Option<FormattedHit<H>>,
    pub can_swing: Option<CanSwing<H>>,
    pub add_experience: Option<AddExperience<H>>,
    pub post_swing: Option<PostSwing<H>>,
    pub armour_set: Option<ArmourSetOf<H>>,
    pub calculate_accuracy: Option<Accuracy<H>>,
    pub calculate_hit: Option<Hit<H>>,
    pub calculate_defence: Option<Defence<H>>,
    pub set_multiplier: Option<SetMultiplier<H>>,
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
            can_swing: None,
            add_experience: None,
            post_swing: None,
            armour_set: None,
            calculate_accuracy: None,
            calculate_hit: None,
            calculate_defence: None,
            set_multiplier: None,
        }
    }
}

impl<H: Host> Clone for Overrides<H> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H: Host> Copy for Overrides<H> {}
