//! `content.global.skill.herblore`: cleaning a herb (`HerbCleanListener`,
//! `Herbs`) and mixing a potion (`PotionMixingListener`, `UnfinishedPotion`,
//! `FinishedPotion`, `GenericPotion`, `HerblorePulse` with `SkillPulse`'s
//! `start`, `pulse` and `stop`).
//!
//! Differential corpus: `fixtures/herblore` (oracle/herblore), replayed by
//! `tests/herblore.rs` over these functions with the oracle's declared
//! state. Every outside call is a [`Host`] method.
//!
//! Covered: the three tables, the "clean" lambda ([`clean`]), the two
//! `onUseWith` lambdas up to `mixPotion` ([`use_with`], [`mix`]), the skill
//! dialogue's two callbacks ([`get_all`], [`create`]) and the pulse
//! ([`Pulse`]). Not covered: the rest of herblore (grinding, herb tar,
//! barbarian mixes, swamp toads).

/// `Skills.HERBLORE`.
pub const HERBLORE: i32 = 15;
/// `HerblorePulse.VIAL_OF_WATER`.
pub const VIAL_OF_WATER: i32 = 227;
/// `HerblorePulse.ANIMATION`.
pub const ANIMATION: i32 = 363;
/// `Sounds.GRIND_2608`.
pub const GRIND: i32 = 2608;
/// The sound of the "clean" lambda.
pub const CLEAN_SOUND: i32 = 5153;
/// `SkillcapePerks.BREWMASTER.attribute`.
pub const BREWMASTER: &str = "cape_perks:brewmaster";

/// The listeners' classes (`scan.contentInterfaces`).
pub const CLEAN_CLASS: &str = "content.global.skill.herblore.HerbCleanListener";
pub const MIXING_CLASS: &str = "content.global.skill.herblore.PotionMixingListener";
/// The "clean" lambda.
pub const CLEAN_LAMBDA: &str = "content.global.skill.herblore.HerbCleanListener$defineListeners$1";
/// The two `onUseWith` lambdas.
pub const UNFINISHED_LAMBDA: &str =
    "content.global.skill.herblore.PotionMixingListener$defineListeners$1";
pub const FINISHED_LAMBDA: &str =
    "content.global.skill.herblore.PotionMixingListener$defineListeners$2";
/// The pulse's class.
pub const PULSE_CLASS: &str = "content.global.skill.herblore.HerblorePulse";

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// A ClassCastException.
    Cast,
    /// An IndexOutOfBoundsException.
    Index,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// The quests whose stage the sources read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quest {
    DruidicRitual,
    ZogreFleshEaters,
    HeroesQuest,
}
impl Quest {
    /// The `Quests` constant.
    pub fn constant(self) -> &'static str {
        match self {
            Self::DruidicRitual => "DRUIDIC_RITUAL",
            Self::ZogreFleshEaters => "ZOGRE_FLESH_EATERS",
            Self::HeroesQuest => "HEROES_QUEST",
        }
    }
    /// `Quests.toString()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::DruidicRitual => "Druidic Ritual",
            Self::ZogreFleshEaters => "Zogre Flesh Eaters",
            Self::HeroesQuest => "Heroes' Quest",
        }
    }
    /// The quest class's index (DruidicRitual.kt, ZogreFleshEaters.kt,
    /// HeroesQuest.kt).
    pub fn index(self) -> usize {
        match self {
            Self::DruidicRitual => 48,
            Self::ZogreFleshEaters => 40,
            Self::HeroesQuest => 75,
        }
    }
}

