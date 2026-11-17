//! `ShooAwayStrayDogListener`: the stray dogs' "shoo-away" option.
use super::{Host, at, away_from};
use crate::world::combat_swing::Result;

/// `strayDogsIds`: the option's ids, and the ids its (empty) behaviour is
/// registered for.
pub const IDS: [i32; 4] = [4766, 4767, 5917, 5918];
/// `Animations.HUMAN_BLOW_RASPBERRY_2110`.
pub const RASPBERRY: i32 = 2110;
/// `Sounds.WOLF_DEATH_911`.
pub const WHINE_SOUND: i32 = 911;

/// `defineListeners()`.
pub fn define_listeners<H: Host>(h: &mut H) -> Result<()> {
    h.on_npc(Some(&IDS), &["shoo-away"])
}

/// The "shoo-away" handler: the player blows a raspberry, the dog whines,
/// kneels and runs twelve steps away; always true.
pub fn shoo_away<H: Host>(h: &mut H, player: &H::Entity, dog: &H::Entity) -> Result<bool> {
    h.lock(dog, 2)?;
    h.walking_reset(dog)?;
    h.face_temporary(dog, Some(player), 1)?;
    h.api_chat(player, "Thbbbbt!")?;
    h.api_animate(player, RASPBERRY)?;
    h.play_audio(player, WHINE_SOUND, 0)?;
    h.api_chat(dog, "Whine!")?;
    h.api_animate(dog, 4774)?;
    run_away(h, player, dog)?;
    Ok(true)
}

/// `dogWhineRunAway(player, dog)`.
pub fn run_away<H: Host>(h: &mut H, player: &H::Entity, dog: &H::Entity) -> Result<()> {
    let player_at = at(h, player)?;
    let dog_at = at(h, dog)?;
    let dest = away_from(dog_at, player_at, 12);
    h.unlock(dog)?;
    h.force_walk(dog, dest, "dumb")
}
