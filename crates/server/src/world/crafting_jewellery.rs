//! `content.global.skill.crafting` at a furnace with a gold bar: jewellery
//! casting (`JewelleryCraftPlugin`, `jewellery.JewelleryCrafting`,
//! `jewellery.JewelleryPulse`,
//! `content.global.handlers.iface.JewelleryInterface`).
//!
//! Differential corpus: `fixtures/crafting-jewellery`
//! (oracle/crafting-jewellery), replayed by `tests/crafting_jewellery.rs`
//! over these functions with the oracle's declared state. Every outside
//! call is a [`Host`] method.
//!
//! Covered: the table, what the two classes register, the plugin's `handle`
//! and `JewelleryCrafting.open` ([`open`]), JewelleryInterface's `handle`
//! ([`button`]) with its amount prompt ([`input`]), `JewelleryCrafting.make`
//! and [`JewelleryPulse`] with `SkillPulse`'s `start`, `pulse` and `stop`.
//! Not covered: the rest of crafting (docs/crafting-jewellery.md).

/// `Skills.CRAFTING`.
pub const CRAFTING: i32 = 12;
/// The interface's opcodes: make one, five, all and X.
pub const OP_MAKE_ONE: i32 = 155;
pub const OP_MAKE_FIVE: i32 = 196;
pub const OP_MAKE_ALL: i32 = 124;
pub const OP_MAKE_X: i32 = 199;

/// The jewellery interface.
pub const INTERFACE: i32 = 446;
pub const RING_MOULD: i32 = 1592;
pub const AMULET_MOULD: i32 = 1595;
pub const NECKLACE_MOULD: i32 = 1597;
pub const BRACELET_MOULD: i32 = 11065;
pub const GOLD_BAR: i32 = 2357;
/// `open`'s `sendInterfaceConfig(446, child, true)` for a held mould.
pub const MOULD_CHILDREN: [(i32, i32); 4] = [
    (RING_MOULD, 14),
    (NECKLACE_MOULD, 36),
    (AMULET_MOULD, 55),
    (BRACELET_MOULD, 74),
];
/// The furnaces JewelleryCraftPlugin adds itself to, in `IDS`' order.
pub const FURNACES: [i32; 13] = [
    4304, 6189, 11010, 11666, 12100, 12809, 18497, 21303, 26814, 30021, 30510, 36956, 37651,
];
/// JewelleryPulse's animation and `Sounds.FURNACE_2725`.
pub const ANIMATION: i32 = 3243;
pub const SOUND: i32 = 2725;

/// The classes (`scan.initializable`).
pub const PLUGIN_CLASS: &str = "content.global.skill.crafting.JewelleryCraftPlugin";
pub const INTERFACE_CLASS: &str = "content.global.handlers.iface.JewelleryInterface";
pub const PULSE_CLASS: &str = "content.global.skill.crafting.jewellery.JewelleryPulse";

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// A ClassCastException.
    Cast,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A `JewelleryCrafting.JewelleryItem` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Jewellery {
    pub name: &'static str,
    pub level: i32,
    pub experience: f64,
    pub component: i32,
    pub send_item: i32,
    pub items: &'static [i32],
}
const fn row(
    name: &'static str,
    level: i32,
    experience: i32,
    component: i32,
    send_item: i32,
    items: &'static [i32],
) -> Jewellery {
    Jewellery {
        name,
        level,
        experience: experience as f64,
        component,
        send_item,
        items,
    }
}
pub const JEWELLERY: [Jewellery; 29] = [
    row("GOLD_RING", 5, 15, 19, 1635, &[2357]),
    row("SAPPIRE_RING", 20, 40, 21, 1637, &[1607, 2357]),
    row("EMERALD_RING", 27, 55, 23, 1639, &[1605, 2357]),
    row("RUBY_RING", 34, 70, 25, 1641, &[1603, 2357]),
    row("DIAMOND_RING", 43, 85, 27, 1643, &[1601, 2357]),
    row("DRAGONSTONE_RING", 55, 100, 29, 1645, &[1615, 2357]),
    row("ONYX_RING", 67, 115, 31, 6575, &[6573, 2357]),
    row("GOLD_NECKLACE", 6, 20, 41, 1654, &[2357]),
    row("SAPPHIRE_NECKLACE", 22, 55, 43, 1656, &[1607, 2357]),
    row("EMERALD_NECKLACE", 29, 60, 45, 1658, &[1605, 2357]),
    row("RUBY_NECKLACE", 40, 75, 47, 1660, &[1603, 2357]),
    row("DIAMOND_NECKLACE", 56, 90, 49, 1662, &[1601, 2357]),
    row("DRAGONSTONE_NECKLACE", 72, 105, 51, 1664, &[1615, 2357]),
    row("ONYX_NECKLACE", 82, 120, 53, 6577, &[6573, 2357]),
    row("SLAYER_RING", 75, 15, 34, 13281, &[4155, 2357]),
    row("GOLD_AMULET", 8, 30, 60, 1673, &[2357]),
    row("SAPPHIRE_AMULET", 24, 63, 62, 1675, &[1607, 2357]),
    row("EMERALD_AMULET", 31, 70, 64, 1677, &[1605, 2357]),
    row("RUBY_AMULET", 50, 85, 66, 1679, &[1603, 2357]),
    row("DIAMOND_AMULET", 70, 100, 68, 1681, &[1601, 2357]),
    row("DRAGONSTONE_AMULET", 80, 150, 70, 1683, &[1615, 2357]),
    row("ONYX_AMULET", 90, 165, 72, 6579, &[6573, 2357]),
    row("GOLD_BRACELET", 7, 25, 79, 11069, &[2357]),
    row("SAPPHIRE_BRACELET", 23, 60, 81, 11072, &[1607, 2357]),
    row("EMERALD_BRACELET", 30, 65, 83, 11076, &[1605, 2357]),
    row("RUBY_BRACELET", 42, 80, 85, 11085, &[1603, 2357]),
    row("DIAMOND_BRACELET", 58, 95, 87, 11092, &[1601, 2357]),
    row("DRAGONSTONE_BRACELET", 74, 110, 89, 11115, &[1615, 2357]),
    row("ONYX_BRACELET", 84, 125, 91, 11130, &[6573, 2357]),
];
fn named(name: &str) -> Option<&'static Jewellery> {
    JEWELLERY.iter().find(|j| j.name == name)
}

