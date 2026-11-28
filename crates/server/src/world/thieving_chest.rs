//! `content.global.skill.thieving.ThievableChestPlugin` with its `Chest`
//! enum and the anonymous pulse of `searchTraps`.
//!
//! Differential corpus: `fixtures/thieving-chests` (oracle/thieving-chests),
//! replayed by `tests/thieving_chests.rs` over these functions with the
//! oracle's declared state. Every outside call is a [`Host`] method.
//!
//! Doors (`PickableDoorHandler`) are `world::thieving_door`.

pub const CLASS: &str = "content.global.skill.thieving.ThievableChestPlugin";
/// The anonymous `Pulse` of `Chest.searchTraps`.
pub const PULSE_CLASS: &str = "content.global.skill.thieving.ThievableChestPlugin$Chest$1";
/// The handler keys `newInstance` puts on every chest's definition.
pub const KEYS: [&str; 2] = ["option:open", "option:search for traps"];
/// The animation of the search and of the opening.
pub const ANIMATION: i32 = 536;
/// The looted chest.
pub const EMPTY: i32 = 2574;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// A ClassCastException.
    Cast,
    /// An IndexOutOfBoundsException.
    Index,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A `Chest` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chest {
    pub name: &'static str,
    pub object_ids: &'static [i32],
    pub level: i32,
    pub experience: f64,
    /// `rewards`: each item's id and amount.
    pub rewards: &'static [(i32, i32)],
    pub respawn: i32,
}

pub const CHESTS: [Chest; 6] = [
    Chest {
        name: "TEN_COIN",
        object_ids: &[2566],
        level: 13,
        experience: 7.8,
        rewards: &[(995, 10)],
        respawn: 7,
    },
    Chest {
        name: "NATURE_RUNE",
        object_ids: &[2567],
        level: 28,
        experience: 25.0,
        rewards: &[(995, 3), (561, 1)],
        respawn: 8,
    },
    Chest {
        name: "FIFTY_COIN",
        object_ids: &[2568],
        level: 43,
        experience: 125.0,
        rewards: &[(995, 50)],
        respawn: 55,
    },
    Chest {
        name: "STEEL_ARROWHEADS",
        object_ids: &[2573],
        level: 47,
        experience: 150.0,
        rewards: &[(41, 5)],
        respawn: 210,
    },
    Chest {
        name: "BLOOD_RUNES",
        object_ids: &[2569],
        level: 59,
        experience: 250.0,
        rewards: &[(995, 500), (565, 2)],
        respawn: 135,
    },
    Chest {
        name: "PALADIN",
        object_ids: &[2570],
        level: 72,
        experience: 500.0,
        rewards: &[(995, 1000), (383, 1), (449, 1), (1623, 1)],
        respawn: 120,
    },
];

/// `Chest.forId(id)`.
pub fn for_id(id: i32) -> Option<usize> {
    CHESTS.iter().position(|c| c.object_ids.contains(&id))
}

/// Every constant's `currentRespawn`: the enum's own state, one value for
/// every chest of a constant in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Respawns(pub [i32; 6]);