/// A `Herbs` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Herb {
    pub name: &'static str,
    pub herb: i32,
    pub experience: f64,
    pub level: i32,
    pub product: i32,
}
const fn herb(name: &'static str, herb: i32, experience: f64, level: i32, product: i32) -> Herb {
    Herb {
        name,
        herb,
        experience,
        level,
        product,
    }
}
/// `Herbs.values()`.
pub const HERBS: [Herb; 20] = [
    herb("GUAM", 199, 2.5, 3, 249),
    herb("MARRENTILL", 201, 3.8, 5, 251),
    herb("TARROMIN", 203, 5.0, 11, 253),
    herb("HARRALANDER", 205, 6.3, 20, 255),
    herb("RANARR", 207, 7.5, 25, 257),
    herb("TOADFLAX", 3049, 8.0, 30, 2998),
    herb("SPIRIT_WEED", 12174, 7.8, 35, 12172),
    herb("IRIT", 209, 8.8, 40, 259),
    herb("AVANTOE", 211, 10.0, 48, 261),
    herb("KWUARM", 213, 11.3, 54, 263),
    herb("SNAPDRAGON", 3051, 11.8, 59, 3000),
    herb("CADANTINE", 215, 12.5, 65, 265),
    herb("LANTADYME", 2485, 13.1, 67, 2481),
    herb("DWARF_WEED", 217, 13.8, 70, 267),
    herb("TORSTOL", 219, 15.0, 75, 269),
    herb("SNAKE_WEED", 1525, 2.5, 3, 1526),
    herb("ARDRIGAL", 1527, 2.5, 3, 1528),
    herb("SITO_FOIL", 1529, 2.5, 3, 1530),
    herb("VOLENCIA_MOSS", 1531, 2.5, 3, 1532),
    herb("ROGUES_PURSE", 1533, 2.5, 3, 1534),
];
/// `Herbs.forItem(item)`.
pub fn herb_for_item(id: i32) -> Option<&'static Herb> {
    HERBS.iter().find(|herb| herb.herb == id)
}

/// An `UnfinishedPotion` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unfinished {
    pub name: &'static str,
    pub base: i32,
    pub ingredient: i32,
    pub level: i32,
    pub potion: i32,
}
const fn unfinished(
    name: &'static str,
    base: i32,
    ingredient: i32,
    level: i32,
    potion: i32,
) -> Unfinished {
    Unfinished {
        name,
        base,
        ingredient,
        level,
        potion,
    }
}
/// `UnfinishedPotion.values()`.
pub const UNFINISHED: [Unfinished; 21] = [
    unfinished("GUAM", 227, 249, 3, 91),
    unfinished("MARRENTILL", 227, 251, 5, 93),
    unfinished("ROGUES_PURSE", 227, 1534, 5, 4840),
    unfinished("TARROMIN", 227, 253, 12, 95),
    unfinished("HARRALANDER", 227, 255, 22, 97),
    unfinished("RANARR", 227, 257, 30, 99),
    unfinished("TOADFLAX", 227, 2998, 34, 3002),
    unfinished("SPIRIT_WEED", 227, 12172, 40, 12181),
    unfinished("IRIT", 227, 259, 45, 101),
    unfinished("AVANTOE", 227, 261, 50, 103),
    unfinished("KWUARM", 227, 263, 55, 105),
    unfinished("STARFLOWER", 227, 9017, 57, 9019),
    unfinished("SNAPDRAGON", 227, 3000, 63, 3004),
    unfinished("CADANTINE", 227, 265, 66, 107),
    unfinished("LANTADYME", 227, 2481, 69, 2483),
    unfinished("DWARF_WEED", 227, 267, 72, 109),
    unfinished("TORSTOL", 227, 269, 75, 111),
    unfinished("STRONG_WEAPON_POISON", 5935, 6016, 73, 5936),
    unfinished("SUPER_STRONG_WEAPON_POISON", 5935, 2398, 82, 5939),
    unfinished("STRONG_ANTIPOISON", 5935, 2998, 68, 5942),
    unfinished("SUPER_STRONG_ANTIPOISON", 5935, 259, 79, 5951),
];
/// `UnfinishedPotion.forItem(item, base)`.
pub fn unfinished_for_item(item: i32, base: i32) -> Option<&'static Unfinished> {
    UNFINISHED.iter().find(|potion| {
        (potion.ingredient == item || potion.ingredient == base)
            && (item == potion.base || base == potion.base)
    })
}

/// A `FinishedPotion`'s `requirement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// `{ true }`.
    None,
    /// `getQuestStage(player, quest) >= stage`.
    StageAtLeast(Quest, i32),
    /// `getQuestStage(player, quest) > stage`.
    StageAbove(Quest, i32),
    /// `isQuestComplete(player, quest)`.
    Complete(Quest),
    /// `hasRequirement(player, quest, false)`, by the `Quests` constant.
    Has(&'static str),
}
use Requirement as R;

