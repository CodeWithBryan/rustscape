//! `SheepBehavior`: the sheep's bleat and the catch-all "shear" option.
use super::{CreaturePulse, CreatureTask, Host, at, away_from, random, random_between};
use crate::world::combat_pulse::pulse;
use crate::world::combat_swing::Result;
use crate::world::dragonfire::Value;

/// `sheepIds`: the ids the behaviour is registered for.
pub const IDS: [i32; 36] = [
    42, 43, 1271, 1272, 1529, 1762, 1763, 1764, 1765, 2377, 2378, 2379, 2380, 3310, 3311, 3579,
    5148, 5149, 5150, 5151, 5152, 5153, 5154, 5155, 5156, 5157, 5158, 5159, 5160, 5161, 5162, 5163,
    5164, 5165, 5172, 5173,
];

pub const PENGUIN_SHEEP: i32 = 3579;
pub const SHEARED_SHEEP: i32 = 5153;
pub const SHEARS: i32 = 1735;
pub const WOOL: i32 = 1737;
/// `Quests.SHEEP_SHEARER`'s name.
pub const SHEEP_SHEARER: &str = "Sheep Shearer";
/// `SheepShearer.ATTR_IS_PENGUIN_SHEEP_SHEARED`.
pub const ATTR_PENGUIN_SHEARED: &str = "/save:sheep-shearer:is-penguin-sheep-sheared";
/// `Sounds.PENGUINSHEEP_ESCAPE_686`.
pub const PENGUIN_ESCAPE_SOUND: i32 = 686;
/// `Sounds.SHEAR_SHEEP_761`.
pub const SHEAR_SOUND: i32 = 761;

/// `tick(self)`: "Baa!" on a draw of 5 in 35 unless attacking or dead;
/// always true.
pub fn tick<H: Host>(h: &mut H, npc: &H::Entity) -> Result<bool> {
    if pulse::is_attacking(h, npc)? || h.is_dead(npc)? {
        return Ok(true);
    }
    if random(h, 35)? == 5 {
        h.api_chat(npc, "Baa!")?;
    }
    Ok(true)
}

/// `defineListeners()`: the catch-all NPC option "shear".
pub fn define_listeners<H: Host>(h: &mut H) -> Result<()> {
    h.on_npc(None, &["shear"])
}

/// The "shear" handler: the penguin sheep always escapes; otherwise shears,
/// a peaceful sheep and room for wool, then three draws in four shear it
/// (transformed for 80 ticks) and the fourth lets it back away.
pub fn shear<H: Host>(h: &mut H, player: &H::Entity, sheep: &H::Entity) -> Result<bool> {
    h.face_temporary(sheep, Some(player), 1)?;
    if h.id(sheep)? == PENGUIN_SHEEP {
        if h.quest_started(player, SHEEP_SHEARER)? {
            h.set_attribute_value(player, ATTR_PENGUIN_SHEARED, Value::Bool(true))?;
        }
        h.api_animate(player, 893)?;
        h.play_audio(player, PENGUIN_ESCAPE_SOUND, 0)?;
        h.api_animate(sheep, 3570)?;
        back_away(
            h,
            player,
            sheep,
            "The... whatever it is... manages to get away from you!",
        )?;
        return Ok(true);
    }
    if !h.inventory_contains(player, SHEARS, 1)? {
        h.api_message(player, "You need shears to shear a sheep.")?;
        return Ok(true);
    }
    if h.has_option(sheep, "attack")? {
        h.api_message(player, "That one looks a little too violent to shear...")?;
        return Ok(true);
    }
    if h.free_slots(player)? == 0 {
        h.api_message(
            player,
            "You don't have enough space in your inventory to carry any wool you would shear.",
        )?;
        return Ok(true);
    }
    h.lock(sheep, 3)?;
    h.walking_reset(sheep)?;
    h.api_animate(player, 893)?;
    if random_between(h, 1, 5)? != 4 {
        h.lock_movement(sheep, 2)?;
        h.npc_transform(sheep, SHEARED_SHEEP)?;
        h.play_audio(player, SHEAR_SOUND, 0)?;
        if !h.api_add_item(player, WOOL, 1)? {
            return Ok(false);
        }
        h.api_message(player, "You get some wool.")?;
        h.submit_creature(CreaturePulse::new(
            80,
            vec![Some(sheep.clone())],
            CreatureTask::Retransform { npc: sheep.clone() },
        ))?;
    } else {
        back_away(h, player, sheep, "The sheep manages to get away from you!")?;
    }
    Ok(true)
}

/// `sheepBackAway(player, sheep, message)`: the message, then the sheep is
/// unlocked and walks three steps the "opposite" way from the player.
pub fn back_away<H: Host>(
    h: &mut H,
    player: &H::Entity,
    sheep: &H::Entity,
    message: &str,
) -> Result<()> {
    let player_at = at(h, player)?;
    let sheep_at = at(h, sheep)?;
    let dest = away_from(sheep_at, player_at, 3);
    h.api_message(player, message)?;
    h.unlock(sheep)?;
    h.force_walk(sheep, dest, "dumb")
}
