//! `content.global.skill.crafting`: cutting gems (`GemCutPlugin`,
//! `gem.GemCutPulse`, `gem.Gems`), chiselling limestone
//! (`ChiselLimestonePlugin`, `limestone.ChiselLimestonePulse`), stringing
//! amulets (`AmuletStringingPlugin` with `JewelleryItem.forProduct`) and
//! stringing silver symbols (`silver.SilverStringingListener` with
//! `SilverProduct.forProductID`).
//!
//! Differential corpus: `fixtures/crafting-tools` (oracle/crafting-tools),
//! replayed by `tests/crafting_tools.rs` over these functions with the
//! oracle's declared state. Every outside call is a [`Host`] method.
//!
//! Covered: `Gems`, `JewelleryItem`'s levels and products and
//! `SilverProduct`'s products and strung ids, what the three plugins and
//! the listener register, the three `UseWithHandler.handle` bodies
//! ([`gem_handle`], [`limestone_handle`], [`amulet_handle`]), the gem
//! dialogue's callbacks ([`gem_get_all`], [`gem_create`]), the two pulses
//! with `SkillPulse`'s `start`, `pulse` and `stop` ([`Pulse`]) and the
//! listener's lambda ([`string_silver`]). Not covered: the rest of
//! crafting (battlestaves and glass are `super::crafting_wares`; jewellery
//! and silver casting, leather, pottery, spinning, weaving, light sources,
//! snelms and snakeskin are not ported).

/// `Skills.CRAFTING`.
pub const CRAFTING: i32 = 12;
/// Both pulses' `CHISEL` and the key of GemCutPlugin's and
/// ChiselLimestonePlugin's `addHandler`.
pub const CHISEL: i32 = 1755;
/// The ball of wool: the key of AmuletStringingPlugin's `addHandler` and
/// the used item of SilverStringingListener's registration.
pub const BALL_OF_WOOL: i32 = 1759;
/// `Sounds.CHISEL_2586`.
pub const CHISEL_SOUND: i32 = 2586;
/// The limestone, the brick it becomes, its level, its experience and
/// `Animation.create(4470)`.
pub const LIMESTONE: i32 = 3211;
pub const LIMESTONE_BRICK: i32 = 3420;
pub const LIMESTONE_LEVEL: i32 = 12;
pub const LIMESTONE_EXPERIENCE: f64 = 6.0;
pub const LIMESTONE_ANIMATION: i32 = 4470;
/// The onyx amulet (unstrung) and the amulet it is strung into.
pub const ONYX_AMULET: i32 = 6579;
pub const ONYX_AMULET_STRUNG: i32 = 6581;

/// The classes (`scan.initializable`, `scan.contentInterfaces`).
pub const GEM_CLASS: &str = "content.global.skill.crafting.GemCutPlugin";
pub const LIMESTONE_CLASS: &str = "content.global.skill.crafting.ChiselLimestonePlugin";
pub const AMULET_CLASS: &str = "content.global.skill.crafting.AmuletStringingPlugin";
pub const SILVER_CLASS: &str = "content.global.skill.crafting.silver.SilverStringingListener";
/// SilverStringingListener's `::stringSilverProduct` reference.
pub const SILVER_LAMBDA: &str =
    "content.global.skill.crafting.silver.SilverStringingListener$defineListeners$1";
/// The pulses' classes.
pub const GEM_PULSE_CLASS: &str = "content.global.skill.crafting.gem.GemCutPulse";
pub const LIMESTONE_PULSE_CLASS: &str =
    "content.global.skill.crafting.limestone.ChiselLimestonePulse";

/// The constructors' allowed nodes, as given (GemCutPlugin names the onyx
/// twice).
pub const GEM_ALLOWED: [i32; 10] = [1623, 1621, 1619, 1617, 1631, 6571, 1625, 1627, 1629, 6571];
pub const LIMESTONE_ALLOWED: [i32; 1] = [LIMESTONE];
pub const AMULET_ALLOWED: [i32; 7] = [1673, 1675, 1677, 1679, 1681, 1683, ONYX_AMULET];
/// The products SilverStringingListener registers the wool with.
pub const STRINGABLE: [i32; 2] = [1714, 1720];

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

