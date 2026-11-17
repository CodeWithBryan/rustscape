//! `NPC.onRegionInactivity` and `Entity.fullRestore` as an NPC reaches them.
use super::{Error, Host, Result, brain};

/// `Entity.fullRestore()`.
pub fn full_restore<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    h.skills_restore(entity)?;
    h.remove_timer(entity, "poison")?;
    h.remove_timer(entity, "poison:immunity")
}

/// `NPC.onRegionInactivity()`: the NPC stops, and an NPC on its way back to
/// spawn is put there at once with the return pulse's arrival body run and
/// the pulse stopped.
pub fn on_region_inactivity<H: Host>(h: &mut H, npc: &H::Entity) -> Result<()> {
    h.walking_reset(npc)?;
    h.pulse_manager_clear(npc)?;
    h.masks_reset(npc)?;
    let brain = brain(h, npc)?;
    let returning = brain.fields.read().returning;
    if returning {
        let here = h.location(npc).ok_or(Error::Null)?;
        h.chunk_remove(npc, here)?;
        let spawn = brain.fields.read().spawn.clone();
        h.set_location(npc, spawn);
        let here = h.location(npc).ok_or(Error::Null)?;
        h.chunk_add(npc, here)?;
        let pulse = brain.fields.read().return_pulse;
        if let Some(pulse) = pulse {
            h.return_arrival(npc)?;
            h.pulse_stop(npc, pulse)?;
        }
    }
    h.remove_renderable(npc)?;
    let here = h.location(npc).ok_or(Error::Null)?;
    let region = h.region_for_id(super::aggression::region_id(here))?;
    if h.region_dynamic(&region) {
        h.npc_clear(npc)?;
    }
    Ok(())
}
