//! `content.global.skill.herblore`: grinding with a pestle and mortar
//! (`GrindItemPlugin`, `GrindingItem`), herb tar (`HerbTarPlugin`,
//! `HerbTarPulse`, `Tars`) and barbarian mixes (`BarbarianMixListener`,
//! `BarbarianPotion`).
//!
//! Differential corpus: `fixtures/herblore-tools` (oracle/herblore-tools),
//! replayed by `tests/herblore_tools.rs` over these functions with the
//! oracle's declared state. Every outside call is a [`Host`] method.
//!
//! Covered: the three tables, what the two plugins and the listener
//! register, the two `UseWithHandler.handle` bodies ([`grind_handle`],
//! [`tar_handle`]), their skill dialogues' callbacks ([`get_all`],
//! [`create`]), the two pulses with `SkillPulse`'s `start`, `pulse` and
//! `stop` ([`Pulse`]) and the listener's `handle` ([`mix`]). Not covered:
//! `SwampToadPlugin`.

/// `Skills.HERBLORE`.
pub const HERBLORE: i32 = 15;
/// The pestle and mortar: GrindItemPlugin's `UseWithHandler(233)` and
/// HerbTarPulse's `PESTLE_AND_MORTAR`.
pub const PESTLE_AND_MORTAR: i32 = 233;
/// The swamp tar: HerbTarPlugin's `UseWithHandler(1939)` and HerbTarPulse's
/// `SWAMP_TAR`, an item of [`TAR_AMOUNT`].
pub const SWAMP_TAR: i32 = 1939;
pub const TAR_AMOUNT: i32 = 15;
/// `Items.FISHING_BAIT_313`.
pub const FISHING_BAIT: i32 = 313;
/// Both pulses' `ANIMATION`.
pub const ANIMATION: i32 = 364;
/// The eggs of BarbarianMixListener's registrations.
pub const ROE: i32 = 11324;
pub const CAVIAR: i32 = 11326;
/// `Items.ATTACK_MIX2_11429`.
pub const ATTACK_MIX: i32 = 11429;
/// `BarbarianTraining.attributeHerblore`.
pub const ATTRIBUTE_HERBLORE: &str = "/save:barbtraining:herblore";
/// The `RepositionChild` both plugins send after their dialogue:
/// interface, child, x, y.
pub const REPOSITION: [i32; 4] = [309, 2, 210, 15];

/// The classes (`scan.initializable`, `scan.contentInterfaces`).
pub const GRIND_CLASS: &str = "content.global.skill.herblore.GrindItemPlugin";
pub const TAR_CLASS: &str = "content.global.skill.herblore.HerbTarPlugin";
pub const MIX_CLASS: &str = "content.global.skill.herblore.BarbarianMixListener";
/// The two `onUseWith` lambdas of BarbarianMixListener: roe, then caviar.
pub const ROE_LAMBDA: &str = "content.global.skill.herblore.BarbarianMixListener$defineListeners$1";
pub const CAVIAR_LAMBDA: &str =
    "content.global.skill.herblore.BarbarianMixListener$defineListeners$2";
/// The pulses' classes.
pub const GRIND_PULSE_CLASS: &str =
    "content.global.skill.herblore.GrindItemPlugin$handle$handler$1$create$1";
pub const TAR_PULSE_CLASS: &str = "content.global.skill.herblore.HerbTarPulse";

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