/// A `FinishedPotion` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Finished {
    pub name: &'static str,
    pub unfinished: i32,
    pub ingredient: i32,
    pub level: i32,
    pub experience: f64,
    pub potion: i32,
    pub requirement: Requirement,
}
const fn finished(
    name: &'static str,
    unfinished: i32,
    ingredient: i32,
    level: i32,
    experience: f64,
    potion: i32,
    requirement: Requirement,
) -> Finished {
    Finished {
        name,
        unfinished,
        ingredient,
        level,
        experience,
        potion,
        requirement,
    }
}
/// `FinishedPotion.values()`.
pub const FINISHED: [Finished; 54] = [
    finished("ATTACK_POTION", 91, 221, 3, 25.0, 121, R::None),
    finished("ANTIPOISON_POTION", 93, 235, 5, 37.5, 175, R::None),
    finished("STRENGTH_POTION", 95, 225, 12, 50.0, 115, R::None),
    finished("RESTORE_POTION", 97, 223, 22, 62.5, 127, R::None),
    finished("ENERGY_POTION", 97, 1975, 26, 67.5, 3010, R::None),
    finished("DEFENCE_POTION", 99, 239, 30, 45.0, 133, R::None),
    finished("AGILITY_POTION", 3002, 2152, 34, 80.0, 3034, R::None),
    finished("COMBAT_POTION", 97, 9736, 36, 84.0, 9741, R::None),
    finished("PRAYER_POTION", 99, 231, 38, 87.5, 139, R::None),
    finished("SUMMONING_POTION", 12181, 12109, 40, 92.0, 12142, R::None),
    finished("SUPER_ATTACK", 101, 221, 45, 100.0, 145, R::None),
    finished("SUPER_ANTIPOISON", 101, 235, 48, 106.3, 181, R::None),
    finished("FISHING_POTION", 103, 231, 50, 112.5, 151, R::None),
    finished("SUPER_ENERGY", 103, 2970, 52, 117.5, 3018, R::None),
    finished("HUNTING_POTION", 103, 10111, 53, 120.0, 10000, R::None),
    finished("SUPER_STRENGTH", 105, 225, 55, 125.0, 157, R::None),
    finished("WEAPON_POISON", 105, 241, 60, 137.5, 187, R::None),
    finished("SUPER_RESTORE", 3004, 223, 63, 142.5, 3026, R::None),
    finished("SUPER_DEFENCE", 107, 239, 66, 160.0, 163, R::None),
    finished("ANTIFIRE", 2483, 241, 69, 157.5, 2454, R::None),
    finished("RANGING_POTION", 109, 245, 72, 162.5, 169, R::None),
    finished("MAGIC", 2483, 3138, 76, 172.5, 3042, R::None),
    finished("ZAMORAK_BREW", 111, 247, 78, 175.0, 189, R::None),
    finished("SARADOMIN_BREW", 3002, 6693, 81, 180.0, 6687, R::None),
    finished("STRONG_WEAPON_POISON", 5936, 223, 73, 165.0, 5937, R::None),
    finished(
        "SUPER_STRONG_WEAPON_POISON",
        5939,
        6018,
        82,
        190.0,
        5940,
        R::None,
    ),
    finished("STRONG_ANTIPOISON", 5942, 6049, 68, 155.0, 5945, R::None),
    finished(
        "SUPER_STRONG_ANTIPOISON",
        5951,
        6051,
        79,
        177.5,
        5954,
        R::None,
    ),
    finished(
        "RELICYMS_BALM",
        4840,
        1526,
        8,
        40.0,
        4844,
        R::StageAtLeast(Quest::ZogreFleshEaters, 7),
    ),
    finished(
        "SERUM_207",
        95,
        592,
        15,
        50.0,
        3410,
        R::Has("SHADES_OF_MORTTON"),
    ),
    finished(
        "BLAMISH_OIL",
        97,
        1581,
        25,
        80.0,
        1582,
        R::StageAbove(Quest::HeroesQuest, 0),
    ),
    finished(
        "SUPER_FISHING_EXPLOSIVE",
        91,
        12630,
        31,
        55.0,
        12633,
        R::Has("KENNITHS_CONCERNS"),
    ),
    finished(
        "SHRINK_ME_QUICK",
        95,
        11205,
        52,
        6.0,
        11204,
        R::Has("GRIM_TALES"),
    ),
    finished(
        "MAGIC_ESSENCE",
        9019,
        9018,
        57,
        130.0,
        9022,
        R::Has("FAIRYTALE_II_CURE_A_QUEEN"),
    ),
    finished(
        "SANFEW_SERUM_MIX1_1",
        3030,
        235,
        65,
        47.5,
        10915,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX1_2",
        3028,
        235,
        65,
        47.5,
        10913,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX1_3",
        3026,
        235,
        65,
        47.5,
        10911,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX1_4",
        3024,
        235,
        65,
        47.5,
        10909,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX2_1",
        10915,
        1526,
        65,
        52.5,
        10923,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX2_2",
        10913,
        1526,
        65,
        52.5,
        10921,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX2_3",
        10911,
        1526,
        65,
        52.5,
        10919,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM_MIX2_4",
        10909,
        1526,
        65,
        52.5,
        10917,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM1",
        10923,
        10937,
        65,
        60.0,
        10931,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM2",
        10921,
        10937,
        65,
        60.0,
        10929,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM3",
        10919,
        10937,
        65,
        60.0,
        10927,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "SANFEW_SERUM4",
        10917,
        10937,
        65,
        60.0,
        10925,
        R::Complete(Quest::ZogreFleshEaters),
    ),
    finished(
        "GUTHIX_BALANCE_UNF_1",
        131,
        1550,
        22,
        25.0,
        7658,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_UNF_2",
        129,
        1550,
        22,
        25.0,
        7656,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_UNF_3",
        127,
        1550,
        22,
        25.0,
        7654,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_UNF_4",
        2430,
        1550,
        22,
        25.0,
        7652,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_1",
        7658,
        7650,
        22,
        25.0,
        7666,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_2",
        7656,
        7650,
        22,
        25.0,
        7664,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_3",
        7654,
        7650,
        22,
        25.0,
        7662,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
    finished(
        "GUTHIX_BALANCE_4",
        7652,
        7650,
        22,
        25.0,
        7660,
        R::Has("IN_AID_OF_THE_MYREQUE"),
    ),
];
/// `FinishedPotion.getPotion(unf, ingredient)`.
pub fn finished_potion(unfinished: i32, ingredient: i32) -> Option<&'static Finished> {
    FINISHED
        .iter()
        .find(|potion| potion.unfinished == unfinished && potion.ingredient == ingredient)
}

