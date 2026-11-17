//! `SkullManager` with `SkullIcon` and its anonymous deep-wilderness
//! container listener: the wilderness flag and level, the deep-wilderness
//! state that `setLevel` derives at 49 and 48, the teleport lock above level
//! 20, the skull causes and the Skulled timer `checkSkull` starts, the skull
//! icon, and the risk-based deep-wilderness skull.
//!
//! The manager's fields are [`SkullManager`], one per player, reached through
//! [`Host::skull`]. The Skulled timer is a required service: `checkSkull`
//! removes the "skulled" timer, asks the registry for a new one
//! (`spawnTimer("skulled", 2000)`) and registers it when the registry has one.
//! The timers port answers those three calls; the Skulled timer's own bodies
//! call back [`set_skull_icon`], [`SkullManager::set_skulled`] and [`reset`]
//! (see docs/player-managers.md for the join). The deep-wilderness listener is
//! a native container listener object ([`DeepWildListener`]); a world binding
//! dispatches it by type to [`listener_update`] and [`listener_refresh`].
use super::{Error, Kind, PlayerRef, Result};
use crate::inventory::container_listener::{Handle as Listener, NativeListener};

/// `SkullManager.DEEP_WILD_DROP_RISK_THRESHOLD`.
pub const DEEP_WILD_DROP_RISK_THRESHOLD: i64 = 100_000;
/// The attribute holding the listener.
pub const LISTENER_ATTRIBUTE: &str = "deepwild-value-listener";
/// The attribute holding the risked value.
pub const RISK_ATTRIBUTE: &str = "deepwild-value-risk";
/// The timer `checkSkull` restarts, and its run interval.
pub const SKULLED_TIMER: &str = "skulled";
pub const SKULLED_TICKS: i32 = 2000;

/// `SkullManager.SkullIcon`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkullIcon {
    None,
    White,
    Red,
    BhRed5,
    BhBlue4,
    BhGreen3,
    BhGrey2,
    BhBrown1,
    Scream,
}
impl SkullIcon {
    pub const ALL: [SkullIcon; 9] = [
        Self::None,
        Self::White,
        Self::Red,
        Self::BhRed5,
        Self::BhBlue4,
        Self::BhGreen3,
        Self::BhGrey2,
        Self::BhBrown1,
        Self::Scream,
    ];
    pub fn id(self) -> i32 {
        Self::ALL.iter().position(|i| *i == self).unwrap() as i32 - 1
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::White => "WHITE",
            Self::Red => "RED",
            Self::BhRed5 => "BH_RED5",
            Self::BhBlue4 => "BH_BLUE4",
            Self::BhGreen3 => "BH_GREEN3",
            Self::BhGrey2 => "BH_GREY2",
            Self::BhBrown1 => "BH_BROWN1",
            Self::Scream => "SCREAM",
        }
    }
    /// `forId`: 0 to 7, anything else NONE.
    pub fn for_id(id: i32) -> Self {
        match id {
            0..=7 => Self::ALL[id as usize + 1],
            _ => Self::None,
        }
    }
}

/// The manager's fields.
#[derive(Clone, Debug)]
pub struct SkullManager<P> {
    player: P,
    pub wilderness: bool,
    pub wilderness_disabled: bool,
    pub level: i32,
    /// `skullCauses`: the players this player skulled on, an ArrayList
    /// compared by identity.
    pub skull_causes: Vec<P>,
    pub skulled: bool,
    pub skull_check_disabled: bool,
    pub deep_wilderness: bool,
}
impl<P: Clone> SkullManager<P> {
    pub fn new(player: P) -> Self {
        Self {
            player,
            wilderness: false,
            wilderness_disabled: false,
            level: 0,
            skull_causes: Vec::new(),
            skulled: false,
            skull_check_disabled: false,
            deep_wilderness: false,
        }
    }
    /// `getPlayer()`.
    pub fn player(&self) -> &P {
        &self.player
    }
    /// `isSkulled()`: the field or deep wilderness.
    pub fn is_skulled(&self) -> bool {
        self.skulled || self.deep_wilderness
    }
    /// `hasWildernessProtection()`: below level 49.
    pub fn has_wilderness_protection(&self) -> bool {
        self.level < 49
    }
    /// `setSkulled(skulled)`.
    pub fn set_skulled(&mut self, skulled: bool) {
        self.skulled = skulled;
    }
}

