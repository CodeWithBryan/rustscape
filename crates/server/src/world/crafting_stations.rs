//! `content.global.skill.crafting` at a spinning wheel and a furnace:
//! spinning (`content.global.handlers.scenery.SpinSpinningWheelPlugin`,
//! `content.global.handlers.iface.SpinningInterface`,
//! `spinning.SpinningItem`, `spinning.SpinningPulse`) and silver casting
//! (`silver.SilverCraftingListener`, `silver.SilverCraftingPulse`,
//! `silver.SilverProduct`).
//!
//! Differential corpus: `fixtures/crafting-stations`
//! (oracle/crafting-stations), replayed by `tests/crafting_stations.rs`
//! over these functions with the oracle's declared state. Every outside
//! call is a [`Host`] method.
//!
//! Covered: the two tables, what the classes register, the wheel's option
//! ([`wheel_handle`]), SpinningInterface's `handle` ([`spin_button`]) with
//! its amount prompt ([`spin_input`]), [`SpinPulse`] with `SkillPulse`'s
//! `start`, `pulse` and `stop`, the silver listener's use-with, open and
//! button lambdas ([`silver_use`], [`silver_open`], [`silver_button`])
//! with its amount prompt ([`silver_input`]) and [`SilverPulse`]. Not
//! covered: the rest of crafting (docs/crafting-stations.md).

/// `Skills.CRAFTING`.
pub const CRAFTING: i32 = 12;
/// The interface's opcodes: make one, five, all and X.
pub const OP_MAKE_ONE: i32 = 155;
pub const OP_MAKE_FIVE: i32 = 196;
pub const OP_MAKE_ALL: i32 = 124;
pub const OP_MAKE_X: i32 = 199;

/// The spinning interface and the wheels SpinSpinningWheelPlugin puts
/// "option:spin" on, in `newInstance`'s order.
pub const SPIN_INTERFACE: i32 = 459;
pub const WHEELS: [i32; 10] = [
    2644, 4309, 8748, 20365, 21304, 25824, 26143, 34497, 36970, 37476,
];
/// SpinningPulse's animation and `Sounds.SPINNING_2590`.
pub const SPIN_ANIMATION: i32 = 896;
pub const SPIN_SOUND: i32 = 2590;
/// The Seers' Village spinning wheel's tile, the Seers' headband and the
/// region of the bowstring task.
pub const SEERS_WHEEL: [i32; 3] = [2711, 3471, 1];
pub const SEERS_HEADBAND: i32 = 14631;
pub const SEERS_REGION: i32 = 10806;
/// The bowstring task's counter, as read and as written.
pub const BOWSTRINGS: &str = "diary:seers:bowstrings-spun";
pub const BOWSTRINGS_SAVED: &str = "/save:diary:seers:bowstrings-spun";

/// The silver casting interface, the silver bar and the furnaces.
pub const SILVER_INTERFACE: i32 = 438;
pub const SILVER_BAR: i32 = 2355;
pub const SILVER_FURNACES: [i32; 23] = [
    2966, 3044, 3294, 4304, 6189, 11009, 11010, 11666, 12100, 12809, 18497, 18525, 18526, 21879,
    22721, 26814, 28433, 28434, 30021, 30510, 36956, 37651, 21303,
];
/// The furnace attribute of the use-with lambda.
pub const FURNACE_ATTRIBUTE: &str = "crafting:silver:furnace_id";
/// `onOpen(438)`'s `sendItemOnInterface` calls: the child and the item.
pub const SILVER_MODELS: [(i32, i32); 9] = [
    (17, 1718),
    (24, 1724),
    (31, 2961),
    (38, 4201),
    (45, 5525),
    (53, 7637),
    (60, 6748),
    (67, 9382),
    (74, 13154),
];
/// SilverCraftingPulse's animation and `Sounds.FURNACE_2725`.
pub const SILVER_ANIMATION: i32 = 3243;
pub const SILVER_SOUND: i32 = 2725;

/// The classes (`scan.initializable`, `scan.contentInterfaces`).
pub const WHEEL_CLASS: &str = "content.global.handlers.scenery.SpinSpinningWheelPlugin";
pub const SPIN_CLASS: &str = "content.global.handlers.iface.SpinningInterface";
pub const SILVER_CLASS: &str = "content.global.skill.crafting.silver.SilverCraftingListener";
/// The silver listener's lambdas.
pub const SILVER_OPEN_LAMBDA: &str =
    "content.global.skill.crafting.silver.SilverCraftingListener$defineInterfaceListeners$1";