/// What the plugin and the pulse ask of the world, for their player and
/// the scenery of the option.
pub trait Host {
    /// The constants' `currentRespawn`.
    fn respawns(&mut self) -> &mut Respawns;
    /// `GameWorld.getTicks()`.
    fn world_ticks(&mut self) -> Result<i32>;
    /// `player.sendMessage(message)` (and the packet dispatch's).
    fn message(&mut self, message: &str) -> Result<()>;
    /// `player.sendMessage(message, ticks)`.
    fn message_later(&mut self, message: &str, ticks: i32) -> Result<()>;
    /// `player.lock(ticks)`.
    fn lock(&mut self, ticks: i32) -> Result<()>;
    /// `player.getSkills().getLifepoints()`.
    fn lifepoints(&mut self) -> Result<i32>;
    /// `player.getImpactHandler().manualHit(player, hit, NORMAL)`.
    fn manual_hit(&mut self, hit: i32) -> Result<()>;
    /// `player.faceLocation(object.getLocation())`.
    fn face_chest(&mut self) -> Result<()>;
    /// `player.getSkills().getLevel(Skills.THIEVING)`.
    fn level(&mut self) -> Result<i32>;
    /// ContentAPI `animate(player, id, false)`: `animator.animate`.
    fn animator_animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(Animation.create(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.getInventory().freeSlots()`.
    fn free_slots(&mut self) -> Result<i32>;
    /// `player.getImpactHandler().setDisabledTicks(ticks)`.
    fn set_disabled_ticks(&mut self, ticks: i32) -> Result<()>;
    /// `GameWorld.getPulser().submit(pulse)`.
    fn submit(&mut self, pulse: TrapPulse) -> Result<()>;
    /// `player.getInventory().add(item, player)`: what does not fit is
    /// dropped for the player.
    fn add_or_drop(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `player.getSkills().addExperience(Skills.THIEVING, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `object.isActive()`.
    fn node_active(&mut self) -> Result<bool>;
    /// `SceneryBuilder.replace(object, object.transform(id), ticks)`.
    fn replace(&mut self, id: i32, ticks: i32) -> Result<()>;
}

/// `Chest.isRespawning()`.
fn is_respawning<H: Host + ?Sized>(h: &mut H, chest: usize) -> Result<bool> {
    let current = h.respawns().0[chest];
    Ok(current > h.world_ticks()?)
}

/// `Chest.getHitAmount(player)`.
fn hit_amount<H: Host + ?Sized>(h: &mut H) -> Result<i32> {
    Ok((h.lifepoints()? / 12).max(2))
}

/// `ThievableChestPlugin.handle(player, node, option)` for a scenery of id
/// `node`: its answer.
pub fn handle<H: Host + ?Sized>(h: &mut H, node: i32, option: &str) -> Result<bool> {
    let chest = for_id(node);
    match option {
        "open" => {
            if let Some(chest) = chest {
                open(h, chest)?;
            }
        }
        // `chest.searchTraps` of a null chest.
        "search for traps" => search_traps(h, chest.ok_or(Error::Null)?)?,
        _ => {}
    }
    Ok(true)
}

/// `Chest.open(player, object)`.
fn open<H: Host + ?Sized>(h: &mut H, chest: usize) -> Result<()> {
    if is_respawning(h, chest)? {
        return h.message("It looks like this chest has already been looted.");
    }
    h.lock(2)?;
    h.message("You have activated a trap on the chest.")?;
    let hit = hit_amount(h)?;
    h.manual_hit(hit)
}

/// `Chest.searchTraps(player, object)`.
fn search_traps<H: Host + ?Sized>(h: &mut H, chest: usize) -> Result<()> {
    h.face_chest()?;
    if is_respawning(h, chest)? {
        return h.message("It looks like this chest has already been looted.");
    }
    if h.level()? < CHESTS[chest].level {
        h.animator_animate(ANIMATION)?;
        h.lock(2)?;
        h.message("You search the chest for traps.")?;
        return h.message_later("You find nothing.", 1);
    }
    if h.free_slots()? == 0 {
        return h.message("Not enough inventory space.");
    }
    h.lock(6)?;
    h.animator_animate(ANIMATION)?;
    h.message("You find a trap on the chest...")?;
    h.set_disabled_ticks(6)?;
    h.submit(TrapPulse { chest, counter: 0 })
}

/// The anonymous `Pulse(1, player)` of `searchTraps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapPulse {
    /// The constant's ordinal.
    pub chest: usize,
    pub counter: i32,
}

impl TrapPulse {
    /// `pulse()`: whether the pulse is done.
    pub fn pulse<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        let chest = CHESTS.get(self.chest).ok_or(Error::Index)?;
        self.counter = self.counter.wrapping_add(1);
        match self.counter {
            2 => h.message("You disable the trap.")?,
            4 => {
                h.animate(ANIMATION)?;
                h.face_chest()?;
                h.message("You open the chest.")?;
            }
            6 => {
                for (id, amount) in chest.rewards {
                    h.add_or_drop(*id, *amount)?;
                }
                h.message("You find treasure inside!")?;
                h.add_experience(chest.experience)?;
                if h.node_active()? {
                    h.replace(EMPTY, 3)?;
                }
                // `setRespawn()`.
                let ticks = h.world_ticks()?;
                let wait = (f64::from(chest.respawn) / 0.6) as i32;
                h.respawns().0[self.chest] = ticks.wrapping_add(wait);
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_pinned_enum() {
        assert_eq!(for_id(2573), Some(3));
        assert_eq!(for_id(2574), None);
        assert_eq!(CHESTS[5].rewards.len(), 4);
        assert_eq!((f64::from(CHESTS[0].respawn) / 0.6) as i32, 11);
    }
}
