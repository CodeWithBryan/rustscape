//! DragonfireShieldListener.kt: the dragonfire shield's "operate" (its
//! breath attack through an anonymous `DragonfireSwingHandler`), "empty",
//! "inspect" and "drop" options, and `DFSItemPlugin`.
use super::{
    Container, Dragonfire, Host, ItemValue, ProjectileTemplate, Shield, Subclass, SwitchAttack,
    Value, gfx, handler,
};
use crate::world::combat_pulse::{handler as base, pulse as cpulse, slot};
use crate::world::combat_swing::{
    BattleState, Error, Handler, HandlerKind, InteractionType, Result,
};
use crate::world::special_attacks::Priority;

pub const SHIELDS: [i32; 2] = [
    handler::DRAGONFIRE_SHIELD,
    handler::DRAGONFIRE_SHIELD_UNCHARGED,
];
const CHARGED: ItemValue = (handler::DRAGONFIRE_SHIELD, 1, 1000);
const UNCHARGED: ItemValue = (handler::DRAGONFIRE_SHIELD_UNCHARGED, 1, 1000);
/// The special attack bar varp.
const SPECIAL_VARP: i32 = 301;

/// The options `defineListeners` registers on both shields, in order.
pub const OPTIONS: [&str; 4] = ["operate", "empty", "inspect", "drop"];

/// One listener instance: its `dfsRecharge` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listener {
    pub dfs_recharge: i32,
}

/// `secondsToTicks(seconds)`.
const fn seconds_to_ticks(seconds: i32) -> i32 {
    seconds.wrapping_mul(1000) / 600
}

/// `minutesToTicks(minutes)`.
const fn minutes_to_ticks(minutes: i32) -> i32 {
    minutes.wrapping_mul(60).wrapping_mul(1000) / 600
}

impl Listener {
    /// The constructor: 30 seconds of recharge with `BETTER_DFS`, two minutes
    /// otherwise, read once.
    pub fn new<H: Host>(h: &H) -> Self {
        Self {
            dfs_recharge: if h.better_dfs() {
                seconds_to_ticks(30)
            } else {
                minutes_to_ticks(2)
            },
        }
    }

    /// `defineListeners()`.
    pub fn define_listeners<H: Host>(&self, h: &mut H) -> Result<()> {
        for option in OPTIONS {
            h.listen(&SHIELDS, option)?;
        }
        Ok(())
    }

    /// The handler registered for `option`, with the clicked item.
    pub fn handle<H: Host>(
        &self,
        h: &mut H,
        option: &str,
        player: &H::Entity,
        item: &H::Item,
    ) -> Result<bool> {
        match option {
            "operate" => self.operate(h, player, item),
            "empty" => empty(h, player, item),
            "inspect" => inspect(h, player, item),
            "drop" => drop(h, player, item),
            _ => Err(Error::Binding(format!("no {option} listener"))),
        }
    }

    /// "operate": a second operate cancels the prepared attack; otherwise a
    /// charged, not recharging shield prepares a dragonfire attack that swings
    /// at once while attacking within reach, or replaces the next swing.
    pub fn operate<H: Host>(&self, h: &mut H, player: &H::Entity, item: &H::Item) -> Result<bool> {
        let using_attack = !h.attribute_flag(player, "dfs_spec")?;
        if !using_attack {
            if !special_toggled(h, player) {
                h.set_varp(player, SPECIAL_VARP, 0)?;
            }
            h.remove_attribute(player, "dfs_spec")?;
            h.combat_pulse(player).temporary_handler = None;
            return Ok(true);
        }
        let not_charged =
            h.item_id(item) == handler::DRAGONFIRE_SHIELD_UNCHARGED || h.item_charge(item) < 20;
        if not_charged {
            h.api_message(player, "Your shield has no charges left.")?;
            return Ok(true);
        }
        if h.is_locked(player, "dfs_recharge")? {
            h.api_message(player, "Your dragonfire shield is recharging.")?;
            return Ok(true);
        }
        h.set_varp(player, SPECIAL_VARP, 1)?;
        h.set_attribute_value(player, "dfs_spec", Value::Bool(true))?;
        let animation = h.new_animation(6696, Priority::Mid);
        let projectile =
            ProjectileTemplate::create(Some(player.clone()), None, 1166, 36, 36, 80, 70, 0, 11);
        let attack = SwitchAttack::new(
            None,
            Some(animation),
            Some(gfx(1165, 0)),
            Some(gfx(1167, 96)),
            Some(projectile),
        );
        let made = h.new_handler(
            Handler::new(HandlerKind::Magic, &[]),
            Subclass::Dragonfire(Dragonfire {
                melee_range: false,
                maximum_hit: 25,
                attack: Some(attack),
                fire: true,
                shield: Some(Shield {
                    player: player.clone(),
                    item: item.clone(),
                    recharge: self.dfs_recharge,
                }),
            }),
        );
        let victim = h.combat_pulse(player).victim.clone();
        if cpulse::is_attacking(h, player)? {
            let target = victim.clone().ok_or(Error::Null)?;
            if base::can_swing(h, &made, player, &target)? == InteractionType::StillInteract {
                cpulse::swing(h, player, victim.as_ref(), &made)?;
                return Ok(true);
            }
        }
        h.combat_pulse(player).temporary_handler = Some(made);
        Ok(true)
    }
}