/// A `GrindingItem` constant: its one item and its product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grinding {
    pub name: &'static str,
    pub item: i32,
    pub product: i32,
}
const fn grinding(name: &'static str, item: i32, product: i32) -> Grinding {
    Grinding {
        name,
        item,
        product,
    }
}
/// `GrindingItem.values()`.
pub const GRINDING: [Grinding; 17] = [
    grinding("UNICORN_HORN", 237, 235),
    grinding("KEBBIT_TEETH", 10109, 10111),
    grinding("BIRDS_NEST", 5075, 6693),
    grinding("GOAT_HORN", 9735, 9736),
    grinding("MUD_RUNE", 4698, 9594),
    grinding("ASHES", 592, 8865),
    grinding("POISON_KARAMBWAN", 3146, 3152),
    grinding("FISHING_BAIT", 313, 12129),
    grinding("SEAWEED", 401, 6683),
    grinding("BAT_BONES", 530, 2391),
    grinding("CHARCOAL", 973, 704),
    grinding("ASTRAL_RUNE_SHARDS", 11156, 11155),
    grinding("GARLIC", 1550, 4668),
    grinding("DRAGON_SCALE", 243, 241),
    grinding("ANCHOVIES", 319, 11266),
    grinding("CHOCOLATE_BAR", 1973, 1975),
    grinding("GUAM_LEAF", 249, 6681),
];
/// `GrindingItem.forItem(item)`.
pub fn grinding_for_item(id: i32) -> Option<&'static Grinding> {
    GRINDING.iter().find(|g| g.item == id)
}

/// A `Tars` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tar {
    pub name: &'static str,
    pub ingredient: i32,
    pub level: i32,
    pub experience: f64,
    pub tar: i32,
}
const fn tar(name: &'static str, ingredient: i32, level: i32, experience: f64, tar: i32) -> Tar {
    Tar {
        name,
        ingredient,
        level,
        experience,
        tar,
    }
}
/// `Tars.values()`.
pub const TARS: [Tar; 5] = [
    tar("GUAM_TAR", 249, 19, 30.0, 10142),
    tar("GROUND_GUAM_TAR", 6681, 19, 30.0, 10142),
    tar("MARRENTILL_TAR", 251, 31, 42.5, 10143),
    tar("TARROMIN_TAR", 253, 39, 55.0, 10144),
    tar("HARRALANDER_TAR", 255, 44, 72.5, 10145),
];
/// `Tars.forItem(item)`.
pub fn tar_for_item(id: i32) -> Option<&'static Tar> {
    TARS.iter().find(|t| t.ingredient == id)
}

/// A `BarbarianPotion` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    pub name: &'static str,
    pub item: i32,
    pub level: i32,
    pub product: i32,
    pub experience: f64,
    /// Whether roe can be added as well as caviar.
    pub both: bool,
}
const fn barbarian(
    name: &'static str,
    item: i32,
    level: i32,
    product: i32,
    experience: f64,
    both: bool,
) -> Mix {
    Mix {
        name,
        item,
        level,
        product,
        experience,
        both,
    }
}
/// `BarbarianPotion.values()`.
pub const MIXES: [Mix; 24] = [
    barbarian("ATTACK_POTION", 123, 4, 11429, 8.0, true),
    barbarian("ANTI_POISION_POTION", 177, 6, 11433, 12.0, true),
    barbarian("RELIC", 4846, 9, 11437, 14.0, true),
    barbarian("STRENGTH_POTION", 117, 14, 11443, 17.0, true),
    barbarian("RESTORE_POTION", 129, 24, 11449, 21.0, true),
    barbarian("ENERGY_POTION", 3012, 29, 11453, 23.0, false),
    barbarian("DEFENCE_POTION", 135, 33, 11457, 25.0, false),
    barbarian("AGILITY_POTION", 3036, 37, 11461, 27.0, false),
    barbarian("COMBAT_POTION", 9743, 40, 11445, 28.0, false),
    barbarian("PRAYER_POTION", 141, 42, 11465, 29.0, false),
    barbarian("SUPER_ATTACK_POTION", 147, 47, 11469, 33.0, false),
    barbarian("SUPER_ANTIPOISION_POTION", 183, 51, 11473, 35.0, false),
    barbarian("FISHING_POTION", 153, 53, 11477, 38.0, false),
    barbarian("SUPER_ENERGY_POTION", 3020, 56, 11481, 42.0, false),
    barbarian("HUNTER_POTION", 10002, 58, 11517, 40.0, false),
    barbarian("SUPER_STRENGTH_POTION", 159, 59, 11485, 42.0, false),
    barbarian("MAGIC_ESSENCE_POTION", 9023, 61, 11489, 43.0, false),
    barbarian("SUPER_RESTORE", 3028, 67, 11493, 48.0, false),
    barbarian("SUPER_DEFENCE_POTION", 165, 71, 11497, 50.0, false),
    barbarian("ANTIDOTE_PLUS", 5947, 74, 11501, 52.0, false),
    barbarian("ANTIFIRE_POTION", 2456, 75, 11505, 53.0, false),
    barbarian("RANGING_POTION", 171, 80, 11509, 54.0, false),
    barbarian("MAGIC_POTION", 3044, 83, 11513, 57.0, false),
    barbarian("ZAMORAK_BREW", 191, 85, 11521, 58.0, false),
];
/// `BarbarianPotion.forId(id)`.
pub fn mix_for_id(id: i32) -> Option<&'static Mix> {
    MIXES.iter().find(|m| m.item == id)
}