/// JewelleryInterface's two `switch`es: the constant of a button.
pub fn for_button(button: i32) -> Option<&'static Jewellery> {
    let name = match button {
        20 => "GOLD_RING",
        22 => "SAPPIRE_RING",
        24 => "EMERALD_RING",
        26 => "RUBY_RING",
        28 => "DIAMOND_RING",
        30 => "DRAGONSTONE_RING",
        32 => "ONYX_RING",
        35 => "SLAYER_RING",
        // `switch (button - 3)`.
        42 => "GOLD_NECKLACE",
        44 => "SAPPHIRE_NECKLACE",
        46 => "EMERALD_NECKLACE",
        48 => "RUBY_NECKLACE",
        50 => "DIAMOND_NECKLACE",
        52 => "DRAGONSTONE_NECKLACE",
        54 => "ONYX_NECKLACE",
        61 => "GOLD_AMULET",
        63 => "SAPPHIRE_AMULET",
        65 => "EMERALD_AMULET",
        67 => "RUBY_AMULET",
        69 => "DIAMOND_AMULET",
        71 => "DRAGONSTONE_AMULET",
        73 => "ONYX_AMULET",
        80 => "GOLD_BRACELET",
        82 => "SAPPHIRE_BRACELET",
        84 => "EMERALD_BRACELET",
        86 => "RUBY_BRACELET",
        88 => "DIAMOND_BRACELET",
        90 => "DRAGONSTONE_BRACELET",
        92 => "ONYX_BRACELET",
        _ => return None,
    };
    named(name)
}

/// `JewelleryCrafting.mouldFor(name)` of a constant's name.
pub fn mould_for(name: &str) -> i32 {
    let name = name.to_ascii_lowercase();
    if name.contains("ring") {
        RING_MOULD
    } else if name.contains("necklace") {
        NECKLACE_MOULD
    } else if name.contains("amulet") {
        AMULET_MOULD
    } else if name.contains("bracelet") {
        BRACELET_MOULD
    } else {
        -1
    }
}

/// `StringUtils.isPlusN(word)` of a lower-case word.
fn is_plus_n(word: &str) -> Result<bool> {
    let mut chars = word.chars();
    let first = chars
        .next()
        .ok_or_else(|| Error::Binding("StringIndexOutOfBoundsException: an empty name".into()))?;
    Ok(matches!(first, 'a' | 'e' | 'i' | 'o' | 'u')
        || (first == 'h' && chars.next().is_some_and(|second| second != 'e')))
}

/// What an amount prompt was answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Int(i32),
    Text(String),
}