/// A `GenericPotion`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Potion {
    pub base: i32,
    pub ingredient: i32,
    pub level: i32,
    pub experience: f64,
    pub product: i32,
}
impl Potion {
    /// `GenericPotion.transform(UnfinishedPotion)`.
    pub fn of_unfinished(potion: &Unfinished) -> Self {
        Self {
            base: potion.base,
            ingredient: potion.ingredient,
            level: potion.level,
            experience: 0.0,
            product: potion.potion,
        }
    }
    /// `GenericPotion.transform(FinishedPotion)`.
    pub fn of_finished(potion: &Finished) -> Self {
        Self {
            base: potion.unfinished,
            ingredient: potion.ingredient,
            level: potion.level,
            experience: potion.experience,
            product: potion.potion,
        }
    }
}

/// `distinct()` of a list of ids, in its order.
fn distinct(ids: impl Iterator<Item = i32>) -> Vec<i32> {
    let mut out = Vec::new();
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// The two `onUseWith(IntType.ITEM, ...)` lambdas, in registration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listener {
    /// A base with an unfinished potion's ingredient.
    Unfinished,
    /// An unfinished potion with a finished potion's ingredient.
    Finished,
}
impl Listener {
    pub const ALL: [Self; 2] = [Self::Unfinished, Self::Finished];
    /// The ids the listener registers: the used ones, then the others.
    pub fn ids(self) -> (Vec<i32>, Vec<i32>) {
        match self {
            Self::Unfinished => (
                distinct(UNFINISHED.iter().map(|p| p.base)),
                distinct(UNFINISHED.iter().map(|p| p.ingredient)),
            ),
            Self::Finished => (
                distinct(FINISHED.iter().map(|p| p.unfinished)),
                distinct(FINISHED.iter().map(|p| p.ingredient)),
            ),
        }
    }
    /// The `onUseWith` lambda.
    pub fn lambda(self) -> &'static str {
        match self {
            Self::Unfinished => UNFINISHED_LAMBDA,
            Self::Finished => FINISHED_LAMBDA,
        }
    }
}