/// `GrindItemPlugin.newInstance`: the ids `addHandler(id, ITEM_TYPE, this)`
/// gets, in order.
pub fn grind_handler_ids() -> Vec<i32> {
    GRINDING.iter().map(|g| g.item).collect()
}
/// `HerbTarPlugin.newInstance`: the ids `addHandler(id, ITEM_TYPE, this)`
/// gets, in order.
pub fn tar_handler_ids() -> Vec<i32> {
    TARS.iter().map(|t| t.ingredient).collect()
}
/// `BarbarianMixListener.defineListeners`: each `onUseWith(IntType.ITEM,
/// potion, egg)` in order, with its lambda.
pub fn mix_registrations() -> Vec<(&'static str, i32, i32)> {
    let mut out = Vec::new();
    for mix in &MIXES {
        if mix.both {
            out.push((ROE_LAMBDA, mix.item, ROE));
        }
        out.push((CAVIAR_LAMBDA, mix.item, CAVIAR));
    }
    out
}

/// The anonymous SkillDialogueHandler a plugin's `handle` opens, with what
/// it captures of the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialogue {
    /// GrindItemPlugin's: `event.usedItem`'s id and the product shown.
    Grind { used: i32, product: i32 },
    /// HerbTarPlugin's: `event.getUsedItem()`'s and `event.getBaseItem()`'s
    /// ids and the tar shown.
    Tar { used: i32, base: i32, tar: i32 },
}
impl Dialogue {
    /// The one item of the ONE_OPTION dialogue.
    pub fn item(self) -> i32 {
        match self {
            Self::Grind { product, .. } => product,
            Self::Tar { tar, .. } => tar,
        }
    }
}

