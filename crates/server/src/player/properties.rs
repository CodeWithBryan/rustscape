//! core.game.node.entity.impl.Properties: the fields an entity's Properties
//! object holds, their constructor defaults and the accessors that only read
//! or write those fields.
//!
//! Not ported here: `combatPulse` (a CombatPulse built from the entity in the
//! constructor; world/combat_pulse is a library not bound to canonical
//! actors), `updateDefenceAnimation` (item/NPC configuration lookups) and
//! `getCurrentCombatLevel`'s world-settings, skull and familiar reads, which
//! [`Properties::current_combat_level`] takes as explicit inputs. The
//! autocast spell is not a field here: the canonical player keeps it once, in
//! `persisted::spells::State`, which the save file reads and writes.
//!
//! The original's `entity` field is the owning entity. Natively a Properties
//! value lives inside the canonical player's fields and stores no node
//! handle, so it adds no ownership edge of its own.
use crate::world::{
    animation::{Animation, Graphics, Priority},
    combat_swing::{ArmourSet, AttackStyle, CombatStyle},
    doors::Location,
};

/// `new int[15]`.
pub const BONUSES: usize = 15;

pub struct Properties {
    pub retaliating: bool,
    pub teleporting: bool,
    pub combat_level: i32,
    pub attack_style: Option<AttackStyle>,
    pub teleport_location: Option<Location>,
    pub spawn_location: Option<Location>,
    /// The array object is replaceable through `setBonuses`, as in the original.
    pub bonuses: Vec<i32>,
    pub attack_speed: i32,
    pub last_animation_end: i64,
    pub attack_animation: Option<Animation>,
    pub defence_animation: Option<Animation>,
    pub death_animation: Option<Animation>,
    pub death_gfx: Option<Graphics>,
    pub range_animation: Option<Animation>,
    pub magic_animation: Option<Animation>,
    /// `spell`: a CombatSpell's book ordinal and id. Spells are not ported;
    /// `setSpell`'s `combatPulse.updateStyle()` is the caller's service.
    pub spell: Option<(i32, i32)>,
    pub armour_set: Option<ArmourSet>,
    pub multi_zone: bool,
    pub safe_zone: bool,
    pub safe_respawn: Option<Location>,
    pub combat_time_out: i32,
    pub npc_walkable: bool,
    pub protect_style: Option<CombatStyle>,
}
impl Default for Properties {
    /// The field initialisers of `new Properties(entity)`, in declaration order.
    fn default() -> Self {
        Self {
            retaliating: true,
            teleporting: false,
            combat_level: 0,
            attack_style: None,
            teleport_location: None,
            spawn_location: None,
            bonuses: vec![0; BONUSES],
            attack_speed: 4,
            last_animation_end: 0,
            attack_animation: Some(Animation::with(422, 0, Some(Priority::High))),
            defence_animation: Some(Animation::new(404)),
            death_animation: Some(Animation::with(9055, 0, Some(Priority::High))),
            death_gfx: Some(Graphics::new(-1, 0, 0)),
            range_animation: None,
            magic_animation: None,
            spell: None,
            armour_set: None,
            multi_zone: false,
            safe_zone: false,
            safe_respawn: None,
            combat_time_out: 10,
            npc_walkable: false,
            protect_style: None,
        }
    }
}
/// What `getCurrentCombatLevel` reads besides the stored combat level, for a player.
#[derive(Clone, Copy, Debug)]
pub struct CombatLevelInputs {
    /// `GameWorld.getSettings().isPvp()`.
    pub pvp_world: bool,
    /// `GameWorld.getSettings().getWild_pvp_enabled()`.
    pub wild_pvp: bool,
    /// `player.getSkullManager().isWilderness()`, read only when `wild_pvp` is true.
    pub in_wilderness: bool,
    /// `player.getFamiliarManager().isUsingSummoning()`.
    pub using_summoning: bool,
    /// `player.getFamiliarManager().getSummoningCombatLevel()`.
    pub summoning_combat_level: i32,
}
impl Properties {
    /// `getCombatAnimation(index)`: 0 attack, 1 magic, 2 range, 3 defence, anything else death.
    pub fn combat_animation(&self, index: i32) -> Option<Animation> {
        match index {
            0 => self.attack_animation.clone(),
            1 => self.magic_animation.clone(),
            2 => self.range_animation.clone(),
            3 => self.defence_animation.clone(),
            _ => self.death_animation.clone(),
        }
    }
    /// `getCurrentCombatLevel` for a player. Integer addition wraps as in Java.
    pub fn current_combat_level(&self, inputs: CombatLevelInputs) -> i32 {
        if (inputs.pvp_world || (inputs.wild_pvp && inputs.in_wilderness))
            && !inputs.using_summoning
        {
            self.combat_level
        } else {
            self.combat_level
                .wrapping_add(inputs.summoning_combat_level)
        }
    }
    /// `setSpell`. Returns whether the original would now call
    /// `combatPulse.updateStyle()`, which is not ported.
    pub fn set_spell(&mut self, spell: Option<(i32, i32)>) -> bool {
        self.spell = spell;
        spell.is_some()
    }
}
