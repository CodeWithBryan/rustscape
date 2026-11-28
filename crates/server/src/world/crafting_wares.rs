//! `content.global.skill.crafting`: battlestaves (`BattlestaffListener`
//! with its `BattlestaffProduct`), glassblowing
//! (`glass.GlassCraftingListener`, `glass.GlassCraftingPulse`,
//! `glass.GlassProduct`), molten glass at a furnace
//! (`glass.GlassMakeListener`, `glass.GlassMakePulse`) and spiky vambraces
//! (`leather.SpikyVambraces`).
//!
//! Differential corpus: `fixtures/crafting-wares` (oracle/crafting-wares),
//! replayed by `tests/crafting_wares.rs` over these functions with the
//! oracle's declared state. Every outside call is a [`Host`] method.
//!
//! Covered: the two tables, what the four classes register, the three
//! use-with lambdas ([`battlestaff_use`], [`glass_use`],
//! [`glass_make_use`]), the battlestaff dialogue's callbacks
//! ([`battlestaff_max`], [`battlestaff_create`]) and its task
//! ([`battlestaff_task`]), the glassblowing interface's lambda
//! ([`glass_button`]) with its amount prompt ([`glass_input`]), the molten
//! glass dialogue's callbacks ([`glass_make_max`], [`glass_make_create`]),
//! the two pulses ([`GlassPulse`], [`GlassMakePulse`]) and
//! `SpikyVambraces.handle` ([`vambraces_handle`]). Not covered: the rest of
//! crafting (docs/crafting-wares.md).

/// `Skills.CRAFTING`.
pub const CRAFTING: i32 = 12;
/// `Items.BATTLESTAFF_1391`.
pub const BATTLESTAFF: i32 = 1391;
/// `Items.AIR_BATTLESTAFF_1397`, whose making is the Varrock diary's task
/// 2, 6.
pub const AIR_BATTLESTAFF: i32 = 1397;
/// The glassblowing pipe, the molten glass and `Components.CRAFTING_GLASS_542`.
pub const GLASSBLOWING_PIPE: i32 = 1785;
pub const MOLTEN_GLASS: i32 = 1775;
pub const GLASS_INTERFACE: i32 = 542;
/// The interface's opcodes: make one, five, all and X.
pub const OP_MAKE_ONE: i32 = 155;
pub const OP_MAKE_FIVE: i32 = 196;
pub const OP_MAKE_ALL: i32 = 124;
pub const OP_MAKE_X: i32 = 199;
/// GlassCraftingPulse's animation and `Sounds.GLASSBLOWING_2724`.
pub const GLASS_ANIMATION: i32 = 884;
pub const GLASS_SOUND: i32 = 2724;
/// The soda ash, the bucket of sand and the empty bucket.
pub const SODA_ASH: i32 = 1781;
pub const BUCKET_OF_SAND: i32 = 1783;
pub const BUCKET: i32 = 1925;
/// GlassMakeListener's `INPUTS` and `FURNACES`.
pub const GLASS_MAKE_INPUTS: [i32; 2] = [SODA_ASH, BUCKET_OF_SAND];
pub const FURNACES: [i32; 13] = [
    4304, 6189, 9390, 11010, 11666, 12100, 12809, 18497, 26814, 30021, 30510, 36956, 37651,
];
/// GlassMakePulse's animation and experience.
pub const GLASS_MAKE_ANIMATION: i32 = 3243;
pub const GLASS_MAKE_EXPERIENCE: f64 = 20.0;
/// `Items.KEBBIT_CLAWS_10113`: SpikyVambraces' allowed node.
pub const KEBBIT_CLAWS: i32 = 10113;
/// The vambraces SpikyVambraces is added under, in `newInstance`'s order,
/// with the product and the word of the message.
pub const VAMBRACES: [(i32, i32, &str); 5] = [
    (1063, 10077, "leather"),
    (1065, 10079, "green dragonhide"),
    (2487, 10081, "blue dragonhide"),
    (2489, 10083, "red dragonhide"),
    (2491, 10085, "black dragonhide"),
];
pub const VAMBRACES_LEVEL: i32 = 32;
pub const VAMBRACES_EXPERIENCE: f64 = 6.0;