/// What the plugins, the pulses and the listener ask of the world, for
/// their one player.
pub trait Host {
    /// ContentAPI `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32>;
    /// ContentAPI `removeItem(player, Item(id, amount))` (and
    /// `removeItem(player, id)`, which is an amount of one).
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// ContentAPI `addItem(player, id, amount)`; the answer is not read.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `player.animator.animate(Animation(id))`; the answer is not read.
    fn animator_animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(Animation(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))`.
    fn reset_animation(&mut self) -> Result<()>;
    /// `QuestRepository.getStage(Quests.DRUIDIC_RITUAL)`.
    fn druidic_ritual_stage(&mut self) -> Result<i32>;
    /// `player.getSkills().getLevel(Skills.HERBLORE)`.
    fn level(&mut self) -> Result<i32>;
    /// `player.skills.getStaticLevel(Skills.HERBLORE)`.
    fn static_level(&mut self) -> Result<i32>;
    /// `player.getPacketDispatch().sendMessage(message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// ContentAPI `sendMessage(player, message)`.
    fn send_message(&mut self, message: &str) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, message: &str) -> Result<()>;
    /// `player.getInventory().containsItem(Item(id, amount))`.
    fn contains_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `player.getInventory().remove(Item(id, amount))`.
    fn remove(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `player.getInventory().add(Item(id, amount))`; the answer is not
    /// read.
    fn add(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `player.getInventory().getAmount(Item(id))`.
    fn get_amount(&mut self, id: i32) -> Result<i32>;
    /// `player.getSkills().addExperience(Skills.HERBLORE, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `rewardXP(player, Skills.HERBLORE, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// `Item(id).getName()`; a null name is [`Error::Null`] where it is
    /// used.
    fn item_name(&mut self, id: i32) -> Result<Option<String>>;
    /// `getAttribute(player, key, 0)` read as an Int.
    fn attribute_int(&mut self, key: &str) -> Result<i32>;
    /// `setAttribute(player, key, value)`.
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()>;
    /// `SkillDialogueHandler(player, ONE_OPTION, item).open()`: the dialogue
    /// whose callbacks are [`get_all`] and [`create`].
    fn open_dialogue(&mut self, dialogue: Dialogue) -> Result<()>;
    /// `PacketRepository.send(RepositionChild, ChildPositionContext(player,
    /// interface, child, x, y))`.
    fn reposition_child(&mut self, interface: i32, child: i32, x: i32, y: i32) -> Result<()>;
    /// `player.pulseManager.run(pulse)` of a pulse just constructed.
    fn run_pulse(&mut self, pulse: Pulse) -> Result<()>;
}

/// The `RepositionChild` both `handle` bodies end with.
fn reposition<H: Host>(h: &mut H) -> Result<()> {
    let [interface, child, x, y] = REPOSITION;
    h.reposition_child(interface, child, x, y)
}

/// `GrindItemPlugin.handle(event)` for the used and the base item's ids.
pub fn grind_handle<H: Host>(h: &mut H, used: i32, base: i32) -> Result<bool> {
    let grind = grinding_for_item(if used == PESTLE_AND_MORTAR {
        base
    } else {
        used
    });
    // `grind.product` of a null constant.
    let product = grind.ok_or(Error::Null)?.product;
    h.open_dialogue(Dialogue::Grind { used, product })?;
    reposition(h)?;
    Ok(true)
}

/// `Tars.forItem(used == 1939 ? base : used)`.
fn event_tar(used: i32, base: i32) -> Option<&'static Tar> {
    tar_for_item(if used == SWAMP_TAR { base } else { used })
}

/// `HerbTarPlugin.handle(event)` for the used and the base item's ids.
pub fn tar_handle<H: Host>(h: &mut H, used: i32, base: i32) -> Result<bool> {
    // `.getTar()` of a null constant.
    let tar = event_tar(used, base).ok_or(Error::Null)?.tar;
    h.open_dialogue(Dialogue::Tar { used, base, tar })?;
    reposition(h)?;
    Ok(true)
}

/// A dialogue's `getAll(index)`; neither reads its index.
pub fn get_all<H: Host>(h: &mut H, dialogue: Dialogue) -> Result<i32> {
    match dialogue {
        Dialogue::Grind { used, .. } => h.amount_in_inventory(used),
        Dialogue::Tar { used, .. } => h.get_amount(used),
    }
}

/// A dialogue's `create(amount, index)`; neither reads its index.
pub fn create<H: Host>(h: &mut H, dialogue: Dialogue, amount: i32) -> Result<()> {
    match dialogue {
        Dialogue::Grind { used, .. } => {
            let pulse = GrindPulse::new(h, used, amount)?;
            h.run_pulse(Pulse::Grind(pulse))
        }
        Dialogue::Tar { used, base, .. } => {
            // The constructor takes a null constant; `start()` is what
            // reads it.
            let pulse = TarPulse::new(event_tar(used, base), amount);
            h.run_pulse(Pulse::Tar(pulse))
        }
    }
}

/// GrindItemPlugin's anonymous `SkillPulse<Item>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrindPulse {
    /// `node.id`: the used item of the event, which is the pestle and
    /// mortar when that was the item used.
    pub node: i32,
    pub amt: i32,
    /// `Pulse.delay`.
    pub delay: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl GrindPulse {
    /// The constructor: SkillPulse's leaves it stopped, then the `init`
    /// block.
    pub fn new<H: Host>(h: &mut H, node: i32, amount: i32) -> Result<Self> {
        let mut amt = amount;
        if amt > h.amount_in_inventory(node)? {
            amt = h.amount_in_inventory(node)?;
        }
        if node == FISHING_BAIT && amt > h.amount_in_inventory(node)?.wrapping_div(10) {
            // `ceil(amount.toDouble() / 10).roundToInt()`.
            amt = (f64::from(h.amount_in_inventory(node)?) / 10.0).ceil() as i32;
        }
        Ok(Self {
            node,
            amt,
            delay: 2,
            running: false,
        })
    }

    /// `reward()`.
    fn reward<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if self.node == FISHING_BAIT {
            let quantity = if h.amount_in_inventory(FISHING_BAIT)? >= 10 {
                10
            } else {
                h.amount_in_inventory(FISHING_BAIT)?
            };
            if h.remove_item(self.node, quantity)? {
                let product = grinding_for_item(self.node).ok_or(Error::Null)?.product;
                h.add_item(product, quantity)?;
            }
        } else if h.remove_item(self.node, 1)? {
            // `GrindingItem.forItem(node).product` of the pestle and
            // mortar, after its removal.
            let product = grinding_for_item(self.node).ok_or(Error::Null)?.product;
            h.add_item(product, 1)?;
        }
        self.amt = self.amt.wrapping_sub(1);
        Ok(self.amt <= 0)
    }
}

