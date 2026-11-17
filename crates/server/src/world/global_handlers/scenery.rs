//! The scenery listeners and option handlers
//! (`content/global/handlers/scenery`): bank booths, chests and deposit
//! boxes, the signpost "look at", "search" and the ladder options.
use super::{
    Error, Host, IronmanMode, Kind, Result, Store, Task, java_cast, kotlin_cast,
    tabs::restrict_for_ironman,
};

/// `org.rs09.consts.Scenery` bank booths (`BankBoothListener.BANK_BOOTHS`).
pub const BANK_BOOTHS: [i32; 32] = [
    2213, 2214, 3045, 5276, 6084, 10517, 11338, 11402, 11758, 12798, 12799, 14367, 14368, 16700,
    18491, 19230, 20325, 20326, 20327, 20328, 22819, 24914, 25808, 26972, 29085, 30015, 30016,
    34205, 34752, 35647, 36786, 37474,
];
/// `BankChestListener`'s chests.
pub const BANK_CHESTS: [i32; 9] = [3194, 4483, 10562, 14382, 16695, 16696, 21301, 27662, 27663];
/// `BankDepositBoxListener`'s deposit boxes.
pub const BANK_DEPOSIT_BOXES: [i32; 7] = [9398, 20228, 25937, 26969, 34755, 36788, 39830];
/// `NPCs.BANKER_494`.
pub const BANKER: i32 = 494;

/// `quickBankBoothUse`.
pub fn booth_quick<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    if h.check_restriction(p, IronmanMode::Ultimate)? {
        return Ok(true);
    }
    if h.lunar_restriction(p, node)? {
        banker_dialogue(h, p, node)?;
        return Ok(true);
    }
    h.open_bank(p)?;
    Ok(true)
}

/// `regularBankBoothUse`.
pub fn booth_regular<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    if h.check_restriction(p, IronmanMode::Ultimate)? {
        return Ok(true);
    }
    if h.booth_quick_open() {
        return booth_quick(h, p, node);
    }
    banker_dialogue(h, p, node)?;
    Ok(true)
}

/// `collectBankBoothUse`.
pub fn booth_collect<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    if h.lunar_restriction(p, node)? {
        banker_dialogue(h, p, node)?;
        return Ok(true);
    }
    h.open_collection_box(p)?;
    Ok(true)
}

/// `attemptToConvertItems(player, used, with)`.
pub fn booth_use<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    used: Option<&H::Node>,
    with: &H::Node,
) -> Result<bool> {
    if !h.has_option(with, "use")? {
        let id = h.id(with)?;
        h.api_message(
            p,
            &format!("You shouldn't be able to do that with object {id}."),
        )?;
        h.api_message(p, "Please screenshot this and report to the developers.")?;
        return Ok(true);
    }
    if !h.booth_note_uim() && h.check_restriction(p, IronmanMode::Ultimate)? {
        return Ok(true);
    }
    if h.lunar_restriction(p, with)? {
        banker_dialogue(h, p, with)?;
        return Ok(true);
    }
    convert_to_notes(h, used, p)?;
    Ok(true)
}

/// `BankBoothListener.convertToNotes(used, player)`.
pub fn convert_to_notes<H: Host + ?Sized>(
    h: &mut H,
    used: Option<&H::Node>,
    p: &H::Node,
) -> Result<()> {
    let item = kotlin_cast(h, used, |k| k == Kind::Item)?;
    let id = h.id(item)?;
    if h.note_change(item)? != id {
        if h.is_unnoted(item)? {
            let amount = h.amount_in_inventory(p, id)?;
            if h.remove_item(p, id, amount, Store::Inventory)? {
                let note = h.note_change(item)?;
                h.add_item(p, note, amount, Store::Inventory)?;
            }
        } else {
            let mut amount = h.amount(item)?;
            let free = h.free_slots(p)?;
            // One more note than free slots: the disappearing note frees the last slot.
            if amount > free && amount != free.wrapping_add(1) {
                amount = free;
            }
            if h.remove_item(p, id, amount, Store::Inventory)? {
                let note = h.note_change(item)?;
                h.add_item(p, note, amount, Store::Inventory)?;
            }
        }
        return Ok(());
    }
    h.api_message(p, "This item can't be noted.")
}

/// `as? BankerNPC` of the first NPC found.
fn banker<H: Host + ?Sized>(h: &H, npc: H::Node) -> Option<H::Node> {
    (h.kind(&npc) == Kind::Banker).then_some(npc)
}

/// `locateAdjacentBankerLinear`: the first NPC north, east, south or west
/// (any NPC stops the search, a non-banker answers null).
fn banker_linear<H: Host + ?Sized>(h: &mut H, node: &H::Node) -> Result<Option<H::Node>> {
    // Direction.NORTH, EAST, SOUTH, WEST: Location.transform(dir) keeps the plane.
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        let l = h.location(node)?.ok_or(Error::Null)?;
        let at = [l[0].wrapping_add(dx), l[1].wrapping_add(dy), l[2]];
        if let Some(npc) = h.find_npc(at)? {
            return Ok(banker(h, npc));
        }
    }
    Ok(None)
}