/// What the listeners and the pulse ask of the world, for their one player.
pub trait Host {
    /// An inventory item as the "clean" lambda holds it.
    type Item;

    /// `QuestRepository.getStage(quest)`.
    fn quest_stage(&mut self, quest: Quest) -> Result<i32>;
    /// ContentAPI `hasRequirement(player, quest, false)`, by the `Quests`
    /// constant.
    fn has_requirement(&mut self, quest: &str) -> Result<bool>;
    /// `getDynLevel(player, Skills.HERBLORE)`, which is
    /// `player.getSkills().getLevel(Skills.HERBLORE)`.
    fn level(&mut self) -> Result<i32>;
    /// ContentAPI `sendMessage(player, message)`.
    fn send_message(&mut self, message: &str) -> Result<()>;
    /// `player.getPacketDispatch().sendMessage(message)` (and
    /// `player.sendMessage(message)`, which is that call).
    fn message(&mut self, message: &str) -> Result<()>;
    /// `lock(player, ticks)`.
    fn lock(&mut self, ticks: i32) -> Result<()>;
    /// `Item(id).getName()`; a null name is [`Error::Null`] where it is
    /// used.
    fn item_name(&mut self, id: i32) -> Result<Option<String>>;
    /// `item.slot`.
    fn slot(&mut self, item: &Self::Item) -> Result<i32>;
    /// `replaceSlot(player, slot, Item(product), item)`; the answer is not
    /// read.
    fn replace_slot(&mut self, slot: i32, product: i32, item: &Self::Item) -> Result<()>;
    /// `rewardXP(player, Skills.HERBLORE, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// `player.getSkills().addExperience(Skills.HERBLORE, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32>;
    /// `player.getInventory().containsItem(Item(id))`.
    fn contains_item(&mut self, id: i32) -> Result<bool>;
    /// `player.getInventory().remove(Item(base), Item(ingredient))`.
    fn remove(&mut self, base: i32, ingredient: i32) -> Result<bool>;
    /// `player.getInventory().add(Item(id))`; the answer is not read.
    fn add(&mut self, id: i32) -> Result<()>;
    /// `player.animate(Animation(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))`.
    fn reset_animation(&mut self) -> Result<()>;
    /// `player.getAttribute(key, false)`.
    fn attribute(&mut self, key: &str) -> Result<bool>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `Consumables.getConsumableById(id)`: its consumable's first id, if
    /// there is one.
    fn consumable_first_id(&mut self, id: i32) -> Result<Option<i32>>;
    /// `player.pulseManager.run(HerblorePulse(player, potion.base, amount,
    /// potion))`.
    fn run_pulse(&mut self, potion: Potion, amount: i32) -> Result<()>;
    /// `sendSkillDialogue(player) { withItems(potion.product) ... }`: the
    /// dialogue whose callbacks are [`get_all`] and [`create`].
    fn open_dialogue(&mut self, potion: Potion) -> Result<()>;
}

