//! `EnchantJewelleryTabListener`, `TeleTabsListener` and
//! `HouseTeleTabOptionListener`: the jewellery enchanting tablets, the
//! teleport tablets and the house tablet.
use super::{
    Altar, Anim, Error, Host, ItemArg, MonitorLog, QueueStrength, Result, Store, Task,
    TeleportType, Value, animate, as_item, delay_entity, item_to_string, teleport,
};

/// `Sounds.POH_TABLET_BREAK_979`.
pub const TABLET_BREAK_SOUND: i32 = 979;
/// `Animations.POH_TABLET_BREAK_4069`.
pub const TABLET_BREAK_ANIMATION: i32 = 4069;
/// The house tablet (`homeTabID`).
pub const HOUSE_TABLET: i32 = 8013;
/// `PlagueCityListeners.ARDOUGNE_TELE_ATTRIBUTE`.
pub const ARDOUGNE_TELE_ATTRIBUTE: &str = "/save:Ardougne:teleport";

/// `TAB_MAPPING`: each tablet with its `mapOf(unenchanted to enchanted)` in
/// insertion order.
pub const TAB_MAPPING: [(i32, [(i32, i32); 4]); 6] = [
    (
        8016,
        [(1637, 2550), (1656, 3853), (1694, 1727), (11072, 11074)],
    ),
    (
        8017,
        [(1639, 2552), (1658, 5521), (1696, 1729), (11076, 11079)],
    ),
    (
        8018,
        [(1641, 2568), (1660, 11194), (1698, 1725), (11085, 11088)],
    ),
    (
        8019,
        [(1643, 2570), (1662, 11090), (1700, 1731), (11092, 11095)],
    ),
    (
        8020,
        [(1645, 2572), (1664, 11113), (1702, 1704), (11115, 11126)],
    ),
    (
        8021,
        [(6575, 6583), (6577, 11128), (6581, 6585), (11130, 11133)],
    ),
];

/// The enchanting tablets' `break` lambda.
pub fn enchant_break<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.send_message(p, "Try using the tablet on the item instead.")?;
    Ok(true)
}

/// The enchanting tablets' use-with lambda (capturing `enchanted`).
pub fn enchant_use<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    tab_item: &H::Node,
    node: &H::Node,
    enchanted: i32,
) -> Result<bool> {
    let id = h.id(tab_item)?;
    let tab = h.new_item(id, 1)?;
    if h.remove_item(p, ItemArg::Item(tab), Store::Inventory)? {
        h.close_all_interfaces(p)?;
        h.play_audio(p, TABLET_BREAK_SOUND, 0, 1, None, 8)?;
        let anim = Anim::new(TABLET_BREAK_ANIMATION);
        animate(h, p, anim, true)?;
        let ticks = h.animation_duration(anim.id)?;
        delay_entity(h, p, ticks)?;
        let delay = h.animation_duration(anim.id)?;
        h.queue_script(
            p,
            delay,
            QueueStrength::Soft,
            false,
            Task::Enchant {
                player: p.clone(),
                node: node.clone(),
                enchanted,
            },
        )?;
    }
    Ok(true)
}

/// The enchanting script: the slot replaced, a dupe alert when the slot
/// held anything but the used item.
pub fn enchant_script<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    node: &H::Node,
    enchanted: i32,
) -> Result<bool> {
    let item = as_item(h, node)?;
    let slot = h.slot(item)?;
    let made = h.new_item(enchanted, 1)?;
    let previous = h.replace_slot(p, slot, &made, Some(item), Store::Inventory)?;
    if previous.as_ref() != Some(item) {
        let replaced = item_to_string(h, previous.as_ref())?;
        let used = item_to_string(h, Some(item))?;
        h.monitor_log(
            p,
            MonitorLog::DupeAlert,
            &format!(
                "Unknown slot-replacement problem when enchanting jewellery (adding {enchanted} replaced {replaced} rather than {used})"
            ),
        )?;
    }
    h.stop_executing(p)
}

/// `TeleTabsListener.TeleTabs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleTab {
    Ardougne,
    Camelot,
    Falador,
    Lumbridge,
    Varrock,
    WatchTower,
    RcGuild,
    Air,
    Mind,
    Water,
    Earth,
    Fire,
    Body,
    Cosmic,
    Chaos,
    Nature,
    Law,
    Death,
    Blood,
    Astral,
}

/// The tablet's requirement check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    /// The default `{ true }`.
    None,
    /// `getAttribute(player, ARDOUGNE_TELE_ATTRIBUTE, false)`.
    Ardougne,
    /// `MysteriousRuinListener.checkReq(player, altar)`.
    Altar(Altar),
}

