//! `NPCDepositListener` (Peer the Seer's "deposit") and `DummyAttackPlugin`
//! (`content/global/handlers/npc`).
use super::{Expression, Host, Result};

/// `NPCs.PEER_THE_SEER_1288`.
pub const PEER_THE_SEER: i32 = 1288;
/// The Fremennik sea boots 1 to 3.
pub const SEA_BOOTS: [i32; 3] = [14571, 14572, 14573];
/// The dummies `DummyAttackPlugin` registers "attack" on.
pub const DUMMIES: [i32; 13] = [
    2038, 15624, 15625, 15626, 15627, 15628, 15629, 15630, 18238, 25648, 28912, 823, 23921,
];

/// `on(PEER_THE_SEER_1288, NPC, "deposit")`.
pub fn peer_deposit<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    if h.any_in_equipment(p, &SEA_BOOTS)? {
        h.open_deposit_box(p)?;
        h.set_interface_text(p, "Peer the Seer's Deposits", 11, 12)?;
    } else {
        // .also { END_DIALOGUE }: no effect.
        h.npc_dialogue(
            p,
            PEER_THE_SEER,
            "Do not pester me, outerlander! I will only deposit items into the banks of those who have earned Fremennik sea boots!",
            Expression::Annoyed,
            false,
        )?;
    }
    Ok(true)
}

/// `DummyAttackPlugin.handle`.
pub fn dummy_attack<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.lock_ticks(p, 3)?;
    let animation = h.attack_animation(p)?;
    h.animate_entity(p, Some(animation))?;
    if h.combat_level(p)? < 8 {
        let mut experience = 5.0_f64;
        match h.attack_style(p)? {
            // STYLE_ACCURATE
            0 => h.add_experience(p, 0, experience)?,
            // STYLE_AGGRESSIVE: strength
            1 => h.add_experience(p, 2, experience)?,
            // STYLE_DEFENSIVE: defence
            3 => h.add_experience(p, 1, experience)?,
            // STYLE_CONTROLLED: shared
            2 => {
                experience /= 3.0;
                h.add_experience(p, 0, experience)?;
                h.add_experience(p, 2, experience)?;
                h.add_experience(p, 1, experience)?;
            }
            _ => {}
        }
    } else {
        h.packet_message(p, "You swing at the dummy...")?;
        h.packet_message(
            p,
            "There is nothing more you can learn from hitting a dummy.",
        )?;
    }
    Ok(true)
}