/// What the bodies ask of the world, for one player.
pub trait Host {
    /// `player.getInterfaceManager().open(new Component(id))`.
    fn open_interface(&mut self, id: i32) -> Result<()>;
    /// `player.getInterfaceManager().close()`.
    fn close_interface(&mut self) -> Result<()>;
    /// `player.getInventory().contains(id, amount)`.
    fn contains(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `player.getInventory().getAmount(new Item(id))`.
    fn amount(&mut self, id: i32) -> Result<i32>;
    /// `sendInterfaceConfig(interface, child, hide)`.
    fn send_interface_config(&mut self, interface: i32, child: i32, hide: bool) -> Result<()>;
    /// `sendItemZoomOnInterface(item, zoom, interface, child)`.
    fn send_item_zoom(&mut self, item: i32, zoom: i32, interface: i32, child: i32) -> Result<()>;
    /// `player.getSkills().getLevel(Skills.CRAFTING)`.
    fn level(&mut self) -> Result<i32>;
    /// `ItemDefinition.forId(id).getName().toLowerCase()`; `None` for a
    /// null name.
    fn item_name_lowercase(&mut self, id: i32) -> Result<Option<String>>;
    /// `player.getPacketDispatch().sendMessage(message)`.
    fn send_message(&mut self, message: &str) -> Result<()>;
    /// `player.sendMessages(first, second)`.
    fn send_messages(&mut self, messages: &[&str]) -> Result<()>;
    /// `SlayerManager.getInstance(player).flags.isRingUnlocked()`.
    fn ring_unlocked(&mut self) -> Result<bool>;
    /// `sendInputDialogue(player, true, "Enter the amount:", handler)`: the
    /// handler is [`input`].
    fn send_input(&mut self, jewellery: &'static Jewellery) -> Result<()>;
    /// `player.getPulseManager().run(pulse)`: the pulse is constructed
    /// stopped and the manager runs [`JewelleryPulse::start`].
    fn run_pulse(&mut self, pulse: JewelleryPulse) -> Result<()>;
    /// `player.getInventory().remove(items)` with one of each id: whether
    /// every one was removed.
    fn remove_all(&mut self, ids: &[i32]) -> Result<bool>;
    /// `player.getInventory().add(new Item(id))`; the answer is not read.
    fn add(&mut self, id: i32) -> Result<()>;
    /// `player.getSkills().addExperience(Skills.CRAFTING, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `player.animate(new Animation(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))`.
    fn reset_animation(&mut self) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
}

fn name_of<H: Host>(h: &mut H, id: i32) -> Result<String> {
    h.item_name_lowercase(id)?.ok_or(Error::Null)
}

/// `JewelleryCraftPlugin.handle(event)`: `JewelleryCrafting.open(player)`.
pub fn handle<H: Host>(h: &mut H) -> Result<bool> {
    open(h)?;
    Ok(true)
}

/// `JewelleryCrafting.open(player)`.
pub fn open<H: Host>(h: &mut H) -> Result<()> {
    h.open_interface(INTERFACE)?;
    for (mould, child) in MOULD_CHILDREN {
        if h.contains(mould, 1)? {
            h.send_interface_config(INTERFACE, child, true)?;
        }
    }
    for data in &JEWELLERY {
        let mut length = 0;
        for &item in data.items {
            if h.contains(item, 1)? {
                length += 1;
            }
        }
        let mould = mould_for(data.name);
        if !h.contains(mould, 1)? {
            length -= 1;
        }
        if length == data.items.len() as i32 && h.level()? >= data.level {
            h.send_item_zoom(data.send_item, 170, INTERFACE, data.component)?;
            continue;
        }
        let name = name_of(h, data.send_item)?;
        if name.contains("amulet") && h.contains(mould, 1)? {
            for &item in data.items {
                if !h.contains(item, 1)? {
                    h.send_item_zoom(1685, 220, INTERFACE, data.component)?;
                    break;
                }
            }
        }
        if name.contains("ring") && !h.contains(RING_MOULD, 1)? {
            continue;
        }
        if name.contains("necklace") && !h.contains(NECKLACE_MOULD, 1)? {
            continue;
        }
        if data.name == "DRAGONSTONE_AMULET" && !h.contains(AMULET_MOULD, 1)? {
            continue;
        }
        if name.contains("amulet") && !h.contains(AMULET_MOULD, 1)? {
            continue;
        }
        if name.contains("bracelet") && !h.contains(BRACELET_MOULD, 1)? {
            continue;
        }
        let blank = if name.contains("ring") {
            1647
        } else if name.contains("necklace") {
            1666
        } else if name.contains("amulet") || name.contains("ammy") {
            1685
        } else if name.contains("bracelet") {
            11067
        } else {
            -1
        };
        h.send_item_zoom(blank, 1, INTERFACE, data.component)?;
    }
    Ok(())
}

/// The most of a constant that can be made: the gold bars for a GOLD
/// constant, else the smaller amount of its first two items.
fn most<H: Host>(h: &mut H, data: &Jewellery) -> Result<i32> {
    if data.name.contains("GOLD") {
        return h.amount(GOLD_BAR);
    }
    let first = h.amount(data.items[0])?;
    let second = h.amount(data.items[1])?;
    Ok(if first > second { second } else { first })
}

/// `JewelleryInterface.handle` for the opcode and the button; its answer
/// is always `true`.
pub fn button<H: Host>(h: &mut H, opcode: i32, button: i32) -> Result<bool> {
    let Some(data) = for_button(button) else {
        return Ok(true);
    };
    if h.level()? < data.level {
        let an = if is_plus_n(&name_of(h, data.send_item)?)? {
            "an"
        } else {
            "a"
        };
        let name = name_of(h, data.send_item)?;
        h.send_message(&format!(
            "You need a crafting level of {} to craft {an} {name}.",
            data.level
        ))?;
        return Ok(true);
    }
    let name = name_of(h, data.send_item)?;
    let mut flag = false;
    for (word, mould) in [
        ("ring", RING_MOULD),
        ("necklace", NECKLACE_MOULD),
        ("amulet", AMULET_MOULD),
        ("bracelet", BRACELET_MOULD),
    ] {
        if name.contains(word) && !h.contains(mould, 1)? {
            flag = true;
        }
    }
    if flag {
        h.send_message("You don't have the required mould to make this.")?;
        return Ok(true);
    }
    let amount = match opcode {
        OP_MAKE_ONE => 1,
        OP_MAKE_FIVE => 5,
        OP_MAKE_ALL => most(h, data)?,
        OP_MAKE_X => {
            h.send_input(data)?;
            return Ok(true);
        }
        _ => 0,
    };
    if !h.ring_unlocked()? && data.name == "SLAYER_RING" {
        h.send_messages(&[
            "You don't know how to make this. Talk to any Slayer master in order to learn the",
            "ability that creates Slayer rings.",
        ])?;
        return Ok(true);
    }
    make(h, data, amount)?;
    Ok(true)
}

/// The amount prompt's handler: `JewelleryCrafting.make(player, d, (int)
/// value)`.
pub fn input<H: Host>(h: &mut H, data: &'static Jewellery, value: &Input) -> Result<()> {
    match value {
        Input::Int(amount) => make(h, data, *amount),
        Input::Text(_) => Err(Error::Cast),
    }
}

/// `JewelleryCrafting.make(player, data, amount)`.
pub fn make<H: Host>(h: &mut H, data: &'static Jewellery, mut amount: i32) -> Result<()> {
    let most = most(h, data)?;
    if amount > most {
        amount = most;
    }
    let mut length = 0usize;
    for &item in data.items {
        if h.contains(item, amount)? {
            length += 1;
        }
    }
    if length != data.items.len() {
        h.send_message("You don't have the required items to make this item.")?;
        return Ok(());
    }
    if h.level()? < data.level {
        h.send_message(&format!(
            "You need a crafting level of {} to craft this.",
            data.level
        ))?;
        return Ok(());
    }
    h.close_interface()?;
    h.run_pulse(JewelleryPulse::new(data, amount))
}

/// A `JewelleryPulse`.
#[derive(Debug, Clone, PartialEq)]
pub struct JewelleryPulse {
    pub jewellery: &'static Jewellery,
    pub amount: i32,
    pub ticks: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl JewelleryPulse {
    pub fn new(jewellery: &'static Jewellery, amount: i32) -> Self {
        Self {
            jewellery,
            amount,
            ticks: 0,
            running: false,
        }
    }
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        Ok(h.level()? >= self.jewellery.level)
    }

    /// `SkillPulse.start()`.
    pub fn start<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if self.check_requirements(h)? {
            self.running = true;
        }
        Ok(())
    }

    /// `SkillPulse.pulse()`: whether the pulse is done.
    pub fn pulse<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if !self.check_requirements(h)? {
            return Ok(true);
        }
        if self.ticks % 5 == 0 {
            h.animate(ANIMATION)?;
            h.play_audio(SOUND)?;
        }
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % 5 != 0 {
            return Ok(false);
        }
        if h.remove_all(self.jewellery.items)? {
            h.add(self.jewellery.send_item)?;
            h.add_experience(self.jewellery.experience)?;
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount < 1)
    }

    /// `SkillPulse.stop()`: `resetAnimation` is true.
    pub fn stop<H: Host>(&mut self, h: &mut H) -> Result<()> {
        h.reset_animation()?;
        self.running = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_pinned_list() {
        assert_eq!(for_button(35).map(|j| j.send_item), Some(13281));
        assert_eq!(for_button(42).map(|j| j.name), Some("GOLD_NECKLACE"));
        assert_eq!(for_button(92).map(|j| j.component), Some(91));
        assert_eq!(for_button(21), None);
        assert_eq!(mould_for("SLAYER_RING"), RING_MOULD);
        assert_eq!(mould_for("ONYX_BRACELET"), BRACELET_MOULD);
        assert_eq!(is_plus_n("emerald ring"), Ok(true));
        assert_eq!(is_plus_n("gold ring"), Ok(false));
        assert_eq!(is_plus_n("h"), Ok(false));
    }
}