pub const SILVER_BUTTON_LAMBDA: &str =
    "content.global.skill.crafting.silver.SilverCraftingListener$defineInterfaceListeners$2";
pub const SILVER_USE_LAMBDA: &str =
    "content.global.skill.crafting.silver.SilverCraftingListener$defineListeners$1";
/// The pulses' classes.
pub const SPIN_PULSE_CLASS: &str = "content.global.skill.crafting.spinning.SpinningPulse";
pub const SILVER_PULSE_CLASS: &str = "content.global.skill.crafting.silver.SilverCraftingPulse";

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// A ClassCastException.
    Cast,
    /// A NumberFormatException.
    Number,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A `SpinningItem` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spin {
    pub name: &'static str,
    pub button: i32,
    pub need: i32,
    pub product: i32,
    pub level: i32,
    pub experience: f64,
}
const fn spin(
    name: &'static str,
    button: i32,
    need: i32,
    product: i32,
    level: i32,
    experience: f64,
) -> Spin {
    Spin {
        name,
        button,
        need,
        product,
        level,
        experience,
    }
}
pub const SPINNING: [Spin; 11] = [
    spin("WOOL", 19, 1737, 1759, 1, 2.5),
    spin("FLAX", 17, 1779, 1777, 10, 15.0),
    spin("ROOT", 23, 6051, 6038, 19, 30.0),
    spin("ROOT1", 23, 6043, 6038, 19, 30.0),
    spin("ROOT2", 23, 6045, 6038, 19, 30.0),
    spin("ROOT3", 23, 6047, 6038, 19, 30.0),
    spin("ROOT4", 23, 6049, 6038, 19, 30.0),
    spin("ROOT5", 23, 6053, 6038, 19, 30.0),
    spin("SINEW", 27, 9436, 9438, 10, 15.0),
    spin("TREE_ROOTS", 31, 6043, 9438, 10, 15.0),
    spin("YACK", 35, 10814, 954, 30, 25.0),
];
/// `SpinningItem.forId(button)`: the first constant of the button.
pub fn spin_for_button(button: i32) -> Option<&'static Spin> {
    SPINNING.iter().find(|s| s.button == button)
}

/// A `SilverProduct` constant, without its strung id
/// (`super::crafting_tools::SILVER`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Silver {
    pub name: &'static str,
    pub button: i32,
    pub required: i32,
    pub product: i32,
    pub amount: i32,
    pub level: i32,
    pub experience: f64,
}
const fn silver(
    name: &'static str,
    button: i32,
    required: i32,
    product: i32,
    amount: i32,
    level: i32,
    experience: f64,
) -> Silver {
    Silver {
        name,
        button,
        required,
        product,
        amount,
        level,
        experience,
    }
}
pub const SILVER: [Silver; 8] = [
    silver("HOLY", 16, 1599, 1714, 1, 16, 50.0),
    silver("UNHOLY", 23, 1594, 1720, 1, 17, 50.0),
    silver("SICKLE", 30, 2976, 2961, 1, 18, 50.0),
    silver("TIARA", 44, 5523, 5525, 1, 23, 52.5),
    silver("SILVTHRIL_CHAIN", 73, 13153, 13154, 1, 47, 100.0),
    silver("LIGHTNING_ROD", 37, 4200, 4201, 1, 20, 50.0),
    silver("SILVTHRILL_ROD", 52, 7649, 7637, 1, 25, 55.0),
    silver("CROSSBOW_BOLTS", 66, 9434, 9382, 10, 21, 50.0),
];
/// `SilverProduct.forButtonID(buttonId)`.
pub fn silver_for_button(button: i32) -> Option<&'static Silver> {
    SILVER.iter().find(|s| s.button == button)
}

/// What an amount prompt was answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Int(i32),
    Text(String),
}
/// `Integer.parseInt(text)` for the decimal digits of ASCII.
fn parse_int(text: &str) -> Result<i32> {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Number);
    }
    text.parse::<i32>().map_err(|_| Error::Number)
}