/// The classes (`scan.contentInterfaces`, `scan.initializable`).
pub const BATTLESTAFF_CLASS: &str = "content.global.skill.crafting.BattlestaffListener";
pub const GLASS_CLASS: &str = "content.global.skill.crafting.glass.GlassCraftingListener";
pub const GLASS_MAKE_CLASS: &str = "content.global.skill.crafting.glass.GlassMakeListener";
pub const VAMBRACES_CLASS: &str = "content.global.skill.crafting.leather.SpikyVambraces";
/// The lambdas' classes.
pub const BATTLESTAFF_LAMBDA: &str =
    "content.global.skill.crafting.BattlestaffListener$defineListeners$1";
pub const GLASS_LAMBDA: &str =
    "content.global.skill.crafting.glass.GlassCraftingListener$defineListeners$1";
pub const GLASS_BUTTON_LAMBDA: &str =
    "content.global.skill.crafting.glass.GlassCraftingListener$defineInterfaceListeners$1";
pub const GLASS_MAKE_LAMBDA: &str =
    "content.global.skill.crafting.glass.GlassMakeListener$defineListeners$1";
/// The pulses' classes.
pub const GLASS_PULSE_CLASS: &str = "content.global.skill.crafting.glass.GlassCraftingPulse";
pub const GLASS_MAKE_PULSE_CLASS: &str = "content.global.skill.crafting.glass.GlassMakePulse";

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

/// A `BattlestaffProduct` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Battlestaff {
    pub name: &'static str,
    pub orb: i32,
    pub product: i32,
    pub amount: i32,
    pub level: i32,
    pub experience: f64,
}
const fn staff(name: &'static str, orb: i32, product: i32, level: i32, xp: f64) -> Battlestaff {
    Battlestaff {
        name,
        orb,
        product,
        amount: 1,
        level,
        experience: xp,
    }
}
pub const BATTLESTAVES: [Battlestaff; 4] = [
    staff("WATER_BATTLESTAFF", 571, 1395, 54, 100.0),
    staff("EARTH_BATTLESTAFF", 575, 1399, 58, 112.5),
    staff("FIRE_BATTLESTAFF", 569, 1393, 62, 125.0),
    staff("AIR_BATTLESTAFF", 573, AIR_BATTLESTAFF, 66, 137.5),
];
/// BattlestaffListener's `orbs`.
pub const ORBS: [i32; 4] = [571, 575, 569, 573];
/// `BattlestaffProduct.productMap[id]`.
pub fn battlestaff_for_orb(id: i32) -> Option<&'static Battlestaff> {
    BATTLESTAVES.iter().find(|s| s.orb == id)
}

/// A `GlassProduct` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    pub name: &'static str,
    pub button: i32,
    pub product: i32,
    pub amount: i32,
    pub level: i32,
    pub experience: f64,
}
const fn glass(name: &'static str, button: i32, product: i32, level: i32, xp: f64) -> Glass {
    Glass {
        name,
        button,
        product,
        amount: 1,
        level,
        experience: xp,
    }
}
pub const GLASS: [Glass; 8] = [
    glass("VIAL", 38, 229, 33, 35.0),
    glass("ORB", 39, 567, 46, 52.5),
    glass("BEER_GLASS", 40, 1919, 1, 17.5),
    glass("CANDLE_LANTERN", 41, 4527, 4, 19.0),
    glass("OIL_LAMP", 42, 4525, 12, 25.0),
    glass("LANTERN_LENS", 43, 4542, 49, 55.0),
    glass("FISHBOWL", 44, 6667, 42, 42.5),
    glass("LIGHT_ORB", 45, 10973, 87, 70.0),
];
/// `GlassProduct.forButtonID(buttonId)`.
pub fn glass_for_button(button: i32) -> Option<&'static Glass> {
    GLASS.iter().find(|g| g.button == button)
}

