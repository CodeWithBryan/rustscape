//! `content.global.skill.thieving.PickableDoorHandler` with its
//! `PickableDoor` class.
//!
//! Differential corpus: `fixtures/thieving-doors` (oracle/thieving-doors),
//! replayed by `tests/thieving_doors.rs` over these functions with the
//! oracle's declared state. Every outside call is a [`Host`] method.
use rs09_game_core::direction::Direction;

pub const CLASS: &str = "content.global.skill.thieving.PickableDoorHandler";
/// The handler keys `newInstance` puts on every door id's definition, in
/// its order.
pub const KEYS: [&str; 2] = ["option:pick-lock", "option:open"];
/// `DOORS`: the scenery ids the handler is put on.
pub const DOORS: [i32; 28] = [
    42028, 2550, 2551, 2554, 2555, 2556, 2557, 2558, 2559, 5501, 7246, 9565, 13314, 13317, 13320,
    13323, 13326, 13344, 13345, 13346, 13347, 13348, 13349, 15759, 34005, 34805, 34806, 34812,
];
/// `LOCK_PICK`.
pub const LOCK_PICK: i32 = 1523;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// A ClassCastException.
    Cast,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A `PickableDoor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickableDoor {
    /// `locations`: every door of the pinned list has one.
    pub location: [i32; 3],
    pub level: i32,
    pub experience: f64,
    /// Whether the lock needs a lockpick.
    pub lockpick: bool,
    /// Whether the locked side is the other one.
    pub flipped: bool,
}

const fn door(location: [i32; 3], level: i32, experience: f64) -> PickableDoor {
    PickableDoor {
        location,
        level,
        experience,
        lockpick: false,
        flipped: false,
    }
}
const fn picked(location: [i32; 3], level: i32, experience: f64, flipped: bool) -> PickableDoor {
    PickableDoor {
        location,
        level,
        experience,
        lockpick: true,
        flipped,
    }
}

/// `pickableDoors`, in the order `newInstance` adds them.
pub const PICKABLE: [PickableDoor; 12] = [
    door([2672, 3308, 0], 1, 3.8),
    door([2672, 3301, 0], 14, 15.0),
    door([2610, 3316, 0], 15, 15.0),
    picked([3190, 3957, 0], 32, 25.0, false),
    door([2565, 3356, 0], 46, 37.5),
    door([2579, 3286, 1], 61, 50.0),
    door([2579, 3307, 1], 61, 50.0),
    door([3018, 3187, 0], 1, 0.0),
    picked([2601, 9482, 0], 82, 0.0, false),
    picked([3044, 3956, 0], 39, 35.0, true),
    picked([3041, 3959, 0], 39, 35.0, true),
    picked([3038, 3956, 0], 39, 35.0, true),
];

/// `forDoor(location)`.
pub fn for_door(location: [i32; 3]) -> Option<usize> {
    PICKABLE.iter().position(|d| d.location == location)
}

/// What the handler asks of the world, for its player and the scenery of
/// the option.
pub trait Host {
    /// `node.getLocation()`.
    fn object_location(&mut self) -> Result<[i32; 3]>;
    /// `object.getDirection()`.
    fn object_direction(&mut self) -> Result<Direction>;
    /// `player.getLocation()`.
    fn player_location(&mut self) -> Result<[i32; 3]>;
    /// `player.getPacketDispatch().sendMessage(message)` (and
    /// `player.sendMessage(message)`, which is that call).
    fn message(&mut self, message: &str) -> Result<()>;
    /// ContentAPI `sendMessage(player, message)`.
    fn send_message(&mut self, message: &str) -> Result<()>;
    /// ContentAPI `handleAutowalkDoor(player, object)`; its answer is not
    /// read.
    fn handle_autowalk_door(&mut self) -> Result<()>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `player.getSkills().getLevel(Skills.THIEVING)`.
    fn level(&mut self) -> Result<i32>;
    /// `player.getImpactHandler().manualHit(player, hit, NORMAL)`.
    fn manual_hit(&mut self, hit: i32) -> Result<()>;
    /// `player.getInventory().containsItem(LOCK_PICK)`.
    fn has_lock_pick(&mut self) -> Result<bool>;
    /// `player.getSkills().addExperience(Skills.THIEVING, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `object.getDefinition().hasAction(action)`.
    fn has_action(&mut self, action: &str) -> Result<bool>;
    /// `DoorActionHandler.getDestination(entity, object)`.
    fn door_destination(&mut self) -> Result<[i32; 3]>;
}