/// What `onOpen(438)` asks of the world, for one player.
pub trait Models {
    /// `sendItemOnInterface(player, interface, child, item)`.
    fn send_item_on_interface(&mut self, interface: i32, child: i32, item: i32) -> Result<()>;
}

/// What the other bodies ask of the world, for one player.
pub trait Host: Models {
    /// The furnace a silver bar was used with.
    type Furnace: Clone;

    /// `player.getInterfaceManager().open(new Component(id))` and
    /// `openInterface(player, id)`.
    fn open_interface(&mut self, id: i32) -> Result<()>;
    /// `player.getInterfaceManager().close()` and `closeInterface(player)`.
    fn close_interface(&mut self) -> Result<()>;
    /// `player.getInventory().contains(id, 1)` and `inInventory(player,
    /// id)`.
    fn contains(&mut self, id: i32) -> Result<bool>;
    /// `player.getInventory().getAmount(new Item(id))` and
    /// `amountInInventory(player, id)`.
    fn amount(&mut self, id: i32) -> Result<i32>;
    /// `ItemDefinition.forId(id).getName()`.
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `player.getPacketDispatch().sendMessage(message)` and
    /// `sendMessage(player, message)`.
    fn send_message(&mut self, message: &str) -> Result<()>;
    /// `player.getSkills().getLevel(Skills.CRAFTING)`.
    fn level(&mut self) -> Result<i32>;
    /// `player.animate(new Animation(id))` and `animate(player, id)`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))`.
    fn reset_animation(&mut self) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `getDiary(DiaryType.SEERS_VILLAGE).isComplete(2)`.
    fn seers_hard_complete(&mut self) -> Result<bool>;
    /// `player.getLocation().withinDistance(Location.create(2711, 3471,
    /// 1))`.
    fn near_seers_wheel(&mut self) -> Result<bool>;
    /// `player.getEquipment().get(EquipmentContainer.SLOT_HAT)`: its id.
    fn hat(&mut self) -> Result<Option<i32>>;
    /// `player.getInventory().remove(new Item(id, 1))` and
    /// `removeItem(player, id, Container.INVENTORY)`.
    fn remove(&mut self, id: i32) -> Result<bool>;
    /// `player.getInventory().add(new Item(id, 1))`; the answer is not
    /// read.
    fn add(&mut self, id: i32) -> Result<()>;
    /// `addItem(player, id, amount)`.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `player.getSkills().addExperience(Skills.CRAFTING, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `rewardXP(player, Skills.CRAFTING, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// `player.getLocation().getRegionId()`.
    fn region_id(&mut self) -> Result<i32>;
    /// `getDiary(DiaryType.SEERS_VILLAGE).isComplete(0, 4)`.
    fn bowstring_task_complete(&mut self) -> Result<bool>;
    /// `player.getAttribute("diary:seers:bowstrings-spun", 0)`.
    fn bowstrings(&mut self) -> Result<i32>;
    /// `player.setAttribute("/save:diary:seers:bowstrings-spun", value)`.
    fn set_bowstrings(&mut self, value: i32) -> Result<()>;
    /// `finishTask(player, DiaryType.SEERS_VILLAGE, 0, 4)`.
    fn finish_bowstring_task(&mut self) -> Result<()>;
    /// `sendInputDialogue(p, true, "Enter the amount:", handler)`: the
    /// handler is [`spin_input`].
    fn send_spin_input(&mut self, spin: &'static Spin) -> Result<()>;
    /// `p.getPulseManager().run(pulse)` and `submitIndividualPulse(p,
    /// pulse)` of a SpinningPulse: the pulse is constructed stopped and the
    /// manager runs [`SpinPulse::start`].
    fn run_spin_pulse(&mut self, pulse: SpinPulse) -> Result<()>;
    /// `setAttribute(player, "crafting:silver:furnace_id", with)`.
    fn set_furnace(&mut self, furnace: &Self::Furnace) -> Result<()>;
    /// `getAttribute(player, "crafting:silver:furnace_id", Scenery(-1, -1,
    /// 0))`: `None` for the default.
    fn furnace(&mut self) -> Result<Option<Self::Furnace>>;
    /// `sendInputDialogue(player, InputType.AMOUNT, "Enter the amount:")`:
    /// the handler is [`silver_input`].
    fn send_silver_input(&mut self, silver: &'static Silver) -> Result<()>;
    /// `submitIndividualPulse(player, SilverCraftingPulse(...))`.
    fn submit_silver_pulse(&mut self, pulse: SilverPulse<Self::Furnace>) -> Result<()>;
    /// `player.dispatch(ResourceProducedEvent(id, amount, furnace,
    /// Items.SILVER_BAR_2355))`.
    fn resource_produced(
        &mut self,
        id: i32,
        amount: i32,
        furnace: &Option<Self::Furnace>,
    ) -> Result<()>;
}

/// `SpinSpinningWheelPlugin.handle(player, node, option)`.
pub fn wheel_handle<H: Host>(h: &mut H) -> Result<bool> {
    h.open_interface(SPIN_INTERFACE)?;
    Ok(true)
}

/// `SpinningInterface.handle` for the opcode and the button; its answer
/// is always `true`.
pub fn spin_button<H: Host>(h: &mut H, opcode: i32, button: i32) -> Result<bool> {
    let Some(spin) = spin_for_button(button) else {
        return Ok(true);
    };
    if !h.contains(spin.need)? {
        let name = h.item_name(spin.need)?.to_lowercase();
        h.send_message(&format!("You need {name} to make this."))?;
        return Ok(true);
    }
    let amount = match opcode {
        OP_MAKE_ONE => 1,
        OP_MAKE_FIVE => 5,
        OP_MAKE_ALL => h.amount(spin.need)?,
        OP_MAKE_X => {
            h.send_spin_input(spin)?;
            return Ok(true);
        }
        // Any other opcode runs the pulse with the amount -1.
        _ => -1,
    };
    h.run_spin_pulse(SpinPulse::new(spin, amount))?;
    Ok(true)
}

/// The spinning amount prompt's handler.
pub fn spin_input<H: Host>(h: &mut H, spin: &'static Spin, value: &Input) -> Result<()> {
    let amount = match value {
        Input::Text(text) => parse_int(text)?,
        Input::Int(n) => *n,
    };
    h.run_spin_pulse(SpinPulse::new(spin, amount))
}

/// A `SpinningPulse`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpinPulse {
    pub spin: &'static Spin,
    pub amount: i32,
    pub ticks: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl SpinPulse {
    pub fn new(spin: &'static Spin, amount: i32) -> Self {
        Self {
            spin,
            amount,
            ticks: 0,
            running: false,
        }
    }
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        h.close_interface()?;
        if h.level()? < self.spin.level {
            h.send_message(&format!(
                "You need a crafting level of {} to make this.",
                self.spin.level
            ))?;
            return Ok(false);
        }
        if !h.contains(self.spin.need)? {
            let name = h.item_name(self.spin.need)?;
            h.send_message(&format!("You need {name} to do this."))?;
            return Ok(false);
        }
        Ok(true)
    }
    fn reward<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        let mut threshold = 4;
        if h.seers_hard_complete()? && h.near_seers_wheel()? && h.hat()? == Some(SEERS_HEADBAND) {
            threshold = 2;
        }
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % threshold != 0 {
            return Ok(false);
        }
        if h.remove(self.spin.need)? {
            h.add(self.spin.product)?;
            h.add_experience(self.spin.experience)?;
            if h.region_id()? == SEERS_REGION && !h.bowstring_task_complete()? {
                if h.bowstrings()? >= 4 {
                    h.set_bowstrings(5)?;
                    h.finish_bowstring_task()?;
                } else {
                    let spun = h.bowstrings()?;
                    h.set_bowstrings(spun.wrapping_add(1))?;
                }
            }
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount < 1)
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
            h.animate(SPIN_ANIMATION)?;
            h.play_audio(SPIN_SOUND)?;
        }
        self.reward(h)
    }

    /// `SkillPulse.stop()`: `resetAnimation` is true.
    pub fn stop<H: Host>(&mut self, h: &mut H) -> Result<()> {
        h.reset_animation()?;
        self.running = false;
        Ok(())
    }
}

