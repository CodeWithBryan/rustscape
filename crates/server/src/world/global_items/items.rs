//! `TeleportCrystalPlugin` (with `TeleportCrystalDialogue`),
//! `EctophialListener`, `ExplorersRingPlugin` and `FaladorShieldPlugin`.
use super::{
    Anim, Arg, Dialogue, Error, Gfx, Host, ItemArg, Quest, QueueStrength, Result, Store,
    StoreValue, Task, TeleportType, Value, animate, as_item, delay_entity, store_int, teleport,
};

// ---------------------------------------------------------------------- teleport crystal

/// `TeleportCrystalDialogue.ID`.
pub const CRYSTAL_DIALOGUE: i32 = 3999111;
/// The teleport crystals 6099 (four charges) to 6102 (one).
pub const CRYSTALS: [i32; 4] = [6099, 6100, 6101, 6102];
/// Lletya (`new Location(2329, 3172)`, plane 0).
pub const LLETYA: [i32; 3] = [2329, 3172, 0];

/// `TeleportCrystalPlugin.handle(player, node, option)`.
pub fn crystal<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: Option<&H::Node>) -> Result<bool> {
    if !h.has_requirement(p, Quest::MourningsEndPartI, true)? {
        return Ok(true);
    }
    if !h.wilderness_check_teleport(p, 20)? {
        h.send_message(p, "The crystal is unresponsive.")?;
        return Ok(true);
    }
    let item = as_item(h, node.ok_or(Error::Null)?)?;
    let id = h.id(item)?;
    h.open_dialogue_plugin(p, CRYSTAL_DIALOGUE, &[Arg::Int(1), Arg::Int(id)])?;
    Ok(true)
}

/// `TeleportCrystalDialogue.newInstance(player)`.
pub fn crystal_dialogue<N: Clone>(player: &N) -> Dialogue<N> {
    Dialogue::Crystal {
        player: player.clone(),
        item_id: None,
        stage: 0,
    }
}

/// `TeleportCrystalDialogue.open(args...)`: `(Integer) args[1]` first, then
/// the switch on `(Integer) args[0]`.
pub fn crystal_open<H: Host + ?Sized>(
    h: &mut H,
    dialogue: &mut Dialogue<H::Node>,
    args: &[Arg],
) -> Result<bool> {
    let Dialogue::Crystal {
        player,
        item_id,
        stage,
    } = dialogue
    else {
        return Err(Error::Binding(Some("not the crystal dialogue".into())));
    };
    *item_id = match args.get(1) {
        None => return Err(Error::Index),
        Some(Arg::Int(i)) => Some(*i),
        Some(Arg::Text(_)) => return Err(Error::Cast),
    };
    match args.first() {
        None => return Err(Error::Index),
        Some(Arg::Text(_)) => return Err(Error::Cast),
        Some(Arg::Int(1)) => {
            h.send_options(
                player,
                "Select an Option",
                &["Teleport to Lletya", "Cancel"],
            )?;
            *stage = 100;
        }
        Some(Arg::Int(_)) => {}
    }
    Ok(true)
}

/// `TeleportCrystalDialogue.handle(interfaceId, buttonId)`: the teleport is
/// sent and the crystal degraded whatever `send` answers, and the dialogue
/// is not ended after it.
pub fn crystal_handle<H: Host + ?Sized>(
    h: &mut H,
    dialogue: &mut Dialogue<H::Node>,
    button: i32,
) -> Result<bool> {
    let Dialogue::Crystal {
        player,
        item_id,
        stage,
    } = dialogue
    else {
        return Err(Error::Binding(Some("not the crystal dialogue".into())));
    };
    h.new_npc(2376)?;
    h.new_npc(3155)?;
    if *stage == 100 {
        match button {
            1 => {
                h.teleport_send(player, LLETYA, TeleportType::Normal)?;
                let id = item_id.ok_or(Error::Null)?;
                let item = h.new_item(id, 1)?;
                degrade(h, player, &item)?;
            }
            2 => {
                // DialoguePlugin.end(): the interpreter's close().
                h.interpreter_close(player)?;
            }
            _ => {}
        }
    }
    Ok(true)
}

