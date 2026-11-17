//! Combat in the live world (work in progress: the timers' hit and NPC
//! poison immunity answers).
use super::{P, host::Live};
use crate::world::combat_pulse::HitsplatType;

impl Live {
    /// `owner.getImpactHandler().manualHit(source, hit, type)` from a timer.
    pub fn timer_manual_hit(
        &mut self,
        _owner: &P,
        _source: &P,
        _hit: i32,
        _kind: HitsplatType,
    ) -> Result<(), String> {
        Err(
            "not ported in the live world: ImpactHandler.manualHit (combat is not installed)"
                .into(),
        )
    }
    /// `NPC.isPoisonImmune()`.
    pub fn npc_poison_immune(&mut self, _npc: &P) -> Result<bool, String> {
        Err("not ported in the live world: NPC.isPoisonImmune (combat is not installed)".into())
    }
}