/// A `Gems` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gem {
    pub name: &'static str,
    pub uncut: i32,
    pub gem: i32,
    pub level: i32,
    pub animation: i32,
    pub experience: f64,
}
const fn gem(
    name: &'static str,
    uncut: i32,
    gem: i32,
    level: i32,
    animation: i32,
    experience: f64,
) -> Gem {
    Gem {
        name,
        uncut,
        gem,
        level,
        animation,
        experience,
    }
}
/// `Gems.values()`.
pub const GEMS: [Gem; 9] = [
    gem("SAPPHIRE", 1623, 1607, 20, 888, 50.0),
    gem("EMERALD", 1621, 1605, 27, 889, 67.0),
    gem("RUBY", 1619, 1603, 34, 887, 85.0),
    gem("DIAMOND", 1617, 1601, 43, 886, 107.5),
    gem("DRAGONSTONE", 1631, 1615, 55, 885, 137.5),
    gem("ONYX", 6571, 6573, 67, 2717, 168.0),
    gem("OPAL", 1625, 1609, 1, 890, 10.0),
    gem("JADE", 1627, 1611, 13, 891, 20.0),
    gem("RED_TOPAZ", 1629, 1613, 16, 892, 25.0),
];
/// `Gems.forId(item)`.
pub fn gem_for_id(id: i32) -> Option<&'static Gem> {
    GEMS.iter().find(|g| g.uncut == id)
}

/// A `JewelleryCrafting.JewelleryItem` constant: its name, level and
/// `sendItem`.
pub const JEWELLERY: [(&str, i32, i32); 29] = [
    ("GOLD_RING", 5, 1635),
    ("SAPPIRE_RING", 20, 1637),
    ("EMERALD_RING", 27, 1639),
    ("RUBY_RING", 34, 1641),
    ("DIAMOND_RING", 43, 1643),
    ("DRAGONSTONE_RING", 55, 1645),
    ("ONYX_RING", 67, 6575),
    ("GOLD_NECKLACE", 6, 1654),
    ("SAPPHIRE_NECKLACE", 22, 1656),
    ("EMERALD_NECKLACE", 29, 1658),
    ("RUBY_NECKLACE", 40, 1660),
    ("DIAMOND_NECKLACE", 56, 1662),
    ("DRAGONSTONE_NECKLACE", 72, 1664),
    ("ONYX_NECKLACE", 82, 6577),
    ("SLAYER_RING", 75, 13281),
    ("GOLD_AMULET", 8, 1673),
    ("SAPPHIRE_AMULET", 24, 1675),
    ("EMERALD_AMULET", 31, 1677),
    ("RUBY_AMULET", 50, 1679),
    ("DIAMOND_AMULET", 70, 1681),
    ("DRAGONSTONE_AMULET", 80, 1683),
    ("ONYX_AMULET", 90, 6579),
    ("GOLD_BRACELET", 7, 11069),
    ("SAPPHIRE_BRACELET", 23, 11072),
    ("EMERALD_BRACELET", 30, 11076),
    ("RUBY_BRACELET", 42, 11085),
    ("DIAMOND_BRACELET", 58, 11092),
    ("DRAGONSTONE_BRACELET", 74, 11115),
    ("ONYX_BRACELET", 84, 11130),
];
/// `JewelleryItem.forProduct(id)`: `productMap` keeps the first constant of
/// a `sendItem`. Its level and `sendItem`.
pub fn jewellery_for_product(id: i32) -> Option<(i32, i32)> {
    JEWELLERY
        .iter()
        .find(|(_, _, send)| *send == id)
        .map(|(_, level, send)| (*level, *send))
}

/// A `SilverProduct` constant: its name, `producedItemId` and `strungId`.
pub const SILVER: [(&str, i32, i32); 8] = [
    ("HOLY", 1714, 1716),
    ("UNHOLY", 1720, 1722),
    ("SICKLE", 2961, -1),
    ("TIARA", 5525, -1),
    ("SILVTHRIL_CHAIN", 13154, -1),
    ("LIGHTNING_ROD", 4201, -1),
    ("SILVTHRILL_ROD", 7637, -1),
    ("CROSSBOW_BOLTS", 9382, -1),
];
/// `SilverProduct.forProductID(id)`: its `strungId`. `PRODUCT_MAP` keeps
/// the last constant of a product, and no two share one.
pub fn silver_strung(id: i32) -> Option<i32> {
    SILVER
        .iter()
        .rev()
        .find(|(_, product, _)| *product == id)
        .map(|(_, _, strung)| *strung)
}

