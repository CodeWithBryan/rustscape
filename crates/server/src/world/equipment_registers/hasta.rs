//! `HastaEquipListener`: hastas cannot be equipped before Otto Godblessed's
//! hasta training has begun.
use super::Host;
use crate::world::combat_swing::{Error, Result};
use rs09_game_core::java_lower::lowercase_root;

/// `BarbarianTraining.attributeHasta`.
pub const ATTRIBUTE_HASTA: &str = "/save:barbtraining:hasta";
pub const MESSAGE: &str =
    "You must begin the relevant section of Otto Godblessed's barbarian training.";

/// The ids `defineListeners` collects: every item definition, in the
/// definition map's iteration order (`definitions`, an input of the world),
/// whose lower-case name contains "hasta". A null name fails the scan.
pub fn hasta_ids<'a>(
    definitions: impl IntoIterator<Item = (i32, Option<&'a str>)>,
) -> Result<Vec<i32>> {
    let mut ids = Vec::new();
    for (id, name) in definitions {
        if lowercase_root(name.ok_or(Error::Null)?).contains("hasta") {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// `defineListeners()`: one `onEquip` over the collected ids.
pub fn define_listeners<'a, H: Host>(
    h: &mut H,
    definitions: impl IntoIterator<Item = (i32, Option<&'a str>)>,
) -> Result<()> {
    let ids = hasta_ids(definitions)?;
    h.on_equip(&ids)
}

/// The `onEquip` handler: false, with a dialogue, below stage 2 of the
/// training.
pub fn on_equip<H: Host>(h: &mut H, player: &H::Entity) -> Result<bool> {
    if h.attribute_int(player, ATTRIBUTE_HASTA, 0)? < 2 {
        h.send_dialogue(player, MESSAGE)?;
        return Ok(false);
    }
    Ok(true)
}