/// The "clean" lambda for the item of the option; its answer is always
/// `true`.
pub fn clean<H: Host>(h: &mut H, id: i32, item: &H::Item) -> Result<bool> {
    h.lock(1)?;
    // requireQuest(player, Quests.DRUIDIC_RITUAL, "before you can use Herblore.").
    if h.quest_stage(Quest::DruidicRitual)? != 100 {
        h.send_message(&format!(
            "You must have completed the {} quest before you can use Herblore.",
            Quest::DruidicRitual.name()
        ))?;
        return Ok(true);
    }
    let Some(herb) = herb_for_item(id) else {
        return Ok(true);
    };
    if h.level()? < herb.level {
        h.send_message(&format!(
            "You cannot clean this herb. You need a Herblore level of {} to attempt this.",
            herb.level
        ))?;
        return Ok(true);
    }
    let slot = h.slot(item)?;
    h.replace_slot(slot, herb.product, item)?;
    h.reward_xp(herb.experience)?;
    h.play_audio(CLEAN_SOUND)?;
    let name = h.item_name(herb.product)?.ok_or(Error::Null)?;
    let name = name.to_lowercase().replace("clean", "");
    h.send_message(&format!(
        "You clean the dirt from the {} leaf.",
        name.trim_matches(|c: char| c <= ' ')
    ))?;
    Ok(true)
}

/// A `FinishedPotion`'s `canMake(player)`.
pub fn can_make<H: Host>(h: &mut H, potion: &Finished) -> Result<bool> {
    Ok(match potion.requirement {
        R::None => true,
        R::StageAtLeast(quest, stage) => h.quest_stage(quest)? >= stage,
        R::StageAbove(quest, stage) => h.quest_stage(quest)? > stage,
        R::Complete(quest) => h.quest_stage(quest)? == 100,
        R::Has(quest) => h.has_requirement(quest)?,
    })
}

/// One of the two lambdas for two items: its answer.
pub fn use_with<H: Host>(h: &mut H, listener: Listener, used: i32, with: i32) -> Result<bool> {
    match listener {
        Listener::Unfinished => {
            let Some(potion) = unfinished_for_item(used, with) else {
                return Ok(false);
            };
            mix(h, Potion::of_unfinished(potion))?;
        }
        Listener::Finished => {
            let Some(potion) = finished_potion(used, with) else {
                return Ok(false);
            };
            if !can_make(h, potion)? {
                h.send_message("You're not sure what effect this might have.")?;
                return Ok(true);
            }
            mix(h, Potion::of_finished(potion))?;
        }
    }
    Ok(true)
}

/// `mixPotion(player, potion)`.
pub fn mix<H: Host>(h: &mut H, potion: Potion) -> Result<()> {
    if h.amount_in_inventory(potion.base)? == 1 || h.amount_in_inventory(potion.ingredient)? == 1 {
        h.run_pulse(potion, 1)
    } else {
        h.open_dialogue(potion)
    }
}

/// The dialogue's `getAll(index)`: `builder.items[index]`, then
/// `calculateMaxAmount`.
pub fn get_all<H: Host>(h: &mut H, potion: &Potion, index: i32) -> Result<i32> {
    if index != 0 {
        return Err(Error::Index);
    }
    h.amount_in_inventory(potion.base)
}

/// The dialogue's `create(amount, index)`: `builder.items[index]`, then the
/// `create` callback with its amount.
pub fn create<H: Host>(h: &mut H, potion: Potion, amount: i32, index: i32) -> Result<()> {
    if index != 0 {
        return Err(Error::Index);
    }
    h.run_pulse(potion, amount)
}

/// `StringUtils.formatDisplayName(name)`.
pub fn format_display_name(name: &str) -> String {
    let name = name.replace('_', " ").to_lowercase();
    let mut out = String::new();
    let mut was_space = true;
    for c in name.chars() {
        if was_space {
            out.extend(c.to_uppercase());
            was_space = false;
        } else {
            out.push(c);
        }
        if c == ' ' {
            was_space = true;
        }
    }
    out
}

/// A `HerblorePulse`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pulse {
    pub potion: Potion,
    pub amount: i32,
    pub initial_amount: i32,
    pub cycles: i32,
    /// `Pulse.delay`.
    pub delay: i32,
    /// `Pulse.running`.
    pub running: bool,
}
impl Pulse {
    /// `new HerblorePulse(player, potion.base, amount, potion)`:
    /// SkillPulse's constructor leaves it stopped.
    pub fn new(potion: Potion, amount: i32) -> Self {
        Self {
            potion,
            amount,
            initial_amount: amount,
            cycles: 0,
            delay: 1,
            running: false,
        }
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
        // animate(): empty.
        self.reward(h)
    }

    /// `SkillPulse.stop()`.
    pub fn stop<H: Host>(&mut self, h: &mut H) -> Result<()> {
        h.reset_animation()?;
        self.running = false;
        Ok(())
    }

