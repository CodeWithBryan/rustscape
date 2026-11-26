//! The combat chase: `CombatPulse`'s private `MovementPulse`, the pulse an
//! entity's combat uses to get in range of its victim.
//!
//! It is the native `MovementPulse` (`world::movement`) in the configuration
//! `CombatPulse` gives it, and these functions are `CombatPulse`'s own
//! statements:
//!
//! - [`new`]: `movement = object : MovementPulse(entity, null) { override fun
//!   pulse() = false }` in the combat pulse's initialiser. The destination is
//!   null when it is made, so the constructor sets no destination flag: the
//!   chase never asks `DestinationFlag.ENTITY` for a tile. Like every
//!   `MovementPulse` constructor it takes over `mover.currentMovement`
//!   (stopping the previous one and resetting the walking queue), which is
//!   why it must be made when the entity's combat pulse is made.
//! - [`set_victim`]: `movement.setLast(null)` and
//!   `movement.setDestination(victim)` of `CombatPulse.setVictim`.
//! - [`update_path`]: `movement.updatePath()` of the pulse's interaction
//!   check. With no flag, the path is found to the victim node itself
//!   (`Pathfinder.find(mover, destination, true, pathfinder)`), from a border
//!   tile when the mover stands inside the victim, truncated beyond the
//!   pathfinding distance, skipped while the victim fights and walks toward
//!   the mover, and led along a walking NPC's path when the destination is
//!   not the mover's own combat victim.
//! - [`pulse`]: the anonymous subclass's `pulse()`. `CombatPulse` never
//!   submits the movement pulse, so only `updatePath` runs.
//!
//! Differentially tested by attack-chase-oracle over the movement oracle's
//! harness (docs/attack-chase.md).
use super::movement::{Constructor, Context, Movement, Node, Result};

/// The combat pulse's `MovementPulse(entity, null)`.
pub fn new(ctx: &mut impl Context, id: usize, entity: Option<Node>) -> Result<Movement> {
    Movement::new(ctx, id, entity, None, None, false, Constructor::Basic)
}

/// `movement.setLast(null)`.
pub fn set_last(movement: &Movement) {
    movement.state.write().last = None;
}

/// `movement.setDestination(victim)`.
pub fn set_destination(movement: &Movement, victim: Option<Node>) {
    movement.state.write().destination = victim;
}

/// `CombatPulse.setVictim`'s two movement statements, in its order.
pub fn set_victim(movement: &Movement, victim: Option<Node>) {
    set_last(movement);
    set_destination(movement, victim);
}

/// `movement.updatePath()`.
pub fn update_path(movement: &mut Movement, ctx: &mut impl Context) -> Result<()> {
    movement.update_path(ctx)
}

/// The anonymous subclass's `pulse()`.
pub const fn pulse() -> bool {
    false
}