/// What the bodies ask of the world, for one player.
pub trait Host {
    /// `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32>;
    /// `inInventory(player, id)`: `inventory.contains(id, 1)`.
    fn in_inventory(&mut self, id: i32) -> Result<bool>;
    /// `player.skills.getLevel(Skills.CRAFTING)`.
    fn level(&mut self) -> Result<i32>;
    /// `sendMessage(player, message)`.
    fn send_message(&mut self, message: &str) -> Result<()>;
    /// `removeItem(player, id)` from the inventory.
    fn remove_item(&mut self, id: i32) -> Result<bool>;
    /// `addItem(player, id, amount)`; the answer is not read.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `player.skills.addExperience(Skills.CRAFTING, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// `player.achievementDiaryManager.finishTask(player, DiaryType.VARROCK,
    /// 2, 6)`.
    fn finish_varrock_task(&mut self) -> Result<()>;
    /// `animate(player, id)`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `player.dispatch(ResourceProducedEvent(id, amount, player))`.
    fn resource_produced(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `openInterface(player, id)`.
    fn open_interface(&mut self, id: i32) -> Result<()>;
    /// `closeInterface(player)`.
    fn close_interface(&mut self) -> Result<()>;
    /// The battlestaff's `sendSkillDialogue` with the product's item: its
    /// callbacks are [`battlestaff_max`] and [`battlestaff_create`] with
    /// the two ids.
    fn open_battlestaff_dialogue(
        &mut self,
        staff: &'static Battlestaff,
        used: i32,
        with: i32,
    ) -> Result<()>;
    /// `runTask(player, 2, repeats) { ... }`: the task is
    /// [`battlestaff_task`] with the amount.
    fn run_battlestaff_task(
        &mut self,
        staff: &'static Battlestaff,
        repeats: i32,
        amount: i32,
    ) -> Result<()>;
    /// `sendInputDialogue(player, InputType.AMOUNT, "Enter the amount:")`:
    /// the handler is [`glass_input`].
    fn send_glass_input(&mut self, glass: &'static Glass) -> Result<()>;
    /// `submitIndividualPulse(player, GlassCraftingPulse(player, product,
    /// amount))`.
    fn submit_glass_pulse(&mut self, pulse: GlassPulse) -> Result<()>;
    /// The molten glass's `sendSkillDialogue` with the molten glass: its
    /// callbacks are [`glass_make_max`] and [`glass_make_create`].
    fn open_glass_make_dialogue(&mut self) -> Result<()>;
    /// `submitIndividualPulse(player, GlassMakePulse(player, id, amount))`.
    fn submit_glass_make_pulse(&mut self, pulse: GlassMakePulse) -> Result<()>;
}

fn make_staff<H: Host>(h: &mut H, staff: &Battlestaff) -> Result<()> {
    if h.remove_item(staff.orb)? && h.remove_item(BATTLESTAFF)? {
        h.add_item(staff.product, staff.amount)?;
        h.reward_xp(staff.experience)?;
    }
    if staff.product == AIR_BATTLESTAFF {
        h.finish_varrock_task()?;
    }
    Ok(())
}

/// BattlestaffListener's use-with lambda for the two items' ids; its
/// answer is always `true`.
pub fn battlestaff_use<H: Host>(h: &mut H, used: i32, with: i32) -> Result<bool> {
    let Some(staff) = battlestaff_for_orb(used) else {
        return Ok(true);
    };
    if h.level()? < staff.level {
        h.send_message(&format!(
            "You need a Crafting level of {} to make this.",
            staff.level
        ))?;
        return Ok(true);
    }
    if h.amount_in_inventory(used)? == 1 || h.amount_in_inventory(with)? == 1 {
        make_staff(h, staff)?;
        return Ok(true);
    }
    h.open_battlestaff_dialogue(staff, used, with)?;
    Ok(true)
}

/// The lambda's `getMaxAmount()`: the dialogue's `calculateMaxAmount`.
pub fn battlestaff_max<H: Host>(h: &mut H, used: i32, with: i32) -> Result<i32> {
    let with = h.amount_in_inventory(with)?;
    let used = h.amount_in_inventory(used)?;
    Ok(with.min(used))
}

/// The battlestaff dialogue's `create { _, amount -> ... }`.
pub fn battlestaff_create<H: Host>(
    h: &mut H,
    staff: &'static Battlestaff,
    used: i32,
    with: i32,
    amount: i32,
) -> Result<()> {
    let repeats = amount.min(battlestaff_max(h, used, with)?);
    h.run_battlestaff_task(staff, repeats, amount)
}

/// One run of the task `battlestaff_create` hands to `runTask`.
pub fn battlestaff_task<H: Host>(h: &mut H, staff: &Battlestaff, amount: i32) -> Result<()> {
    if amount < 1 {
        return Ok(());
    }
    make_staff(h, staff)
}

/// GlassCraftingListener's use-with lambda.
pub fn glass_use<H: Host>(h: &mut H) -> Result<bool> {
    h.open_interface(GLASS_INTERFACE)?;
    Ok(true)
}

fn glass_make<H: Host>(h: &mut H, glass: &'static Glass, amount: i32) -> Result<()> {
    h.close_interface()?;
    h.submit_glass_pulse(GlassPulse {
        glass,
        amount,
        delay: 1,
    })
}

/// GlassCraftingListener's `on(542)` lambda for the opcode and the
/// button; its answer is always `true`.
pub fn glass_button<H: Host>(h: &mut H, opcode: i32, button: i32) -> Result<bool> {
    let Some(glass) = glass_for_button(button) else {
        return Ok(true);
    };
    if !h.in_inventory(GLASSBLOWING_PIPE)? {
        h.send_message("You need a glassblowing pipe to do this.")?;
        return Ok(true);
    }
    if !h.in_inventory(MOLTEN_GLASS)? {
        h.send_message("You need molten glass to do this.")?;
        return Ok(true);
    }
    if h.level()? < glass.level {
        h.send_message(&format!(
            "You need a Crafting level of {} to make this.",
            glass.level
        ))?;
        return Ok(true);
    }
    match opcode {
        OP_MAKE_ONE => glass_make(h, glass, 1)?,
        OP_MAKE_FIVE => glass_make(h, glass, 5)?,
        OP_MAKE_ALL => {
            let amount = h.amount_in_inventory(MOLTEN_GLASS)?;
            glass_make(h, glass, amount)?;
        }
        OP_MAKE_X => h.send_glass_input(glass)?,
        _ => {}
    }
    Ok(true)
}

/// The amount prompt's handler: `make(player, product,
/// Integer.parseInt(value.toString()))` for an amount.
pub fn glass_input<H: Host>(h: &mut H, glass: &'static Glass, value: i32) -> Result<()> {
    glass_make(h, glass, value)
}

/// A `GlassCraftingPulse`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlassPulse {
    pub glass: &'static Glass,
    pub amount: i32,
    /// `Pulse.delay`.
    pub delay: i32,
}
impl GlassPulse {
    /// `pulse()`: whether the pulse is done.
    pub fn pulse<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if self.amount < 1 {
            return Ok(true);
        }
        if !h.in_inventory(GLASSBLOWING_PIPE)? || !h.in_inventory(MOLTEN_GLASS)? {
            return Ok(true);
        }
        h.animate(GLASS_ANIMATION)?;
        h.play_audio(GLASS_SOUND)?;
        let name = self.glass.name.to_lowercase().replace('_', " ");
        // The unpowered orb and the oil lamp.
        if self.glass.product == 567 || self.glass.product == 4525 {
            h.send_message(&format!("You make an {name}."))?;
        } else {
            h.send_message(&format!("You make a {name}."))?;
        }
        if h.remove_item(MOLTEN_GLASS)? {
            h.add_item(self.glass.product, self.glass.amount)?;
            h.reward_xp(self.glass.experience)?;
            h.resource_produced(self.glass.product, self.glass.amount)?;
        } else {
            return Ok(true);
        }
        self.amount = self.amount.wrapping_sub(1);
        self.delay = 3;
        Ok(false)
    }
}

