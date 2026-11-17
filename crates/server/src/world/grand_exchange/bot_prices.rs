//! BotPrices.kt: the bot price of an item (fixed overrides, else the
//! definition's value).
use super::{Host, Result};

/// `getPriceOverrides(id)`.
pub fn price_override(id: i32) -> Option<i32> {
    Some(match id {
        7936 => 50,  // PURE_ESSENCE_7936
        1777 => 250, // BOW_STRING_1777
        1513 => 750, // MAGIC_LOGS_1513
        1739 => 250, // COWHIDE_1739
        536 => 1250, // DRAGON_BONES_536
        1753 => 550, // GREEN_DRAGONHIDE_1753
        207 => 1214, // GRIMY_RANARR_207
        211 => 453,  // GRIMY_AVANTOE_211
        215 => 232,  // GRIMY_CADANTINE_215
        217 => 86,   // GRIMY_DWARF_WEED_217
        199 => 50,   // GRIMY_GUAM_199
        205 => 115,  // GRIMY_HARRALANDER_205
        209 => 860,  // GRIMY_IRIT_209
        213 => 334,  // GRIMY_KWUARM_213
        2485 => 115, // GRIMY_LANTADYME_2485
        201 => 250,  // GRIMY_MARRENTILL_201
        379 => 268,  // LOBSTER_379
        377 => 265,  // RAW_LOBSTER_377
        987 => 5250, // LOOP_HALF_OF_A_KEY_987
        985 => 4263, // TOOTH_HALF_OF_A_KEY_985
        373 => 400,  // SWORDFISH_373
        371 => 390,  // RAW_SWORDFISH_371
        385 => 720,  // SHARK_385
        383 => 710,  // RAW_SHARK_383
        _ => return None,
    })
}

/// `getPrice(id)`.
pub fn get_price<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<i32> {
    if let Some(price) = price_override(id) {
        return Ok(price);
    }
    Ok(h.definitions()?.for_id(id).read().value)
}