/// The value of the "deepwild-value-listener" attribute.
#[derive(Clone, Debug)]
pub enum ListenerAttribute {
    Absent,
    Listener(Listener),
    /// Any other value: the cast to ContainerListener fails.
    Other,
}

/// The anonymous ContainerListener of `updateDWSkullIcon`. Its update calls
/// its refresh, its refresh `updateDWSkullIcon()` of the manager that made it.
pub struct DeepWildListener<P> {
    pub player: P,
}
impl<P: PlayerRef> NativeListener for DeepWildListener<P> {
    fn retained_nodes(&self) -> Vec<crate::world::player_walk::NodeHandle> {
        self.player.retained_nodes()
    }
}

/// Every call SkullManager makes outside its class.
pub trait Host {
    type Player: PlayerRef;
    /// What `spawnTimer` returns: a timer object of the timers port.
    type Timer;
    /// The player's own manager. The borrow ends before any service call.
    fn skull(&mut self, player: &Self::Player) -> Result<&mut SkullManager<Self::Player>>;
    /// `ContentAPI.removeTimer(player, identifier)`.
    fn remove_timer(&mut self, player: &Self::Player, identifier: &str) -> Result<()>;
    /// `ContentAPI.spawnTimer(identifier, runInterval)`: the registry's new
    /// instance, or None (`registerTimer` then returns at once).
    fn spawn_timer(&mut self, identifier: &str, run_interval: i32) -> Result<Option<Self::Timer>>;
    /// `entity.timers.registerTimer(timer)`.
    fn register_timer(&mut self, player: &Self::Player, timer: Self::Timer) -> Result<()>;
    /// `getAppearance().setSkullIcon(icon)`.
    fn appearance_skull_icon(&mut self, player: &Self::Player, icon: i32) -> Result<()>;
    /// `updateAppearance()`.
    fn update_appearance(&mut self, player: &Self::Player) -> Result<()>;
    /// `getAppearance().sync()`.
    fn appearance_sync(&mut self, player: &Self::Player) -> Result<()>;
    /// `getLocks().lockTeleport(ticks)`.
    fn lock_teleport(&mut self, player: &Self::Player, ticks: i32) -> Result<()>;
    /// `getLocks().unlockTeleport()`.
    fn unlock_teleport(&mut self, player: &Self::Player) -> Result<()>;
    /// `ServerConstants.ENHANCED_DEEP_WILDERNESS`, a world setting.
    fn enhanced_deep_wilderness(&mut self) -> bool;
    /// `getAttribute("deepwild-value-listener")`.
    fn listener_attribute(&mut self, player: &Self::Player) -> Result<ListenerAttribute>;
    /// `setAttribute("deepwild-value-listener", listener)`.
    fn set_listener_attribute(&mut self, player: &Self::Player, listener: &Listener) -> Result<()>;
    /// `setAttribute("deepwild-value-risk", value)` (a Long).
    fn set_risk_attribute(&mut self, player: &Self::Player, value: i64) -> Result<()>;
    /// `removeAttribute(key)`.
    fn remove_attribute(&mut self, player: &Self::Player, key: &str) -> Result<()>;
    /// `getInventory()`/`getEquipment()` `.getListeners().add(listener)`.
    fn add_listener(
        &mut self,
        player: &Self::Player,
        kind: Kind,
        listener: &Listener,
    ) -> Result<()>;
    /// `.getListeners().remove(listener)`.
    fn remove_listener(
        &mut self,
        player: &Self::Player,
        kind: Kind,
        listener: &Listener,
    ) -> Result<()>;
    /// `toArray()`: (id, amount) per slot.
    fn items(&mut self, player: &Self::Player, kind: Kind) -> Result<Vec<Option<(i32, i32)>>>;
    /// `ItemDefinition.forId(id).getAlchemyValue(true)`.
    fn alchemy_value(&mut self, id: i32) -> Result<i32>;
}

/// `setSkullIcon(icon)`: the appearance's icon, then an appearance update.
pub fn set_skull_icon<H: Host>(h: &mut H, player: &H::Player, icon: i32) -> Result<()> {
    h.appearance_skull_icon(player, icon)?;
    h.update_appearance(player)
}