/// SilverCraftingListener's use-with lambda for the furnace.
pub fn silver_use<H: Host>(h: &mut H, furnace: &H::Furnace) -> Result<bool> {
    h.set_furnace(furnace)?;
    h.open_interface(SILVER_INTERFACE)?;
    Ok(true)
}

/// The `onOpen(438)` lambda.
pub fn silver_open<H: Models>(h: &mut H) -> Result<bool> {
    for (child, item) in SILVER_MODELS {
        h.send_item_on_interface(SILVER_INTERFACE, child, item)?;
    }
    Ok(true)
}

fn silver_make<H: Host>(h: &mut H, silver: &'static Silver, amount: i32) -> Result<()> {
    h.close_interface()?;
    let furnace = h.furnace()?;
    h.submit_silver_pulse(SilverPulse {
        silver,
        furnace,
        amount,
        delay: 1,
    })
}

/// The `on(438)` lambda for the opcode and the button; its answer is
/// always `true`.
pub fn silver_button<H: Host>(h: &mut H, opcode: i32, button: i32) -> Result<bool> {
    let Some(silver) = silver_for_button(button) else {
        return Ok(true);
    };
    if !h.contains(silver.required)? {
        let name = h.item_name(silver.required)?.to_lowercase();
        h.send_message(&format!("You need a {name} to make this item."))?;
        return Ok(true);
    }
    if silver.name == "SILVTHRILL_ROD" || silver.name == "SILVTHRIL_CHAIN" {
        h.send_message("You can't do that yet.")?;
        return Ok(true);
    }
    if h.level()? < silver.level {
        h.send_message(&format!(
            "You need a Crafting level of {} to make this.",
            silver.level
        ))?;
        return Ok(true);
    }
    match opcode {
        OP_MAKE_ONE => silver_make(h, silver, 1)?,
        OP_MAKE_FIVE => silver_make(h, silver, 5)?,
        OP_MAKE_ALL => {
            let amount = h.amount(SILVER_BAR)?;
            silver_make(h, silver, amount)?;
        }
        OP_MAKE_X => h.send_silver_input(silver)?,
        _ => {}
    }
    Ok(true)
}