/// What the plugins, the pulses and the listener ask of the world, for
/// their one player.
pub trait Host {
    /// `player.getInventory().getAmount(Item(id))`.
    fn get_amount(&mut self, id: i32) -> Result<i32>;
    /// `player.getInventory().containsItem(Item(id))`.
    fn contains_item(&mut self, id: i32) -> Result<bool>;
    /// `player.getInventory().remove(Item(id))`.
    fn remove(&mut self, id: i32) -> Result<bool>;
    /// `player.getInventory().remove(event.getUsedItem(),
    /// event.getBaseItem())`: the two items of the event.
    fn remove_event_items(&mut self) -> Result<bool>;
    /// `player.getInventory().add(Item(id))`; the answer is not read.
    fn add(&mut self, id: i32) -> Result<()>;
    /// `player.getSkills().getLevel(Skills.CRAFTING)`.
    fn level(&mut self) -> Result<i32>;
    /// `player.getPacketDispatch().sendMessage(message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// `player.getSkills().addExperience(Skills.CRAFTING, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `player.animate(animation)` of an animation made with its id alone
    /// (`new Animation(id)` and `Animation.create(id)` are the same value).
    fn animate(&mut self, id: i32) -> Result<()>;
    /// ContentAPI `removeItem(player, id)`.
    fn remove_item(&mut self, id: i32) -> Result<bool>;
    /// ContentAPI `addItem(player, id)`; the answer is not read.
    fn add_item(&mut self, id: i32) -> Result<()>;
    /// `SkillDialogueHandler(player, ONE_OPTION, gem.getGem()).open()`: the
    /// dialogue whose callbacks are [`gem_get_all`] and [`gem_create`].
    fn open_dialogue(&mut self, gem: &'static Gem) -> Result<()>;
    /// `player.getPulseManager().run(pulse)` of a pulse just constructed.
    fn run_pulse(&mut self, pulse: Pulse) -> Result<()>;
}

/// `GemCutPlugin.handle(event)` for the used and the base item's ids.
pub fn gem_handle<H: Host>(h: &mut H, used: i32, base: i32) -> Result<bool> {
    // `gem.getGem()` of a null constant.
    let gem = gem_for_id(if used == CHISEL { base } else { used }).ok_or(Error::Null)?;
    if h.get_amount(gem.uncut)? == 1 {
        // `handler.create(0, 1)`.
        gem_create(h, gem, 0)?;
    } else {
        h.open_dialogue(gem)?;
    }
    Ok(true)
}

/// The gem dialogue's `getAll(index)`; it does not read its index.
pub fn gem_get_all<H: Host>(h: &mut H, gem: &'static Gem) -> Result<i32> {
    h.get_amount(gem.uncut)
}

/// The gem dialogue's `create(amount, index)`; it does not read its index.
pub fn gem_create<H: Host>(h: &mut H, gem: &'static Gem, amount: i32) -> Result<()> {
    h.run_pulse(Pulse::Gem(GemPulse {
        gem,
        amount,
        running: false,
    }))
}

/// `ChiselLimestonePlugin.handle(event)`.
pub fn limestone_handle<H: Host>(h: &mut H) -> Result<bool> {
    h.run_pulse(Pulse::Limestone(LimestonePulse {
        ticks: 0,
        running: false,
    }))?;
    Ok(true)
}

/// `AmuletStringingPlugin.handle(event)` for the used item's id and the
/// id of the node it was used with.
pub fn amulet_handle<H: Host>(h: &mut H, used: i32, with: i32) -> Result<bool> {
    let product = if used == ONYX_AMULET {
        ONYX_AMULET
    } else {
        with
    };
    let Some((level, send)) = jewellery_for_product(product) else {
        return Ok(true);
    };
    if h.level()? < level {
        h.message(&format!(
            "You need a Crafting level of at least {level} to do that."
        ))?;
        return Ok(true);
    }
    if h.remove_event_items()? {
        h.add(if send == ONYX_AMULET {
            ONYX_AMULET_STRUNG
        } else {
            send.wrapping_add(19)
        })?;
    }
    Ok(true)
}

/// SilverStringingListener's `stringSilverProduct(player, used, with)` for
/// the two items' ids; its answer is always `true`.
pub fn string_silver<H: Host>(h: &mut H, used: i32, with: i32) -> Result<bool> {
    if let Some(strung) = silver_strung(with)
        && h.remove_item(with)?
        && h.remove_item(used)?
    {
        h.add_item(strung)?;
    }
    Ok(true)
}

/// A `GemCutPulse`; its `ticks` field is never written.
#[derive(Debug, Clone, PartialEq)]
pub struct GemPulse {
    pub gem: &'static Gem,
    pub amount: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl GemPulse {
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if !h.contains_item(CHISEL)? {
            return Ok(false);
        }
        if !h.contains_item(self.gem.uncut)? {
            return Ok(false);
        }
        if h.level()? < self.gem.level {
            h.message(&format!(
                "You need a crafting level of {} to craft this gem.",
                self.gem.level
            ))?;
            return Ok(false);
        }
        Ok(true)
    }
    fn reward<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if h.remove(self.gem.uncut)? {
            h.add(self.gem.gem)?;
            h.add_experience(self.gem.experience)?;
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount < 1)
    }
}