/// `TeleportCrystalDialogue.degrade(player, item)`.
fn degrade<H: Host + ?Sized>(h: &mut H, p: &H::Node, item: &H::Node) -> Result<()> {
    let id = h.id(item)?;
    let next = id.wrapping_add(1);
    let old = h.new_item(id, 1)?;
    h.inventory_remove(p, &old)?;
    let new = h.new_item(next, 1)?;
    h.inventory_add(p, &new)?;
    if id < 6102 {
        h.send_message(p, "Your teleportation crystal has degraded from use.")
    } else {
        h.send_messages(
            p,
            &[
                "Your teleportation crystal has degraded to a tiny elf crystal,",
                "Eluned can re-enchant it.",
            ],
        )
    }
}

// ---------------------------------------------------------------------- ectophial

/// `Items.ECTOPHIAL_4251` (full).
pub const ECTOPHIAL: i32 = 4251;
/// `Items.ECTOPHIAL_4252` (empty).
pub const ECTOPHIAL_EMPTY: i32 = 4252;
/// `Scenery.ECTOFUNTUS_5282`.
pub const ECTOFUNTUS: i32 = 5282;
const FILL_ANIMATION: Anim = Anim::new(832);
const EMPTY_ANIMATION: Anim = Anim::new(9609);
const EMPTY_GRAPHICS: Gfx = Gfx::new(1688);

/// `refillEctophial(player)`.
pub fn ectophial_refill<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    let ticks = h.animation_duration(FILL_ANIMATION.id)?;
    delay_entity(h, p, ticks)?;
    animate(h, p, FILL_ANIMATION, false)?;
    h.play_audio(p, 1132, 0, 1, None, 8)?;
    if h.remove_item(p, ItemArg::Id(ECTOPHIAL_EMPTY), Store::Inventory)?
        && h.add_item(p, ECTOPHIAL, 1, Store::Inventory)?
    {
        h.send_message(p, "You refill the ectophial from the Ectofuntus.")?;
    }
    Ok(true)
}

/// The ectophial's `empty` lambda.
pub fn ectophial_empty<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    if !h.has_requirement(p, Quest::GhostsAhoy, true)? {
        return Ok(true);
    }
    if h.is_tele_blocked(p)? {
        h.send_message(p, "A magical force has stopped you from teleporting.")?;
        return Ok(true);
    }
    delay_entity(h, p, 10)?;
    h.close_all_interfaces(p)?;
    h.queue_script(
        p,
        0,
        QueueStrength::Soft,
        false,
        Task::Ectophial {
            player: p.clone(),
            node: node.clone(),
        },
    )?;
    Ok(true)
}

/// The ectophial's queued script.
pub fn ectophial_script<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    node: &H::Node,
    stage: i32,
) -> Result<bool> {
    match stage {
        0 => {
            h.send_message(
                p,
                "You empty the ectoplasm onto the ground around your feet...",
            )?;
            h.play_audio(p, 4580, 0, 1, None, 8)?;
            h.visualize(p, EMPTY_ANIMATION, EMPTY_GRAPHICS)?;
            let slot = h.slot(as_item(h, node)?)?;
            let empty = h.new_item(ECTOPHIAL_EMPTY, 1)?;
            let full = h.new_item(ECTOPHIAL, 1)?;
            h.replace_slot(p, slot, &empty, Some(&full), Store::Inventory)?;
            let ticks = h.animation_duration(EMPTY_ANIMATION.id)?;
            h.delay_script(p, ticks)
        }
        1 => {
            teleport(h, p, [3658, 3517, 0], TeleportType::Ectophial)?;
            h.send_message(p, "...and the world changes around you.")?;
            h.delay_script(p, 9)
        }
        2 => {
            h.face_location(p, [3659, 3519, 0])?;
            ectophial_refill(h, p)?;
            h.stop_executing(p)
        }
        _ => h.stop_executing(p),
    }
}

