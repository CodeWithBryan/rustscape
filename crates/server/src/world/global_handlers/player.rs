//! The player option handlers (`content/global/handlers/player`):
//! `AttackOptionPlugin`, `FollowOptionPlugin` with its MovementPulse body,
//! `RequestOption` and the login plugin `LoginValidationPlugin`.
use super::{Host, Kind, Result, Store, Task, Value, java_cast, kotlin_cast};
use crate::world::trade::RequestType;

/// `AttackOptionPlugin.handle`: `player.attack(node)`, true.
pub fn attack<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    h.attack(player, node)?;
    Ok(true)
}

/// `FollowOptionPlugin.handle`: the target cast, then a FOLLOW_ENTITY
/// MovementPulse run as a STANDARD pulse.
pub fn follow<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    // final Player target = ((Player) node);
    java_cast(h, node, Kind::is_player)?;
    let body = Task::Follow {
        player: player.clone(),
        target: node.cloned(),
    };
    let pulse = h.movement_pulse(player, node, body)?;
    h.run_pulse(player, &pulse, Some(super::PulseType::Standard))?;
    Ok(true)
}

/// The follow pulse's `pulse()`: face the target, never finish.
pub fn follow_pulse<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    target: Option<&H::Node>,
) -> Result<bool> {
    h.face(player, target)?;
    Ok(false)
}

/// The follow pulse's `stop()`: the MovementPulse stop, then `mover.face(null)`.
pub fn follow_stop<H: Host + ?Sized>(h: &mut H, mover: &H::Node, pulse: &H::Pulse) -> Result<()> {
    h.movement_stop(pulse)?;
    h.face(mover, None)?;
    Ok(())
}

/// `RequestOption`'s "Trade with" listener.
pub fn trade<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    let target = kotlin_cast(h, node, Kind::is_player)?;
    h.request(player, target, RequestType::Trade)?;
    Ok(true)
}

/// `RequestOption`'s "Req Assist" listener.
pub fn assist<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    if let Some(bot) = node.filter(|n| h.kind(n) == Kind::Bot) {
        h.bot_info(player, bot)?;
    }
    let target = kotlin_cast(h, node, Kind::is_player)?;
    h.request(player, target, RequestType::Assist)?;
    Ok(true)
}

/// `Items.QUEST_POINT_CAPE_9813`.
pub const QUEST_POINT_CAPE: i32 = 9813;
/// `Items.QUEST_POINT_HOOD_9814`.
pub const QUEST_POINT_HOOD: i32 = 9814;
/// One day in milliseconds (`TimeUnit.DAYS.toMillis(1)`).
const DAY: i64 = 86_400_000;

/// `LoginValidationPlugin.newInstance(player)`.
pub fn login_validation<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    h.unlock(player)?;
    if h.is_artificial(player)? {
        return Ok(());
    }
    if !h.valid_login(player)? {
        return Ok(());
    }
    if h.dev_mode()? {
        h.toggle_debug(player)?;
    }
    let wave = h.get_attribute(player, "fc_wave")?;
    if Value::int(wave, -1)? > -1 {
        h.activity_start(player, "fight caves", true)?;
    }
    let falconry = h.get_attribute(player, "falconry")?;
    if Value::bool(falconry, false)? {
        h.activity_start(player, "falconry", true)?;
    }
    if h.dragon_slayer_attribute(player, "repaired")? {
        h.set_varp(player, 177, 1_967_876, false)?;
    }
    let now = h.current_time_millis();
    if h.loot_share_delay(player)? < now && h.loot_share_delay(player)? != 0 {
        // (int) (points - points * 0.10)
        let first = f64::from(h.loot_share_points(player)?);
        let second = f64::from(h.loot_share_points(player)?);
        h.set_loot_share_points(player, java_d2i(first - second * 0.10))?;
    } else {
        let now = h.current_time_millis();
        h.set_loot_share_delay(player, now.wrapping_add(DAY))?;
    }
    check_quest_point_items(h, player)
}

/// Java's `(int)` of a double: truncation, saturating, NaN to 0.
pub fn java_d2i(v: f64) -> i32 {
    if v.is_nan() { 0 } else { v as i32 }
}

/// `checkQuestPointsItems`.
fn check_quest_point_items<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    if h.has_completed_all(player)?
        || !h.any_in_equipment(player, &[QUEST_POINT_CAPE, QUEST_POINT_HOOD])?
    {
        return Ok(());
    }
    let mut first: Option<String> = None;
    let mut second: Option<String> = None;
    let (mut item1, mut item2, mut amount) = (0, 0, 0);
    for id in [QUEST_POINT_CAPE, QUEST_POINT_HOOD] {
        if !h.remove_item(player, id, 1, Store::Equipment)? {
            continue;
        }
        amount += 1;
        let location = if h.add_item(player, id, 1, Store::Inventory)? {
            "your inventory"
        } else if h.add_item(player, id, 1, Store::Bank)? {
            "your bank"
        } else {
            let key = if id == QUEST_POINT_CAPE {
                "/save:reclaim-qp-cape"
            } else {
                "/save:reclaim-qp-hood"
            };
            h.api_set_attribute(player, key, Value::Bool(true))?;
            "the Wise Old Man"
        };
        if amount == 1 {
            item1 = id;
            first = Some(location.into());
        }
        if amount == 2 {
            item2 = id;
            second = Some(location.into());
        }
    }
    let text = |v: Option<String>| v.unwrap_or_else(|| "null".into());
    if amount == 2 {
        let name1 = h.item_name(item1)?;
        let name2 = h.item_name(item2)?;
        let message = format!(
            "As you no longer have completed all the quests, your {name1} unequips itself to {} and your {name2} unequips itself to {}!",
            text(first),
            text(second)
        );
        h.double_item_dialogue(player, item1, item2, &message)
    } else {
        let name1 = h.item_name(item1)?;
        let message = format!(
            "As you no longer have completed all the quests, your {name1} unequips itself to {}!",
            text(first)
        );
        h.item_dialogue(player, item1, &message)
    }
}