impl TeleTab {
    pub const ALL: [Self; 20] = [
        Self::Ardougne,
        Self::Camelot,
        Self::Falador,
        Self::Lumbridge,
        Self::Varrock,
        Self::WatchTower,
        Self::RcGuild,
        Self::Air,
        Self::Mind,
        Self::Water,
        Self::Earth,
        Self::Fire,
        Self::Body,
        Self::Cosmic,
        Self::Chaos,
        Self::Nature,
        Self::Law,
        Self::Death,
        Self::Blood,
        Self::Astral,
    ];

    /// The tablet's item id, its destination and its requirement.
    pub fn data(self) -> (i32, [i32; 3], Requirement) {
        use Requirement as R;
        match self {
            Self::Ardougne => (8011, [2662, 3307, 0], R::Ardougne),
            Self::Camelot => (8010, [2757, 3477, 0], R::None),
            Self::Falador => (8009, [2966, 3380, 0], R::None),
            Self::Lumbridge => (8008, [3222, 3218, 0], R::None),
            Self::Varrock => (8007, [3212, 3423, 0], R::None),
            Self::WatchTower => (8012, [2548, 3114, 0], R::None),
            Self::RcGuild => (13598, [1696, 5460, 2], R::None),
            Self::Air => (13599, [2978, 3296, 0], R::Altar(Altar::Air)),
            Self::Mind => (13600, [2979, 3510, 0], R::Altar(Altar::Mind)),
            Self::Water => (13601, [3182, 3162, 0], R::Altar(Altar::Water)),
            Self::Earth => (13602, [3304, 3472, 0], R::Altar(Altar::Earth)),
            Self::Fire => (13603, [3311, 3252, 0], R::Altar(Altar::Fire)),
            Self::Body => (13604, [3055, 3443, 0], R::Altar(Altar::Body)),
            Self::Cosmic => (13605, [2411, 4380, 0], R::Altar(Altar::Cosmic)),
            Self::Chaos => (13606, [3058, 3593, 0], R::Altar(Altar::Chaos)),
            Self::Nature => (13607, [2868, 3013, 0], R::Altar(Altar::Nature)),
            Self::Law => (13608, [2857, 3378, 0], R::Altar(Altar::Law)),
            Self::Death => (13609, [1863, 4639, 0], R::Altar(Altar::Death)),
            Self::Blood => (13610, [3559, 9778, 0], R::Altar(Altar::Blood)),
            Self::Astral => (13611, [2156, 3862, 0], R::Altar(Altar::Astral)),
        }
    }

    /// `TeleTabs.forId(id)`.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.data().0 == id)
    }

    /// The registered ids, in declaration order.
    pub fn ids() -> Vec<i32> {
        Self::ALL.iter().map(|t| t.data().0).collect()
    }

    /// `requirementCheck(player)`.
    pub fn requirement<H: Host + ?Sized>(self, h: &mut H, p: &H::Node) -> Result<bool> {
        match self.data().2 {
            Requirement::None => Ok(true),
            Requirement::Ardougne => {
                let v = h.get_attribute(p, ARDOUGNE_TELE_ATTRIBUTE)?;
                Value::bool(v, false)
            }
            Requirement::Altar(altar) => h.altar_requirement(p, altar),
        }
    }
}

/// TeleTabsListener's `break` lambda.
pub fn teletab<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    let tab = h.id(node)?;
    let Some(t) = TeleTab::for_id(tab) else {
        return Ok(true);
    };
    if !h.in_inventory(p, tab, 1)? {
        return Ok(true);
    }
    let location = t.data().1;
    if h.in_inventory(p, tab, 1)? {
        if t.requirement(h, p)? {
            if teleport(h, p, location, TeleportType::Teletabs)? {
                let id = h.id(node)?;
                let item = h.new_item(id, 1)?;
                h.remove_item(p, ItemArg::Item(item), Store::Inventory)?;
            }
        } else if t == TeleTab::Ardougne {
            h.send_message(p, "You need to complete Plague City to use this tablet.")?;
        } else {
            h.send_message(p, "A strange power blocks your teleport.")?;
        }
    }
    Ok(true)
}

/// HouseTeleTabOptionListener's `break` lambda.
pub fn house_tablet<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<bool> {
    let has_house = h.house_exit_location(p)?.is_some();
    if !has_house {
        h.send_message(
            p,
            "You must have a house to teleport to before attempting that.",
        )?;
        return Ok(false);
    }
    h.close_interface(p)?;
    h.lock(p, 5)?;
    let id = h.id(node)?;
    if h.in_inventory(p, id, 1)? {
        h.house_pre_enter(p, false)?;
        // Kotlin's parameter check on `teleport(entity, loc, ...)`.
        let location = h.house_enter_location(p)?.ok_or(Error::Null)?;
        if teleport(h, p, location, TeleportType::Teletabs)? {
            let id = h.id(node)?;
            let item = h.new_item(id, 1)?;
            h.remove_item(p, ItemArg::Item(item), Store::Inventory)?;
            h.house_post_enter(p, false)?;
        }
    }
    Ok(true)
}