/// `player.settings.isSpecialToggled`.
fn special_toggled<H: Host>(h: &mut H, player: &H::Entity) -> bool {
    let (settings, _) = h.settings(player);
    settings.fields.read().special_toggled
}

fn shield_of<H: Host>(h: &H, r: &H::HandlerRef) -> Result<Shield<H>> {
    handler::fields(h, r)?.shield.ok_or(Error::Null)
}

pub(crate) fn swing_member<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    swing(h, r, entity, victim, Some(state))
}

/// The anonymous handler's `swing`: a player attacker clears the prepared
/// attack, needs the charged shield worn (-1 otherwise), spends 20 charges
/// (the shield in the shield slot becomes uncharged below 20), is delayed 3
/// ticks and locks `dfs_recharge`; then `DragonfireSwingHandler.swing`.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let captured = shield_of(h, r)?;
    if let Some(e) = entity.filter(|e| h.kind(e).is_player()) {
        let player = &captured.player;
        if !special_toggled(h, player) {
            h.set_varp(player, SPECIAL_VARP, 0)?;
        }
        h.remove_attribute(player, "dfs_spec")?;
        let shield = h.equipment(player, slot::SHIELD)?;
        let Some(shield) = shield.filter(|s| h.item_id(s) == handler::DRAGONFIRE_SHIELD) else {
            return Ok(-1);
        };
        let at = h.location(e)?.ok_or(Error::Null)?;
        h.play_global_audio(at, 3761)?;
        h.delay_entity(player, 3)?;
        let charge = h.item_charge(&shield).wrapping_sub(20);
        h.set_item_charge(&shield, charge);
        if h.item_charge(&shield) < 20 && h.item_slot(&captured.item) == slot::SHIELD {
            let at = h.item_slot(&captured.item);
            h.replace_slot(player, at, UNCHARGED, Some(CHARGED), Container::Equipment)?;
        }
        h.update_bonuses(player)?;
        h.lock_key(player, "dfs_recharge", captured.recharge)?;
    }
    handler::swing(h, r, entity, victim, state)
}

/// The anonymous handler's `visualizeImpact`: the fire strike hit sound at
/// the victim 20 ticks later, then `DragonfireSwingHandler.visualizeImpact`.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let target = victim.ok_or(Error::Null)?;
    let at = h.location(target)?.ok_or(Error::Null)?;
    h.play_global_audio_delayed(at, 161, 20)?;
    handler::visualize_impact(h, r, entity, victim, state)
}

/// "empty": the shield in the clicked slot becomes uncharged.
pub fn empty<H: Host>(h: &mut H, player: &H::Entity, item: &H::Item) -> Result<bool> {
    let at = h.item_slot(item);
    h.replace_slot(player, at, UNCHARGED, Some(CHARGED), Container::Inventory)?;
    // ContentAPI visualize(entity, anim, gfx) with ids: Animation(id), Graphics(id).
    let animation = h.new_animation(6700, Priority::Mid);
    h.visualize_entity(player, Some(&animation), Some(gfx(1160, 0)))?;
    h.api_message(player, "You release the charges.")?;
    let here = h.location(player)?.ok_or(Error::Null)?;
    h.play_global_audio(here, 3760)?;
    Ok(true)
}

/// "inspect": the charges are the charge divided by 20.
pub fn inspect<H: Host>(h: &mut H, player: &H::Entity, item: &H::Item) -> Result<bool> {
    if h.item_id(item) == handler::DRAGONFIRE_SHIELD_UNCHARGED {
        h.api_message(player, "The shield has no charges.")?;
        return Ok(true);
    }
    let text = format!("The shield has {} charges.", h.item_charge(item) / 20);
    h.api_message(player, &text)?;
    Ok(true)
}

/// "drop": a charged shield is uncharged first and the inventory slot's item
/// is dropped.
pub fn drop<H: Host>(h: &mut H, player: &H::Entity, item: &H::Item) -> Result<bool> {
    let mut shield = Some(item.clone());
    let at = h.item_slot(item);
    if h.item_id(item) == handler::DRAGONFIRE_SHIELD {
        let slot = h.item_slot(item);
        h.replace_slot(player, slot, UNCHARGED, Some(CHARGED), Container::Inventory)?;
        shield = h.inventory_item(player, at)?;
    }
    let shield = shield.ok_or(Error::Null)?;
    h.drop_item(player, &shield)?;
    Ok(true)
}

/// `DFSItemPlugin.newInstance`: registers the charged shield.
pub fn item_plugin_new_instance<H: Host>(h: &mut H) -> Result<()> {
    h.register_item_plugin(handler::DRAGONFIRE_SHIELD)
}

/// `DFSItemPlugin.getDeathItem(item)`: an uncharged shield whatever the item.
pub fn death_item() -> ItemValue {
    UNCHARGED
}
