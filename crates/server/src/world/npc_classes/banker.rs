//! `BankerNPC`: the generic bankers' "bank", "collect" and "talk-to"
//! listeners, their destination override, the booth search of `init` and
//! the companion's `checkLunarIsleRestriction`, `canAccessBank`,
//! `attemptBank` and `attemptCollect`.
//!
//! The conversation (`BankerDialogueLabellerFile`) is ported in
//! `world::dialogue_dsl::banker`; here it stays the
//! [`Host::banker_conversation`] service, which a live host answers with
//! `dialogue_dsl::banker::talk_to`.
use super::{
    Class, Direction, Error, Host, Result, Value, as_npc, region_id, transform, transform_direction,
};
use crate::world::global_handlers::scenery::BANK_BOOTHS;

/// `LUNAR_ISLE_BANK_REGION`.
pub const LUNAR_ISLE_BANK_REGION: i32 = 8253;
/// `ODOVACAR_FEE`.
pub const ODOVACAR_FEE: i32 = 100;
/// `NPCs.ODOVACAR_5383`.
pub const ODOVACAR: i32 = 5383;
/// `Items.COINS_995`.
pub const COINS: i32 = 995;

/// `checkLunarIsleRestriction(player, node)`.
pub fn check_lunar_isle_restriction<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: &H::Npc,
) -> Result<bool> {
    let location = h.location(node)?.ok_or(Error::Null)?;
    if region_id(location) != LUNAR_ISLE_BANK_REGION {
        return Ok(false);
    }
    if !h.has_seal_of_passage(player)? {
        return Ok(true);
    }
    Ok(false)
}

/// `canAccessBank(player, npc)`.
pub fn can_access_bank<H: Host + ?Sized>(h: &mut H, player: &H::Npc, npc: &H::Npc) -> Result<bool> {
    if h.id(npc)? != ODOVACAR {
        return Ok(true);
    }
    if h.remove_item(player, COINS, ODOVACAR_FEE)? {
        h.api_message(player, "You pay 100 coins to use the travelling bank.")?;
        return Ok(true);
    }
    Ok(false)
}

/// `attemptBank(player, node)` (the "bank" listener).
pub fn attempt_bank<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    let npc = as_npc(h, node)?;
    if check_lunar_isle_restriction(h, player, npc)? {
        let id = h.id(npc)?;
        h.api_open_dialogue(player, id, npc)?;
        return Ok(true);
    }
    if !can_access_bank(h, player, npc)? {
        h.api_message(player, "You need 100 coins to use the travelling bank.")?;
        return Ok(true);
    }
    h.face_location(npc, None)?;
    h.open_bank_account(player)?;
    Ok(true)
}

/// `attemptCollect(player, node)` (the "collect" listener).
pub fn attempt_collect<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    let npc = as_npc(h, node)?;
    if check_lunar_isle_restriction(h, player, npc)? {
        let id = h.id(npc)?;
        h.api_open_dialogue(player, id, npc)?;
        return Ok(true);
    }
    h.face_location(npc, None)?;
    h.open_collection_box(player)?;
    Ok(true)
}

/// The "talk-to" listener: the banker conversation with `node as NPC`.
pub fn talk_to<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    let npc = as_npc(h, node)?;
    h.banker_conversation(player, npc)?;
    Ok(true)
}

/// `findAdjacentBankBoothLocation()` on `npc`: the first side (south, west,
/// north, east) whose next tile holds a bank booth, with the tile beyond it.
pub fn find_adjacent_bank_booth<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
) -> Result<Option<(Direction, [i32; 3])>> {
    for side in [
        Direction::South,
        Direction::West,
        Direction::North,
        Direction::East,
    ] {
        let location = h.location(npc)?.ok_or(Error::Null)?;
        let booth = transform_direction(location, side, 1);
        if let Some(scenery) = h.get_scenery(booth)? {
            let id = h.id(&scenery)?;
            if BANK_BOOTHS.contains(&id) {
                return Ok(Some((side, transform_direction(booth, side, 1))));
            }
        }
    }
    Ok(None)
}

/// `provideDestinationOverride(entity, node)`.
pub fn destination<H: Host + ?Sized>(h: &mut H, node: Option<&H::Npc>) -> Result<Option<[i32; 3]>> {
    let npc = as_npc(h, node)?;
    let location = |h: &mut H| h.location(npc)?.ok_or(Error::Null);
    Ok(Some(match h.id(npc)? {
        // Ogress bankers are 2x2 with their spawn on the south-western tile.
        7049 | 7050 => transform(location(h)?, 3, 1, 0),
        // Magnus has no bank booth nearby.
        5488 => transform_direction(location(h)?, Direction::North, 2),
        6532..=6535 => {
            let l = location(h)?;
            let direction = h.direction(npc)?.ok_or(Error::Null)?;
            transform_direction(l, direction, 1)
        }
        _ => {
            if h.class(npc) == Some(Class::Banker) {
                if let Some((_, tile)) = find_adjacent_bank_booth(h, npc)? {
                    return Ok(Some(tile));
                }
            }
            return h.location(npc);
        }
    }))
}

/// `init()`: `super.init()`, then a banker beside a booth faces it, stops
/// walking and is marked `facing_booth`.
pub fn init<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    h.base_init(npc)?;
    if let Some((side, _)) = find_adjacent_bank_booth(h, npc)? {
        h.set_direction(npc, Some(side))?;
        h.set_walks(npc, false)?;
        h.set_attribute(npc, "facing_booth", Value::Bool(true))?;
    }
    Ok(())
}