/// GlassMakeListener's use-with lambda; its answer is always `true`.
pub fn glass_make_use<H: Host>(h: &mut H) -> Result<bool> {
    if !h.in_inventory(SODA_ASH)? || !h.in_inventory(BUCKET_OF_SAND)? {
        h.send_message(
            "You need at least one heap of soda ash and one bucket of sand to do this.",
        )?;
        return Ok(true);
    }
    h.open_glass_make_dialogue()?;
    Ok(true)
}

/// The molten glass dialogue's `calculateMaxAmount`.
pub fn glass_make_max<H: Host>(h: &mut H) -> Result<i32> {
    let ash = h.amount_in_inventory(SODA_ASH)?;
    let sand = h.amount_in_inventory(BUCKET_OF_SAND)?;
    Ok(ash.min(sand))
}

/// The molten glass dialogue's `create { id, amount -> ... }`.
pub fn glass_make_create<H: Host>(h: &mut H, id: i32, amount: i32) -> Result<()> {
    h.submit_glass_make_pulse(GlassMakePulse {
        product: id,
        amount,
        delay: 1,
    })
}

/// A `GlassMakePulse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlassMakePulse {
    pub product: i32,
    pub amount: i32,
    /// `Pulse.delay`.
    pub delay: i32,
}
impl GlassMakePulse {
    /// `pulse()`: whether the pulse is done.
    pub fn pulse<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if self.amount < 1 {
            return Ok(true);
        }
        if !h.in_inventory(SODA_ASH)? || !h.in_inventory(BUCKET_OF_SAND)? {
            return Ok(true);
        }
        h.animate(GLASS_MAKE_ANIMATION)?;
        h.send_message("You heat the sand and soda ash in the furnace to make glass.")?;
        if h.remove_item(SODA_ASH)? && h.remove_item(BUCKET_OF_SAND)? {
            h.add_item(BUCKET, 1)?;
            h.add_item(MOLTEN_GLASS, 1)?;
            h.reward_xp(GLASS_MAKE_EXPERIENCE)?;
            h.resource_produced(self.product, self.amount)?;
        } else {
            return Ok(true);
        }
        self.amount = self.amount.wrapping_sub(1);
        self.delay = 2;
        Ok(false)
    }
}

/// `SpikyVambraces.handle(event)` for the id of the node the item was
/// used with; its answer is always `true`.
pub fn vambraces_handle<H: Host>(h: &mut H, with: i32) -> Result<bool> {
    let Some(&(vamb, product, leather)) = VAMBRACES.iter().find(|v| v.0 == with) else {
        return Ok(true);
    };
    if h.level()? >= VAMBRACES_LEVEL {
        if h.remove_item(vamb)? && h.remove_item(KEBBIT_CLAWS)? {
            h.add_item(product, 1)?;
            h.reward_xp(VAMBRACES_EXPERIENCE)?;
            h.send_message(&format!(
                "You carefully attach the sharp claws to the {leather} vambraces."
            ))?;
        }
    } else {
        h.send_message("You need a crafting level of 32 to craft this.")?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_the_pinned_lists() {
        assert_eq!(battlestaff_for_orb(573).map(|s| s.product), Some(1397));
        assert_eq!(battlestaff_for_orb(BATTLESTAFF), None);
        assert_eq!(glass_for_button(38).map(|g| g.product), Some(229));
        assert_eq!(glass_for_button(37), None);
        assert_eq!(glass_for_button(46), None);
    }
}
