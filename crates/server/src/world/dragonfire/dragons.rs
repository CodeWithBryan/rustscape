//! content/global/handlers/npc: `ChromaticDragonNPC`, `MetalDragonNPC` and
//! `ChromaticDragonBehavior`.
//!
//! The two classes are AbstractNPC subclasses ([`DragonClass`] implements
//! `npc_classes::Abstract`, so `npc_classes::registry` registers and
//! constructs them). Each class has one static dragonfire attack, made the
//! first time the class is used ([`dragonfire`]); each instance has its own
//! `MultiSwingHandler` over two melee attacks and that attack
//! ([`combat_action`]), which its `getSwingHandler` answers. Both answer
//! `getDragonfireProtection` with every token ([`PROTECTION`]).
use super::{Host, ItemValue, ProjectileTemplate, SwitchAttack, gfx, handler, multi};
use crate::world::combat_swing::{CombatStyle, Result};
use crate::world::npc_classes::{self as nc, Abstract, Constructor};
use crate::world::special_attacks::Priority;

/// One of the dragon AbstractNPC subclasses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DragonClass {
    Chromatic,
    Metal,
}

/// `getIds()` of each class.
pub mod ids {
    /// Green, blue, red and black dragons and the lava dragon.
    pub const CHROMATIC: [i32; 21] = [
        941, 4677, 4678, 4679, 4680, 55, 4681, 4682, 4683, 4684, 53, 4669, 4670, 4671, 4672, 54,
        4673, 4674, 4675, 4676, 8629,
    ];
    /// Bronze, iron, steel and the house's steel dragon.
    pub const METAL: [i32; 4] = [1590, 1591, 1592, 3590];
}

impl DragonClass {
    pub const ALL: [Self; 2] = [Self::Chromatic, Self::Metal];
}

impl Abstract for DragonClass {
    fn name(&self) -> &'static str {
        match self {
            Self::Chromatic => "content.global.handlers.npc.ChromaticDragonNPC",
            Self::Metal => "content.global.handlers.npc.MetalDragonNPC",
        }
    }
    fn ids(&self) -> Vec<i32> {
        match self {
            Self::Chromatic => ids::CHROMATIC.to_vec(),
            Self::Metal => ids::METAL.to_vec(),
        }
    }
    /// `ChromaticDragonNPC()` is `super(941, null)`, `MetalDragonNPC()`
    /// `super(1590, null)`; `AbstractNPC(id, location)` walks.
    fn prototype(&self) -> (i32, Constructor) {
        let id = match self {
            Self::Chromatic => 941,
            Self::Metal => 1590,
        };
        (
            id,
            Constructor {
                walks: true,
                aggressive: None,
            },
        )
    }
    /// `construct(id, location, objects)`: `new XDragonNPC(id, location)`.
    fn construct(&self, _location: Option<[i32; 3]>) -> nc::Result<Option<Constructor>> {
        Ok(Some(Constructor {
            walks: true,
            aggressive: None,
        }))
    }
}

/// `getDragonfireProtection(fire)` of both classes: `0x2 | 0x4 | 0x8`.
pub const PROTECTION: i32 = 0x2 | 0x4 | 0x8;

/// The class's static `DRAGONFIRE`: `DragonfireSwingHandler.get` with
/// animation 81 (HIGH) and maximum 52; chromatic dragons breathe in melee
/// range with graphic (1, 64), metal dragons from afar with projectile 54.
pub fn dragonfire<H: Host>(h: &mut H, class: DragonClass) -> SwitchAttack<H> {
    let animation = h.new_animation(81, Priority::High);
    match class {
        DragonClass::Chromatic => handler::get(
            h,
            true,
            52,
            Some(animation),
            Some(gfx(1, 64)),
            None,
            None,
            true,
        ),
        DragonClass::Metal => {
            let projectile = ProjectileTemplate::create(None, None, 54, 40, 36, 41, 46, 20, 255);
            handler::get(
                h,
                false,
                52,
                Some(animation),
                None,
                None,
                Some(projectile),
                true,
            )
        }
    }
}

/// An instance's `combatAction`: two melee attacks (animations 80 and 91)
/// and the class's dragonfire; melee distance only for metal dragons.
pub fn combat_action<H: Host>(
    h: &mut H,
    class: DragonClass,
    dragonfire: &SwitchAttack<H>,
) -> Result<H::HandlerRef> {
    let melee = h.style_handler(CombatStyle::Melee);
    let mut attacks = vec![];
    for id in [80, 91] {
        let animation = h.new_animation(id, Priority::High);
        attacks.push(SwitchAttack::new(
            Some(melee.clone()),
            Some(animation),
            None,
            None,
            None,
        ));
    }
    attacks.push(dragonfire.clone());
    multi::new(h, class == DragonClass::Metal, attacks)
}

/// `ChromaticDragonBehavior`'s dragon ids and its mutable `EGG_RATE` (1000).
pub mod behaviour {
    pub const GREEN: [i32; 5] = [941, 4677, 4678, 4679, 4680];
    pub const BLUE: [i32; 5] = [55, 4681, 4682, 4683, 4684];
    pub const RED: [i32; 5] = [53, 4669, 4670, 4671, 4672];
    pub const BLACK: [i32; 5] = [54, 4673, 4674, 4675, 4676];
    /// The initial `EGG_RATE`.
    pub const EGG_RATE: i32 = 1000;
    pub const BLACK_EGG: i32 = 12480;
    pub const RED_EGG: i32 = 12477;
    pub const BLUE_EGG: i32 = 12478;
    pub const GREEN_EGG: i32 = 12479;
    pub const DRAGON_BONES: i32 = 536;

    /// `NPCBehavior(*ids)`: the ids the behaviour is registered for.
    pub fn ids() -> Vec<i32> {
        [GREEN, BLUE, RED, BLACK].concat()
    }
}

/// `Skills.SUMMONING`.
const SUMMONING: i32 = 23;

/// `ChromaticDragonBehavior.onDropTableRolled(self, killer, drops)`: every
/// dragon egg is removed, then a killer with static summoning 99 who rolls
/// `EGG_RATE` gets the dragon's egg (dragon bones for an id of none of the
/// four colours). `egg_rate` is the current `EGG_RATE`.
pub fn on_drop_table_rolled<H: Host>(
    h: &mut H,
    npc: &H::Entity,
    killer: &H::Entity,
    drops: &mut Vec<ItemValue>,
    egg_rate: i32,
) -> Result<()> {
    use behaviour::*;
    drops.retain(|(id, _, _)| ![BLACK_EGG, RED_EGG, BLUE_EGG, GREEN_EGG].contains(id));
    if h.static_level(killer, SUMMONING)? >= 99 && roll(h, egg_rate)? {
        let id = h.id(npc)?;
        let egg = if GREEN.contains(&id) {
            GREEN_EGG
        } else if BLUE.contains(&id) {
            BLUE_EGG
        } else if RED.contains(&id) {
            RED_EGG
        } else if BLACK.contains(&id) {
            BLACK_EGG
        } else {
            DRAGON_BONES
        };
        drops.push((egg, 1, 1000));
    }
    Ok(())
}

/// `RandomFunction.roll(chance)`.
fn roll<H: Host>(h: &mut H, chance: i32) -> Result<bool> {
    if chance <= 1 {
        return Ok(true);
    }
    Ok(super::random(h, chance)? == 1)
}