/// The silver amount prompt's handler: `make(player, product,
/// Integer.parseInt(value.toString()))`.
pub fn silver_input<H: Host>(h: &mut H, silver: &'static Silver, value: &Input) -> Result<()> {
    let amount = match value {
        Input::Text(text) => parse_int(text)?,
        Input::Int(n) => *n,
    };
    silver_make(h, silver, amount)
}

/// A `SilverCraftingPulse`.
#[derive(Debug, Clone, PartialEq)]
pub struct SilverPulse<F> {
    pub silver: &'static Silver,
    /// The furnace attribute when the pulse was made; `None` for the
    /// default `Scenery(-1, -1, 0)`.
    pub furnace: Option<F>,
    pub amount: i32,
    /// `Pulse.delay`.
    pub delay: i32,
}
impl<F: Clone> SilverPulse<F> {
    /// `pulse()`: whether the pulse is done.
    pub fn pulse<H: Host<Furnace = F>>(&mut self, h: &mut H) -> Result<bool> {
        if self.amount < 1 {
            return Ok(true);
        }
        if !h.contains(self.silver.required)? || !h.contains(SILVER_BAR)? {
            return Ok(true);
        }
        h.animate(SILVER_ANIMATION)?;
        h.play_audio(SILVER_SOUND)?;
        if h.remove(SILVER_BAR)? && h.add_item(self.silver.product, self.silver.amount)? {
            h.reward_xp(self.silver.experience)?;
            h.resource_produced(self.silver.product, self.silver.amount, &self.furnace)?;
        } else {
            return Ok(true);
        }
        self.amount = self.amount.wrapping_sub(1);
        self.delay = 5;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_the_pinned_lists() {
        assert_eq!(spin_for_button(23).map(|s| s.name), Some("ROOT"));
        assert_eq!(spin_for_button(31).map(|s| s.product), Some(9438));
        assert_eq!(spin_for_button(18), None);
        assert_eq!(silver_for_button(66).map(|s| s.amount), Some(10));
        assert_eq!(silver_for_button(17), None);
        assert_eq!(parse_int("-12"), Ok(-12));
        assert_eq!(parse_int("+7"), Ok(7));
        assert_eq!(parse_int(""), Err(Error::Number));
        assert_eq!(parse_int("2147483648"), Err(Error::Number));
        assert_eq!(parse_int("1 "), Err(Error::Number));
    }
}