/// `reset()`: the causes cleared, no icon, not skulled, appearance synced.
pub fn reset<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.skull(player)?.skull_causes.clear();
    set_skull_icon(h, player, -1)?;
    h.skull(player)?.skulled = false;
    h.appearance_sync(player)
}

/// `checkSkull(other)`: `other` is the player handle when the entity is a
/// Player, None for an NPC or null. The cause is added before the timer is
/// restarted, so a running Skulled timer's removal (which resets) clears it.
pub fn check_skull<H: Host>(
    h: &mut H,
    player: &H::Player,
    other: Option<&H::Player>,
) -> Result<()> {
    let Some(other) = other else {
        return Ok(());
    };
    {
        let own = h.skull(player)?;
        if !own.wilderness || own.skull_check_disabled {
            return Ok(());
        }
    }
    if h.skull(other)?.skull_causes.iter().any(|p| p == player) {
        return Ok(());
    }
    {
        let own = h.skull(player)?;
        if own.skull_causes.contains(other) {
            return Ok(());
        }
        own.skull_causes.push(other.clone());
    }
    h.remove_timer(player, SKULLED_TIMER)?;
    if let Some(timer) = h.spawn_timer(SKULLED_TIMER, SKULLED_TICKS)? {
        h.register_timer(player, timer)?;
    }
    Ok(())
}

/// `setLevel(level)`: deep wilderness from 49, left below 48; the teleport
/// lock above 20; then the level.
pub fn set_level<H: Host>(h: &mut H, player: &H::Player, level: i32) -> Result<()> {
    if !h.skull(player)?.deep_wilderness && level >= 49 {
        set_deep_wilderness(h, player, true)?;
    } else if h.skull(player)?.deep_wilderness && level < 48 {
        set_deep_wilderness(h, player, false)?;
    }
    if level > 20 {
        h.lock_teleport(player, 1_000_000)?;
    } else {
        h.unlock_teleport(player)?;
    }
    h.skull(player)?.level = level;
    Ok(())
}

/// `setDeepWilderness(deep)`: the risk skull with the enhanced setting when
/// entering, otherwise its removal; then the skull check and the flag.
pub fn set_deep_wilderness<H: Host>(h: &mut H, player: &H::Player, deep: bool) -> Result<()> {
    if h.enhanced_deep_wilderness() && deep {
        update_dw_skull_icon(h, player)?;
    } else {
        remove_dw_skull_icon(h, player)?;
    }
    let own = h.skull(player)?;
    own.skull_check_disabled = deep;
    own.deep_wilderness = deep;
    Ok(())
}

/// `Item.getAlchemyValue()`: at least 1 per item (the definition read twice
/// when above 1), times the amount, as a long.
fn alchemy<H: Host>(h: &mut H, id: i32, amount: i32) -> Result<i64> {
    let mut value: i64 = 1;
    if i64::from(h.alchemy_value(id)?) > value {
        value = i64::from(h.alchemy_value(id)?);
    }
    Ok(value.wrapping_mul(i64::from(amount)))
}

/// `updateDWSkullIcon()`: the listener installed once (attribute first,
/// then the inventory's and the equipment's lists), the risked value summed
/// with protect item's most valuable item left out, stored, and the red skull
/// at the threshold, the brown one below it.
pub fn update_dw_skull_icon<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    if matches!(h.listener_attribute(player)?, ListenerAttribute::Absent) {
        let listener = Listener::new(DeepWildListener {
            player: player.clone(),
        });
        h.set_listener_attribute(player, &listener)?;
        h.add_listener(player, Kind::Inventory, &listener)?;
        h.add_listener(player, Kind::Equipment, &listener)?;
    }
    let mut value: i64 = 0;
    let mut max: i64 = 0;
    for kind in [Kind::Inventory, Kind::Equipment] {
        for (id, amount) in h.items(player, kind)?.into_iter().flatten() {
            let alch = alchemy(h, id, amount)?;
            value = value.wrapping_add(alch);
            max = max.max(alch);
        }
    }
    value = value.wrapping_sub(max);
    h.set_risk_attribute(player, value)?;
    let skull = if value >= DEEP_WILD_DROP_RISK_THRESHOLD {
        SkullIcon::Red
    } else {
        SkullIcon::BhBrown1
    };
    set_skull_icon(h, player, skull.id())
}