/// A `HerbTarPulse`.
#[derive(Debug, Clone, PartialEq)]
pub struct TarPulse {
    pub tar: Option<&'static Tar>,
    pub amount: i32,
    /// `Pulse.delay`.
    pub delay: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl TarPulse {
    /// `new HerbTarPulse(player, null, tar, amount)`.
    pub fn new(tar: Option<&'static Tar>, amount: i32) -> Self {
        Self {
            tar,
            amount,
            delay: 1,
            running: false,
        }
    }

    /// `checkRequirements()`.
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        // QuestRepository.isComplete: `getStage(player) >= 100`.
        if h.druidic_ritual_stage()? < 100 {
            h.message("You must complete the Druidic Ritual quest before you can use Herblore.")?;
            return Ok(false);
        }
        let level = h.level()?;
        let tar = self.tar.ok_or(Error::Null)?;
        if level < tar.level {
            h.message(&format!(
                "You need a Herblore level of at least {} in order to do this.",
                tar.level
            ))?;
            return Ok(false);
        }
        if !h.contains_item(PESTLE_AND_MORTAR, 1)? {
            h.message("You need Pestle and Mortar in order to crush the herb.")?;
            return Ok(false);
        }
        if !h.contains_item(SWAMP_TAR, TAR_AMOUNT)? {
            h.message("You need at least 15 swamp tar in order to do this.")?;
            return Ok(false);
        }
        Ok(true)
    }

    /// `reward()`.
    fn reward<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if self.delay == 1 {
            self.delay = 4;
            return Ok(false);
        }
        let tar = self.tar.ok_or(Error::Null)?;
        if h.contains_item(SWAMP_TAR, TAR_AMOUNT)?
            && h.contains_item(tar.ingredient, 1)?
            && h.remove(SWAMP_TAR, TAR_AMOUNT)?
            && h.remove(tar.ingredient, 1)?
        {
            h.add(tar.tar, TAR_AMOUNT)?;
            h.add_experience(tar.experience)?;
            let name = h.item_name(tar.ingredient)?.ok_or(Error::Null)?;
            let name = name.to_lowercase().replace("clean", "");
            h.message(&format!(
                "You add the {} to the swamp tar.",
                name.trim_matches(|c: char| c <= ' ')
            ))?;
        } else {
            return Ok(true);
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount == 0)
    }
}