// ---------------------------------------------------------------------- explorer's ring

/// `RINGS`.
pub const RINGS: [i32; 3] = [13560, 13561, 13562];
/// `CABBAGE_PORT`.
pub const CABBAGE_PORT: [i32; 3] = [3051, 3291, 0];
/// `getStoreFile()`'s archive.
pub const RING_ARCHIVE: &str = "daily-explorer-ring";
/// `Skills.MAGIC`.
const MAGIC: i32 = 6;

/// `getRingLevel(id)`.
pub fn ring_level(id: i32) -> i32 {
    match id {
        13560 => 1,
        13561 => 2,
        13562 => 3,
        _ => -1,
    }
}

/// The `run-replenish` lambda.
pub fn ring_replenish<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    h.store_archive(RING_ARCHIVE)?;
    let user = h.username(p)?;
    let key = format!("{}:run", h.lowercase_default(&user));
    let charges = store_int(h.store_get(RING_ARCHIVE, &key)?.as_ref(), 0);
    if charges >= ring_level(h.id(node)?) {
        h.send_dialogue(
            p,
            "Your ring appears to have no more run energy recharges left for today.",
        )?;
        return Ok(true);
    }
    if h.run_energy(p)? == 100.0 {
        h.send_message(
            p,
            "You are fully rested. You do not need to use the ring's power for the moment.",
        )?;
        return Ok(true);
    }
    h.update_run_energy(p, -50.0)?;
    h.play_audio(p, 5035, 0, 1, None, 8)?;
    h.store_archive(RING_ARCHIVE)?;
    let user = h.username(p)?;
    let key = format!("{}:run", h.lowercase_default(&user));
    h.store_put(RING_ARCHIVE, &key, StoreValue::Int(charges.wrapping_add(1)))?;
    h.send_message(
        p,
        "You feel refreshed as the ring revitalises you and a charge is used up.",
    )?;
    h.visualize(p, Anim::new(9988), Gfx::new(1733))?;
    Ok(true)
}

/// The `low-alchemy` lambda.
pub fn ring_alchemy<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    if !h.has_level_stat(p, MAGIC, 21)? {
        h.send_message(p, "You need a Magic level of 21 in order to do that.")?;
        return Ok(true);
    }
    h.store_archive(RING_ARCHIVE)?;
    let user = h.username(p)?;
    let key = format!("{}:alchs", h.lowercase_root(&user));
    let remaining = store_int(h.store_get(RING_ARCHIVE, &key)?.as_ref(), 30);
    if remaining <= 0 {
        h.send_message(p, "You have used up all of your charges for the day.")?;
        return Ok(true);
    }
    h.send_dialogue(p, "Choose the item that you wish to convert to coins.")?;
    h.add_dialogue_action(
        p,
        Task::AlchemyAction {
            player: p.clone(),
            remaining,
        },
    )?;
    Ok(true)
}

/// The dialogue action: the item select with the captured count.
pub fn ring_alchemy_action<H: Host + ?Sized>(h: &mut H, p: &H::Node, remaining: i32) -> Result<()> {
    h.send_item_select(
        p,
        &["Choose"],
        false,
        Task::AlchemySelect {
            player: p.clone(),
            remaining,
        },
    )
}

/// The item-select callback: one charge less than when the option was used.
pub fn ring_alchemy_select<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    remaining: i32,
    slot: i32,
) -> Result<()> {
    let Some(item) = h.inventory_get(p, slot)? else {
        return Ok(());
    };
    if !h.alchemize(p, &item, false, true)? {
        return Ok(());
    }
    h.store_archive(RING_ARCHIVE)?;
    let user = h.username(p)?;
    let key = format!("{}:alchs", h.lowercase_root(&user));
    h.store_put(
        RING_ARCHIVE,
        &key,
        StoreValue::Int(remaining.wrapping_sub(1)),
    )
}