/// `removeDWSkullIcon()`: the white skull when skulled, none otherwise; the
/// listener taken off both lists (the attribute's value is cast first); both
/// attributes removed.
pub fn remove_dw_skull_icon<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    let icon = if h.skull(player)?.skulled { 0 } else { -1 };
    set_skull_icon(h, player, icon)?;
    match h.listener_attribute(player)? {
        ListenerAttribute::Other => return Err(Error::Cast),
        ListenerAttribute::Listener(listener) => {
            h.remove_listener(player, Kind::Inventory, &listener)?;
            h.remove_listener(player, Kind::Equipment, &listener)?;
        }
        ListenerAttribute::Absent => {}
    }
    h.remove_attribute(player, LISTENER_ATTRIBUTE)?;
    h.remove_attribute(player, RISK_ATTRIBUTE)
}

/// The SkullManager members other libraries declare as required services,
/// answered from the native manager (docs/player-managers.md lists each
/// service and its answer). `set_level`, `check_skull`, `reset` and
/// `set_skull_icon` above answer the setters with effects.
pub mod answers {
    use super::{Host, Result};

    /// `isWilderness()`: zones `skull_wilderness`, npc_behaviour
    /// `skull_wilderness`, combat_pulse `is_wilderness`, player_death
    /// `skull_wilderness`.
    pub fn is_wilderness<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
        Ok(h.skull(player)?.wilderness)
    }
    /// `setWilderness(value)`: wilderness_zone `set_skull_wilderness`.
    pub fn set_wilderness<H: Host>(h: &mut H, player: &H::Player, value: bool) -> Result<()> {
        h.skull(player)?.wilderness = value;
        Ok(())
    }
    /// `getLevel()`: zones `skull_level`.
    pub fn level<H: Host>(h: &mut H, player: &H::Player) -> Result<i32> {
        Ok(h.skull(player)?.level)
    }
    /// `isWildernessDisabled()`: wilderness_zone `skull_wilderness_disabled`.
    pub fn is_wilderness_disabled<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
        Ok(h.skull(player)?.wilderness_disabled)
    }
    /// `setWildernessDisabled(value)`.
    pub fn set_wilderness_disabled<H: Host>(
        h: &mut H,
        player: &H::Player,
        value: bool,
    ) -> Result<()> {
        h.skull(player)?.wilderness_disabled = value;
        Ok(())
    }
    /// `isDeepWilderness()`: wilderness_zone `skull_deep_wilderness`.
    pub fn is_deep_wilderness<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
        Ok(h.skull(player)?.deep_wilderness)
    }
    /// `isSkulled()`: player_death `skulled`.
    pub fn is_skulled<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
        Ok(h.skull(player)?.is_skulled())
    }
    /// `setSkulled(value)`: player_death `set_skulled`, Skulled.onRegister.
    pub fn set_skulled<H: Host>(h: &mut H, player: &H::Player, value: bool) -> Result<()> {
        h.skull(player)?.set_skulled(value);
        Ok(())
    }
    /// `hasWildernessProtection()`: npc_behaviour's service.
    pub fn has_wilderness_protection<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
        Ok(h.skull(player)?.has_wilderness_protection())
    }
    /// `isSkullCheckDisabled()`.
    pub fn is_skull_check_disabled<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
        Ok(h.skull(player)?.skull_check_disabled)
    }
    /// `setSkullCheckDisabled(value)`.
    pub fn set_skull_check_disabled<H: Host>(
        h: &mut H,
        player: &H::Player,
        value: bool,
    ) -> Result<()> {
        h.skull(player)?.skull_check_disabled = value;
        Ok(())
    }
}

/// The listener's `update(container, event)`: its own refresh.
pub fn listener_update<H: Host>(h: &mut H, listener: &DeepWildListener<H::Player>) -> Result<()> {
    listener_refresh(h, listener)
}

/// The listener's `refresh(container)`: `updateDWSkullIcon()` of its manager.
pub fn listener_refresh<H: Host>(h: &mut H, listener: &DeepWildListener<H::Player>) -> Result<()> {
    update_dw_skull_icon(h, &listener.player)
}
