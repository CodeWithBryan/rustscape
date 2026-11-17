//! `toys/`: Diango's holiday-item reclaim (DiangoOptionHandler and the
//! DiangoReclaimInterface component plugin) and the snow globe's interface
//! (SnowGlobePlugin's SnowGlobeInterface).
use super::{Anim, Container, Error, Gfx, Host, Result, Task, Value};

/// Diango's NPC id.
pub const DIANGO: i32 = 970;
/// The reclaim interface.
pub const RECLAIM_INTERFACE: i32 = 468;
/// The snow globe interface.
pub const SNOW_GLOBE_INTERFACE: i32 = 659;
/// The attribute the offered items are kept in.
pub const RECLAIMABLES: &str = "diango-reclaimables";

/// `DiangoReclaimInterface.HOLIDAY_ITEMS`, in order.
pub const HOLIDAY_ITEMS: [i32; 38] = [
    4079, 10507, 10508, 4566, 6722, 6857, 6856, 6858, 6859, 6860, 6861, 6862, 6863, 9920, 9921,
    9922, 9923, 9924, 9925, 11019, 11020, 11021, 11022, 11789, 11949, 12645, 14076, 14077, 14081,
    14595, 14602, 14603, 14605, 14654, 14596, 6867, 6866, 6865,
];
/// The staff of the raven and the two items that stand for it.
const STAFF_OF_THE_RAVEN: i32 = 14654;
const RAVEN_IN_INVENTORY: i32 = 14655;
const RAVEN_EQUIPPED: i32 = 14656;

/// `DiangoReclaimInterface.newInstance`: the holiday items join `ITEMS`
/// (every call adds them again).
pub fn reclaim_new_instance(items: &mut Vec<i32>) {
    items.extend(HOLIDAY_ITEMS);
}

/// `DiangoReclaimInterface.getEligibleItems(player)`: each item of `ITEMS`
/// not worn, carried, banked or in the toy box (the staff of the raven also
/// needs buying and neither of its forms).
pub fn eligible_items<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
) -> Result<Vec<Option<H::Node>>> {
    let ids = h.statics().diango_items.clone();
    let mut out = Vec::new();
    for id in ids {
        let item = h.constant_item(id, 1);
        if h.container_contains(player, Container::Equipment, &item)?
            || h.container_contains(player, Container::Inventory, &item)?
            || h.container_contains(player, Container::Bank, &item)?
            || h.storage_contains(player, "TOY_BOX", id)?
        {
            continue;
        }
        if id == STAFF_OF_THE_RAVEN {
            if h.in_inventory(player, RAVEN_IN_INVENTORY, 1)?
                || h.in_equipment(player, RAVEN_EQUIPPED, 1)?
            {
                continue;
            }
            let purchased = match h.get_attribute(player, "sotr:purchased")? {
                None | Some(Value::Null) => false,
                Some(Value::Bool(b)) => b,
                Some(_) => return Err(Error::Cast),
            };
            if !purchased {
                continue;
            }
        }
        out.push(Some(item));
    }
    Ok(out)
}

/// `DiangoReclaimInterface.open(player)`: the open interface closes, the
/// eligible items are kept and shown, and the interface opens.
pub fn open<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    if let Some(current) = h.opened_component(player)? {
        h.component_close(player, current)?;
    }
    let items = eligible_items(h, player)?;
    h.set_attribute(player, RECLAIMABLES, Value::Items(items.clone()))?;
    if !items.is_empty() {
        h.generate_items(
            player,
            &items,
            &["Examine", "Take"],
            RECLAIM_INTERFACE,
            2,
            8,
            8,
        )?;
    }
    h.open_component(player, RECLAIM_INTERFACE, None)
}

/// DiangoOptionHandler's `handle`: the reclaim interface. True.
pub fn holiday_items<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    open(h, player)?;
    Ok(true)
}

/// `DiangoReclaimInterface.handle`: take (155: the item, then the interface
/// again) or examine (196); always false.
pub fn reclaim_handle<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    opcode: i32,
    slot: i32,
) -> Result<bool> {
    let reclaimables = match h.get_attribute(player, RECLAIMABLES)? {
        None | Some(Value::Null) => eligible_items(h, player)?,
        Some(Value::Items(items)) => items,
        Some(_) => return Err(Error::Cast),
    };
    let item = usize::try_from(slot)
        .ok()
        .and_then(|s| reclaimables.get(s).cloned())
        .ok_or(Error::Index)?;
    let Some(item) = item else {
        h.send_message(player, "Something went wrong there. Please try again.")?;
        return Ok(true);
    };
    match opcode {
        155 => {
            h.container_add(player, Container::Inventory, &item)?;
            h.close_interface(player)?;
            open(h, player)?;
        }
        196 => {
            let id = h.id(&item)?;
            let examine = h.item_examine(id).ok_or(Error::Null)?;
            h.send_message(player, &examine)?;
        }
        _ => {}
    }
    Ok(false)
}

const DOWNFAST: Anim = Anim::new(7537);
const DOWNSLOW: Anim = Anim::new(7538);
const STOMP: Anim = Anim::new(7528);
const SNOW: Gfx = Gfx::new(1284);
/// `Items.SNOWBALL_11951`.
pub const SNOWBALL: i32 = 11951;

/// `SnowGlobeInterface.handle`: button 2 (continue) closes the interface and
/// fills the inventory with snowballs a tick later; anything else puts the
/// globe down. True.
pub fn snow_globe_handle<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    button: i32,
) -> Result<bool> {
    if button == 2 {
        h.close_interface(player)?;
        h.animate(player, DOWNSLOW)?;
        h.pulse_run(
            player,
            Task::SnowGlobe {
                player: player.clone(),
            },
            1,
        )?;
        return Ok(true);
    }
    h.animate(player, DOWNFAST)?;
    Ok(true)
}

/// The snow globe pulse: the stomp, then a snowball for every free slot.
pub fn snow_globe_pulse<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.visualize(player, STOMP, SNOW)?;
    let free = h.container_free_slots(player, Container::Inventory)?;
    let snow = h.new_item(SNOWBALL, free)?;
    h.container_add(player, Container::Inventory, &snow)?;
    Ok(true)
}