/// A `ChiselLimestonePulse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimestonePulse {
    pub ticks: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl LimestonePulse {
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if !h.contains_item(CHISEL)? {
            return Ok(false);
        }
        if !h.contains_item(LIMESTONE)? {
            return Ok(false);
        }
        if h.level()? < LIMESTONE_LEVEL {
            h.message("You need a crafting level of 12 to make limestone bricks.")?;
            return Ok(false);
        }
        Ok(true)
    }
    fn reward<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % 2 != 0 {
            return Ok(false);
        }
        if h.remove(LIMESTONE)? {
            h.add(LIMESTONE_BRICK)?;
            h.add_experience(LIMESTONE_EXPERIENCE)?;
        }
        Ok(!h.contains_item(LIMESTONE)?)
    }
}

/// One of the two pulses, as a pulse manager holds it. Both keep the
/// delay of 1 and neither resets the animation when it stops.
#[derive(Debug, Clone, PartialEq)]
pub enum Pulse {
    Gem(GemPulse),
    Limestone(LimestonePulse),
}
impl Pulse {
    /// The pulse's class.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Gem(_) => GEM_PULSE_CLASS,
            Self::Limestone(_) => LIMESTONE_PULSE_CLASS,
        }
    }
    /// `Pulse.running`.
    pub fn running(&self) -> bool {
        match self {
            Self::Gem(p) => p.running,
            Self::Limestone(p) => p.running,
        }
    }
    fn set_running(&mut self, running: bool) {
        match self {
            Self::Gem(p) => p.running = running,
            Self::Limestone(p) => p.running = running,
        }
    }
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        match self {
            Self::Gem(p) => p.check_requirements(h),
            Self::Limestone(p) => p.check_requirements(h),
        }
    }

    /// `SkillPulse.start()`.
    pub fn start<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if self.check_requirements(h)? {
            self.set_running(true);
        }
        Ok(())
    }

    /// `SkillPulse.pulse()`: whether the pulse is done.
    pub fn pulse<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if !self.check_requirements(h)? {
            return Ok(true);
        }
        // animate(): `ticks % 5 == 0 || ticks < 1`.
        let (ticks, animation) = match self {
            Self::Gem(p) => (0, p.gem.animation),
            Self::Limestone(p) => (p.ticks, LIMESTONE_ANIMATION),
        };
        if ticks % 5 == 0 || ticks < 1 {
            h.play_audio(CHISEL_SOUND)?;
            h.animate(animation)?;
        }
        match self {
            Self::Gem(p) => p.reward(h),
            Self::Limestone(p) => p.reward(h),
        }
    }

    /// `SkillPulse.stop()` with `resetAnimation` false.
    pub fn stop(&mut self) {
        self.set_running(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_the_pinned_lists() {
        assert_eq!(gem_for_id(1623).map(|g| g.gem), Some(1607));
        assert_eq!(gem_for_id(CHISEL), None);
        assert_eq!(jewellery_for_product(1673), Some((8, 1673)));
        assert_eq!(jewellery_for_product(BALL_OF_WOOL), None);
        assert_eq!(jewellery_for_product(0), None);
        assert_eq!(silver_strung(1714), Some(1716));
        assert_eq!(silver_strung(2961), Some(-1));
        assert_eq!(silver_strung(1716), None);
    }
}
