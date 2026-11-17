//! The item-on-object basics: filling vessels at water sources, buckets at
//! sand pits, the Blue Moon Inn chest key, the hat stand and the sack of hay.
use super::tables::{self, WaterVessel};
use super::{
    Anim, Error, Host, ItemArg, Kind, Result, RunBody, Store, Task, char_lower,
    contains_ignore_case, run_task,
};

// ---------------------------------------------------------------------- WaterSourceListener

/// `WaterSourceListener.animation`.
pub const FILL_ANIMATION: Anim = Anim::new(832);
/// `Sounds.WELL_FILL_2615` and `Sounds.TAP_FILL_2609`.
pub const WELL_FILL: i32 = 2615;
pub const TAP_FILL: i32 = 2609;
/// The Falador water pump (the Falador diary's task 0/7).
pub const FALADOR_PUMP: i32 = 11661;

/// `formatMsgText(name, template)`: every `@` becomes the name up to its
/// first `(`, each character lower-cased on its own.
pub fn format_message(name: &str, template: &str) -> String {
    let mut out = String::new();
    for tc in template.chars() {
        if tc == '@' {
            for nc in name.chars() {
                if nc == '(' {
                    break;
                }
                out.push(char_lower(nc));
            }
        } else {
            out.push(tc);
        }
    }
    out
}

/// WaterSourceListener's use-with lambda: a vessel at a source. Only the
/// bucket goes down a well; the Falador pump completes its diary task; the
/// fill pulse (delay 1) does the rest. False for anything not a vessel.
pub fn water<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    let Some(vessel) = WaterVessel::for_id(h.id(used)?) else {
        return Ok(false);
    };
    let source = h.name(with)?.ok_or(Error::Null)?;
    if contains_ignore_case(&source, "well") && !vessel.wellable() {
        let name = h.name(used)?.ok_or(Error::Null)?;
        h.send_message(player, &format_message(&name, tables::NON_WELLABLE))?;
        return Ok(true);
    }
    if vessel == WaterVessel::Bucket && h.id(with)? == FALADOR_PUMP {
        h.get_diary(player, super::Diary::Falador)?;
        if !h.diary_complete(player, super::Diary::Falador, 0, 7)? {
            h.get_diary(player, super::Diary::Falador)?;
            h.diary_update(player, super::Diary::Falador, 0, 7, true)?;
        }
    }
    h.pulse_run(
        player,
        Task::WaterFill {
            player: player.clone(),
            used: used.clone(),
            with: with.clone(),
            vessel,
        },
        1,
    )?;
    Ok(true)
}

/// The fill pulse's `pulse()`: one vessel filled (when it is removed); it
/// finishes once none of that vessel is left.
pub fn water_pulse<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
    vessel: WaterVessel,
) -> Result<bool> {
    let id = h.id(used)?;
    if h.remove_item(player, ItemArg::Id(id), Store::Inventory)? {
        h.animate(player, FILL_ANIMATION)?;
        let name = h.name(used)?.ok_or(Error::Null)?;
        h.send_message(player, &format_message(&name, vessel.fill_message()))?;
        h.add_item_or_drop(player, vessel.output(), 1)?;
        let source = h.name(with)?.ok_or(Error::Null)?;
        let sound = if contains_ignore_case(&source, "well") {
            WELL_FILL
        } else {
            TAP_FILL
        };
        h.play_audio(player, sound, 0, 1, None, 8)?;
    }
    if !vessel.autofill() {
        return Ok(true);
    }
    let id = h.id(used)?;
    Ok(h.amount_in_inventory(player, id)? == 0)
}

// ---------------------------------------------------------------------- SandSourceListener

/// `Items.BUCKET_1925` and `Items.BUCKET_OF_SAND_1783`.
pub const BUCKET: i32 = 1925;
pub const BUCKET_OF_SAND: i32 = 1783;
/// `SandSourceListener.ANIMATION`.
pub const SAND_ANIMATION: Anim = Anim::new(895);

/// SandSourceListener's use-with lambda: a `runTask` of delay 2 repeated
/// once per empty bucket counted now. True.
pub fn sand<H: Host + ?Sized>(h: &mut H, player: &H::Node, used: &H::Node) -> Result<bool> {
    let id = h.id(used)?;
    let buckets = h.amount_in_inventory(player, id)?;
    run_task(
        h,
        player,
        2,
        buckets,
        RunBody::Sand {
            player: player.clone(),
            used: used.clone(),
            trigger: 0,
        },
    )?;
    Ok(true)
}

/// The sand task's body: a bucket removed (animating every other time) and a
/// bucket of sand added; the shared trigger counts every run.
pub fn sand_task<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    trigger: &mut i32,
) -> Result<()> {
    if h.remove_item(player, ItemArg::Item(used.clone()), Store::Inventory)? {
        if *trigger % 2 == 0 {
            h.animate(player, SAND_ANIMATION)?;
        }
        if h.add_item(player, BUCKET_OF_SAND, 1, Store::Inventory)? {
            h.send_message(player, "You fill the bucket with sand.")?;
        }
    }
    *trigger = trigger.wrapping_add(1);
    Ok(())
}

// ---------------------------------------------------------------------- ChestKeyListener, HatStand, SackOnHay

/// `Items.CHEST_KEY_432`, `Scenery.CHEST_2079`, `Items.PIRATE_MESSAGE_433`.
pub const CHEST_KEY: i32 = 432;
pub const CHEST: i32 = 2079;
pub const PIRATE_MESSAGE: i32 = 433;

/// ChestKeyListener's use-with lambda: the key goes, the chest opens for
/// three ticks and the message is taken.
pub fn chest_key<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    if !h.remove_item(player, ItemArg::Item(used.clone()), Store::Inventory)? {
        return Ok(false);
    }
    if h.kind(with) != Kind::Scenery {
        return Err(Error::Cast);
    }
    h.replace_scenery(with, 2080, 3, None)?;
    h.add_item(player, PIRATE_MESSAGE, 1, Store::Inventory)?;
    h.send_message(player, "You unlock the chest.")?;
    h.send_message(player, "All that's in the chest is a message...")?;
    h.send_message(player, "You take the message from the chest.")?;
    Ok(true)
}

/// The hat stand.
pub const HAT_STAND: i32 = 374;

/// `HatStand.hats`: every definition whose `equipment_slot` (0 without one)
/// is the head's, in the definitions table's order.
pub fn hats(definitions: &[super::Definition]) -> Vec<i32> {
    definitions
        .iter()
        .filter(|d| d.equipment_slot.unwrap_or(0) == tables::HEAD)
        .map(|d| d.id)
        .collect()
}

/// HatStand's use-with lambda.
pub fn hat_stand<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.send_dialogue(player, "It'd probably fall off if I tried to do that.")?;
    Ok(true)
}

/// `Items.EMPTY_SACK_5418` and `Items.HAY_SACK_6057`.
pub const EMPTY_SACK: i32 = 5418;
pub const HAY_SACK: i32 = 6057;

/// SackOnHay's use-with lambda.
pub fn sack_on_hay<H: Host + ?Sized>(h: &mut H, player: &H::Node, used: &H::Node) -> Result<bool> {
    let sack = super::as_item(h, used)?.clone();
    if h.remove_item(player, ItemArg::Item(sack), Store::Inventory)? {
        h.add_item(player, HAY_SACK, 1, Store::Inventory)?;
        h.send_message(player, "You fill the sack with hay.")?;
    }
    Ok(true)
}