/// One of the two pulses, as a pulse manager holds it.
#[derive(Debug, Clone, PartialEq)]
pub enum Pulse {
    Grind(GrindPulse),
    Tar(TarPulse),
}
impl Pulse {
    /// The pulse's class.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Grind(_) => GRIND_PULSE_CLASS,
            Self::Tar(_) => TAR_PULSE_CLASS,
        }
    }
    /// `Pulse.delay`.
    pub fn delay(&self) -> i32 {
        match self {
            Self::Grind(p) => p.delay,
            Self::Tar(p) => p.delay,
        }
    }
    /// `Pulse.running`.
    pub fn running(&self) -> bool {
        match self {
            Self::Grind(p) => p.running,
            Self::Tar(p) => p.running,
        }
    }
    fn set_running(&mut self, running: bool) {
        match self {
            Self::Grind(p) => p.running = running,
            Self::Tar(p) => p.running = running,
        }
    }
    fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        match self {
            // GrindItemPlugin's: `return true`.
            Self::Grind(_) => Ok(true),
            Self::Tar(p) => p.check_requirements(h),
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
        match self {
            Self::Grind(p) => {
                h.animator_animate(ANIMATION)?;
                p.reward(h)
            }
            Self::Tar(p) => {
                h.animate(ANIMATION)?;
                p.reward(h)
            }
        }
    }

    /// `SkillPulse.stop()`.
    pub fn stop<H: Host>(&mut self, h: &mut H) -> Result<()> {
        h.reset_animation()?;
        self.set_running(false);
        Ok(())
    }
}

/// `BarbarianMixListener.handle(player, inputPotion, egg)` for the two
/// items' ids: its answer.
pub fn mix<H: Host>(h: &mut H, input: i32, egg: i32) -> Result<bool> {
    let stage = h.attribute_int(ATTRIBUTE_HERBLORE)?;
    if stage == 0 {
        return Ok(false);
    }
    let Some(potion) = mix_for_id(input) else {
        return Ok(false);
    };
    // hasLevelStat(player, Skills.HERBLORE, level).
    if h.static_level()? < potion.level {
        h.send_message(&format!(
            "You need a herblore level of {} to make this mix.",
            potion.level
        ))?;
        return Ok(true);
    }
    if !h.remove_item(potion.item, 1)? {
        return Ok(false);
    }
    if !h.remove_item(egg, 1)? {
        h.add_item(potion.item, 1)?;
        return Ok(false);
    }
    h.add_item(potion.product, 1)?;
    h.reward_xp(potion.experience)?;
    h.send_message("You combine your potion with the fish eggs.")?;
    if stage == 1 && potion.product == ATTACK_MIX {
        h.set_attribute_int(ATTRIBUTE_HERBLORE, 2)?;
        h.send_dialogue(
            "You feel you have learned more of barbarian ways. Otto might wish to talk to you more.",
        )?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_the_pinned_lists() {
        assert_eq!(grinding_for_item(237).map(|g| g.product), Some(235));
        assert_eq!(grinding_for_item(PESTLE_AND_MORTAR), None);
        // Clean guam is ground and is a tar's ingredient.
        assert_eq!(grinding_for_item(249).map(|g| g.product), Some(6681));
        assert_eq!(tar_for_item(249).map(|t| t.tar), Some(10142));
        assert_eq!(tar_for_item(6681).map(|t| t.name), Some("GROUND_GUAM_TAR"));
        assert_eq!(mix_for_id(123).map(|m| m.product), Some(ATTACK_MIX));
        // Five potions take roe as well: 5 + 24 registrations.
        let registrations = mix_registrations();
        assert_eq!(registrations.len(), 29);
        assert_eq!(registrations[0], (ROE_LAMBDA, 123, ROE));
        assert_eq!(registrations[1], (CAVIAR_LAMBDA, 123, CAVIAR));
        assert_eq!(registrations[28], (CAVIAR_LAMBDA, 191, CAVIAR));
    }
}