/// `locateAdjacentBankerSquare(node, 2)`: `Location(x, y)` is on plane 0.
fn banker_square<H: Host + ?Sized>(
    h: &mut H,
    node: &H::Node,
    size: i32,
) -> Result<Option<H::Node>> {
    let l = h.location(node)?.ok_or(Error::Null)?;
    let (low_y, high_y) = (l[1].wrapping_sub(size), l[1].wrapping_add(size));
    let l = h.location(node)?.ok_or(Error::Null)?;
    for y in low_y..=high_y {
        let (low_x, high_x) = (l[0].wrapping_sub(size), l[0].wrapping_add(size));
        for x in low_x..=high_x {
            if let Some(npc) = h.find_npc([x, y, 0])? {
                return Ok(banker(h, npc));
            }
        }
    }
    Ok(None)
}

/// `tryInvokeBankerDialogue`.
pub fn banker_dialogue<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<()> {
    let found = match banker_linear(h, node)? {
        Some(npc) => Some(npc),
        None => banker_square(h, node, 2)?,
    };
    let Some(npc) = found else {
        return Ok(());
    };
    let id = h.id(&npc)?;
    if h.dialogue_contains(id)? {
        let at = h.location(node)?;
        h.face_location(&npc, at)?;
        let id = h.id(&npc)?;
        let location = h.location(&npc)?;
        h.open_dialogue(p, id, vec![super::DialogueArg::NewNpc { id, location }])
    } else {
        h.open_dialogue(p, BANKER, Vec::new())
    }
}

/// `BankChestListener`'s "bank"/"use".
pub fn chest<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.open_bank(p)?;
    Ok(true)
}

/// `BankDepositBoxListener.openDepositBox`.
pub fn deposit_box<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    restrict_for_ironman(h, p, IronmanMode::Ultimate, |h| {
        if !h.open(p, 11)? {
            return Err(Error::Null);
        }
        h.set_close_event(p, 11, Task::DepositBoxClose)?;
        h.remove_tabs(p, &[0, 1, 2, 3, 4, 5, 6])?;
        let inventory = h.inventory(p)?;
        let items = h.to_array(&inventory)?;
        h.generate_items(
            p,
            items,
            &[
                "Examine",
                "Deposit-X",
                "Deposit-All",
                "Deposit-10",
                "Deposit-5",
                "Deposit-1",
            ],
            11,
            15,
            5,
            7,
            None,
        )
    })?;
    Ok(true)
}

/// `LookAtOptionPlugin.handle`.
pub fn look_at<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: Option<&H::Node>) -> Result<bool> {
    let name = h.name(node.ok_or(Error::Null)?)?.ok_or(Error::Null)?;
    let text = format!(
        "The {} seem to be going south-west.",
        h.lowercase_default(&name)
    );
    h.packet_message(p, &text)?;
    Ok(true)
}

/// `SearchOptionPlugin.handle`.
pub fn search<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: Option<&H::Node>) -> Result<bool> {
    let node = node.ok_or(Error::Null)?;
    let name = h.name(node)?.ok_or(Error::Null)?;
    if name == "Bookcase" {
        h.packet_message(p, "You search the books...")?;
        h.packet_message(p, "You find nothing of interest to you.")?;
        return Ok(true);
    }
    if h.id(node)? == 14743 {
        let inventory = h.inventory(p)?;
        if !h.contain_items(&inventory, &[946])? && !h.in_combat(p)? {
            h.packet_message(p, "You mindlessly reach into the sack labeled 'knives'...")?;
            h.packet_message_delay(
                p,
                "Against all odds you pull out a knife without hurting yourself.",
                2,
            )?;
            let inventory = h.inventory(p)?;
            h.add(&inventory, 946, 1)?;
            return Ok(true);
        }
    }
    let name = h.name(node)?.ok_or(Error::Null)?;
    let text = format!(
        "You search the {} but find nothing.",
        h.lowercase_default(&name)
    );
    h.packet_message(p, &text)?;
    Ok(true)
}

/// `LadderManagingPlugin.handle`.
pub fn ladder<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    java_cast(h, node, |k| k == Kind::Scenery)?;
    super::climb::climb_ladder(h, p, node, option)?;
    Ok(true)
}

/// `LadderManagingPlugin.getDestination`.
pub fn ladder_destination<H: Host + ?Sized>(
    h: &mut H,
    node: Option<&H::Node>,
) -> Result<Option<[i32; 3]>> {
    java_cast(h, node, |k| k == Kind::Scenery)?;
    super::climb::get_destination(h, node)
}
