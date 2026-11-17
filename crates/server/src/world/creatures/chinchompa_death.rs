//! `ChinchompaDeathListener`: a chinchompa killed by a player (not trapped)
//! explodes and may hit the killer standing next to it.
use super::{Host, at, random_between, surrounding_tiles};
use crate::world::combat_pulse::{HitsplatType, impact};
use crate::world::combat_swing::{Graphics, Result};

/// `chinchompaIDs`.
pub const IDS: [i32; 2] = [5079, 5080];
/// The explosion: `Graphics(157, 96)` and sound 158.
pub const EXPLOSION: Graphics = Graphics {
    id: 157,
    height: 96,
    delay: 0,
};
pub const EXPLOSION_SOUND: i32 = 158;

/// `onDeathFinished(self, killer)`: unless the "hunter" attribute marks a
/// trapped chinchompa, a player killer sees the explosion and, on one of the
/// eight surrounding tiles, takes a hit of `random(0, 3)` through ContentAPI
/// `impact` (the killer's own manual hit, then the hurt sound).
pub fn on_death_finished<H: Host>(h: &mut H, npc: &H::Entity, killer: &H::Entity) -> Result<()> {
    if h.has_attribute(npc, "hunter")? || !h.kind(killer).is_player() {
        return Ok(());
    }
    let explosion = at(h, npc)?;
    let tiles = surrounding_tiles(explosion);
    h.send_graphics(EXPLOSION, explosion)?;
    h.play_global_audio(explosion, EXPLOSION_SOUND)?;
    let killer_at = h.location(killer)?;
    if killer_at.is_some_and(|l| tiles.contains(&l)) {
        let amount = random_between(h, 0, 3)?;
        impact::manual_hit(h, killer, Some(killer), amount, Some(HitsplatType::Normal))?;
        if h.kind(killer).is_player() {
            h.play_hurt_audio(killer)?;
        }
    }
    Ok(())
}

/// `onRespawn(self)`: the "hunter" attribute trapping left behind is removed.
pub fn on_respawn<H: Host>(h: &mut H, npc: &H::Entity) -> Result<()> {
    h.remove_attribute(npc, "hunter")
}