/// `PickableDoorHandler.handle(player, node, option)`: its answer.
pub fn handle<H: Host + ?Sized>(h: &mut H, option: &str) -> Result<bool> {
    let location = h.object_location()?;
    let door = for_door(location);
    match option {
        "open" => {
            match door {
                None => h.message("The door is locked.")?,
                Some(door) => open(h, &PICKABLE[door])?,
            }
            Ok(true)
        }
        "pick-lock" => {
            match door {
                None => h.send_message("This door cannot be unlocked.")?,
                Some(door) => pick_lock(h, &PICKABLE[door])?,
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// `PickableDoorHandler.getDestination(node, n)` for a scenery `n`.
pub fn destination<H: Host + ?Sized>(h: &mut H) -> Result<Option<[i32; 3]>> {
    if h.has_action("pick-lock")? {
        return h.door_destination().map(Some);
    }
    Ok(None)
}

/// `PickableDoor.isInside(player, object)`.
fn is_inside<H: Host + ?Sized>(h: &mut H) -> Result<bool> {
    let player = h.player_location()?;
    let object = h.object_location()?;
    // Direction.getLogicalDirection(player.getLocation(), object.getLocation()).
    let toward = Direction::logical_delta(
        object[0].wrapping_sub(player[0]),
        object[1].wrapping_sub(player[1]),
    );
    let facing = h.object_direction()?;
    Ok(matches!(
        (facing, toward),
        (Direction::South, Direction::West)
            | (Direction::East, Direction::South)
            | (Direction::North, Direction::East)
    ))
}

/// `PickableDoor.open(player, object)`.
fn open<H: Host + ?Sized>(h: &mut H, door: &PickableDoor) -> Result<()> {
    if is_inside(h)? != door.flipped {
        h.handle_autowalk_door()?;
        h.message("You go through the door.")
    } else {
        h.message("The door is locked.")
    }
}

/// `PickableDoor.pickLock(player, object)`.
fn pick_lock<H: Host + ?Sized>(h: &mut H, door: &PickableDoor) -> Result<()> {
    let success = h.random(12)? >= 4;
    if is_inside(h)? != door.flipped {
        return h.message("The door is already unlocked.");
    }
    if h.level()? < door.level {
        h.message("You attempt to pick the lock.")?;
        let hit = h.random(10)? < 5;
        // RandomFunction.random(1, 3).
        let amount = 1i32.wrapping_add(h.random(2)?);
        h.manual_hit(amount)?;
        return h.message(if hit {
            "You have activated a trap on the lock."
        } else {
            "You fail to pick the lock."
        });
    }
    if door.lockpick && !h.has_lock_pick()? {
        return h.message("You need a lockpick in order to pick this lock.");
    }
    if success {
        h.add_experience(door.experience)?;
        h.handle_autowalk_door()?;
    }
    h.message("You attempt to pick the lock.")?;
    h.message(if success {
        "You manage to pick the lock."
    } else {
        "You fail to pick the lock."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_pinned_list() {
        assert_eq!(for_door([2672, 3308, 0]), Some(0));
        assert_eq!(for_door([2672, 3308, 1]), None);
        assert_eq!(PICKABLE.iter().filter(|d| d.lockpick).count(), 5);
        assert_eq!(PICKABLE.iter().filter(|d| d.flipped).count(), 3);
        assert_eq!(DOORS.len(), 28);
    }
}