/// The `cabbage-port` lambda (and `teleport(player)`).
pub fn ring_cabbage<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    teleport(h, p, CABBAGE_PORT, TeleportType::Cabbage)?;
    Ok(true)
}

/// The `operate`/`rub` lambda.
pub fn ring_operate<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    if ring_level(h.id(node)?) < 3 {
        h.send_message(p, "This item can not be operated.")?;
        return Ok(true);
    }
    ring_cabbage(h, p)
}

// ---------------------------------------------------------------------- falador shield

/// The Falador shields 1 to 3.
pub const SHIELDS: [i32; 3] = [14577, 14578, 14579];
const ANIM_EMOTE: i32 = 11012;
const GFX_EMOTE: [i32; 3] = [1966, 1965, 1965];
const GFX_PRAYER_RESTORE: [i32; 3] = [1962, 1963, 1964];
/// The attribute the restore time is stored under (read without the prefix).
pub const RESTORE_TIME: &str = "diary:falador:shield-restore-time";

/// `getLevel(itemId)`.
pub fn shield_level(id: i32) -> i32 {
    match id {
        14577 => 0,
        14578 => 1,
        14579 => 2,
        _ => -1,
    }
}

fn level_value(table: &[i32; 3], level: i32) -> Result<i32> {
    usize::try_from(level)
        .ok()
        .and_then(|l| table.get(l).copied())
        .ok_or(Error::Index)
}

/// `FaladorShieldPlugin.handle(player, node, option)`.
pub fn falador_shield<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    // `(Item) node`: null passes the cast, then fails at getId().
    let item = match node {
        None => None,
        Some(n) => Some(as_item(h, n)?),
    };
    let level = shield_level(h.id(item.ok_or(Error::Null)?)?);
    match option.ok_or(Error::Null)? {
        "prayer-restore" => {
            let time = match h.get_attribute(p, RESTORE_TIME)? {
                None => None,
                Some(Value::Long(l)) => Some(l),
                Some(_) => return Err(Error::Cast),
            };
            let charged = match time {
                Some(t) => t > h.current_time_millis(),
                None => false,
            };
            if charged {
                h.send_message(p, "You have no charges left today.")?;
            } else {
                let bonus = if level == 0 {
                    0.25
                } else if level == 1 {
                    0.5
                } else {
                    1.0
                };
                h.prayer_effect(p, 0.0, bonus, false)?;
                h.graphics(p, Gfx::new(level_value(&GFX_PRAYER_RESTORE, level)?))?;
                let at = h.current_time_millis().wrapping_add(86_400_000);
                h.set_attribute(
                    p,
                    "/save:diary:falador:shield-restore-time",
                    Value::Long(at),
                )?;
                let which = if level < 2 { "some" } else { "your" };
                h.send_message(p, &format!("You restore {which} prayer points."))?;
            }
            return Ok(true);
        }
        "operate" => h.submit_world(Task::FaladorEmote {
            entity: p.clone(),
            level,
            delay: 0,
        })?,
        _ => {}
    }
    Ok(true)
}

/// The emote pulse's `pulse()`.
pub fn falador_pulse<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Node,
    level: i32,
    delay: &mut i32,
) -> Result<bool> {
    if *delay == 0 {
        h.lock_all(entity)?;
        h.animate(entity, Anim::new(ANIM_EMOTE))?;
        h.graphics(entity, Gfx::new(level_value(&GFX_EMOTE, level)?))?;
    } else if *delay == 3 {
        h.force_animation(entity, Anim::RESET)?;
        h.unlock(entity)?;
        return Ok(true);
    }
    *delay = delay.wrapping_add(1);
    Ok(false)
}