    /// `checkRequirements()`.
    pub fn check_requirements<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        // QuestRepository.isComplete: `getStage(player) >= 100`.
        if h.quest_stage(Quest::DruidicRitual)? < 100 {
            h.message("You must complete the Druidic Ritual quest before you can use Herblore.")?;
            return Ok(false);
        }
        if h.level()? < self.potion.level {
            h.message(&format!(
                "You need a Herblore level of at least {} in order to do this.",
                self.potion.level
            ))?;
            return Ok(false);
        }
        Ok(h.contains_item(self.potion.base)? && h.contains_item(self.potion.ingredient)?)
    }

    /// `reward()`.
    pub fn reward<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        if self.potion.base == VIAL_OF_WATER {
            if self.initial_amount == 1 && self.delay == 1 {
                h.animate(ANIMATION)?;
                self.delay = 3;
                return Ok(false);
            }
            self.handle_unfinished(h)?;
        } else {
            if self.initial_amount == 1 && self.delay == 1 {
                h.animate(ANIMATION)?;
                self.delay = 3;
                return Ok(false);
            }
            if self.delay == 1 {
                self.delay = 3;
                h.animate(ANIMATION)?;
                return Ok(false);
            }
            self.handle_finished(h)?;
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount == 0)
    }

    /// Both handlers' condition: the two items held, then removed.
    fn take<H: Host>(&self, h: &mut H) -> Result<bool> {
        Ok(h.contains_item(self.potion.base)?
            && h.contains_item(self.potion.ingredient)?
            && h.remove(self.potion.base, self.potion.ingredient)?)
    }

    /// `handleUnfinished()`.
    fn handle_unfinished<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if self.cycles == 0 {
            h.animate(ANIMATION)?;
        }
        if self.take(h)? {
            h.add(self.potion.product)?;
            let name = h.item_name(self.potion.ingredient)?.ok_or(Error::Null)?;
            let name = format_display_name(&name.to_lowercase().replace("clean", ""));
            h.message(&format!("You put the{name} leaf into the vial of water."))?;
            h.play_audio(GRIND)?;
            let cycles = self.cycles;
            self.cycles = cycles.wrapping_add(1);
            if cycles == 3 {
                h.animate(ANIMATION)?;
                self.cycles = 0;
            }
        }
        Ok(())
    }

    /// `handleFinished()`.
    fn handle_finished<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if self.take(h)? {
            let mut item = self.potion.product;
            if h.attribute(BREWMASTER)? && h.random(100)? < 15 {
                if let Some(first) = h.consumable_first_id(item)? {
                    item = first;
                }
                h.message("Due to your expertise, you manage to make an extra dose.")?;
            }
            h.add(item)?;
            h.add_experience(self.potion.experience)?;
            let name = h.item_name(self.potion.ingredient)?.ok_or(Error::Null)?;
            h.message(&format!(
                "You mix the {} into your potion.",
                name.to_lowercase()
            ))?;
            h.play_audio(GRIND)?;
            h.animate(ANIMATION)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_the_pinned_lists() {
        assert_eq!(herb_for_item(199).map(|h| h.product), Some(249));
        assert_eq!(herb_for_item(249), None);
        assert_eq!(unfinished_for_item(227, 249).map(|p| p.potion), Some(91));
        assert_eq!(unfinished_for_item(249, 227).map(|p| p.potion), Some(91));
        // Toadflax is an ingredient of both bases: the base decides.
        assert_eq!(
            unfinished_for_item(5935, 2998).map(|p| p.potion),
            Some(5942)
        );
        assert_eq!(finished_potion(91, 221).map(|p| p.potion), Some(121));
        assert_eq!(finished_potion(221, 91), None);
        assert_eq!(Listener::Unfinished.ids().0, [227, 5935]);
        assert_eq!(Listener::Unfinished.ids().1.len(), 19);
    }

    #[test]
    fn display_names_capitalise_after_spaces() {
        assert_eq!(format_display_name(" guam"), " Guam");
        assert_eq!(format_display_name("dwarf_weed"), "Dwarf Weed");
    }
}
