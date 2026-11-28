//! `content.global.skill.fletching.stringing` and `.arrow`: a string used on
//! an unstrung bow or crossbow (`StringingListeners`, `StringItemScript`,
//! `StringableCraftInfo`) and the three arrow assemblies (`ArrowListeners`,
//! `HeadlessArrowCraftScript`, `TippedArrowCraftScript`,
//! `FlightedOgreArrowCraftScript`, `ArrowCraftInfo`, `Feathers`).
//!
//! Differential corpus: `fixtures/fletching` (oracle/fletching), replayed by
//! `tests/fletching.rs` over these functions with the oracle's declared
//! state; the unit tests below are a second, hand-read check. Every outside
//! call is a [`Host`] method.
//!
//! Covered: both tables, each listener's checks before its
//! SkillDialogueHandler ([`string_use`]), each handler's `create` and
//! `getAll` ([`create`], [`get_all`]) and the four queued scripts
//! ([`script`]). The SkillDialogueHandler itself is `dialogue_dsl::skill`'s.
//! Not covered: the rest of fletching (bolts, darts, crossbow limbs, gems,
//! grapples).
use super::{
    dialogue_dsl::skill::SkillDialogue,
    fletching_logs::{Error, Quest, Result, SEERS_ZONES},
};

pub const BOW_STRING: i32 = 1777;
pub const CROSSBOW_STRING: i32 = 9438;
pub const ARROW_SHAFT: i32 = 52;
pub const HEADLESS_ARROW: i32 = 53;
pub const OGRE_ARROW_SHAFT: i32 = 2864;
pub const FLIGHTED_OGRE_ARROW: i32 = 2865;
/// `ArrowListeners.FLIGHTED_OGRE_ARROW_LEVEL`.
pub const FLIGHTED_OGRE_ARROW_LEVEL: i32 = 5;
/// `Feathers.all`.
pub const FEATHERS: [i32; 6] = [314, 10087, 10088, 10089, 10090, 10091];
/// `StringableCraftInfo.stringItemIds`.
pub const STRINGS: [i32; 2] = [BOW_STRING, CROSSBOW_STRING];
/// `AchievementDiaryAttributeKeys.FLETCHED_UNSTRUNG_MAGIC_SHORTBOW`.
pub const ATTRIBUTE_MAGIC_SHORTBOW: &str = "diary:seers:fletch-magic-short-bow";
/// `initialDelay` of every script here.
pub const INITIAL_DELAY: i32 = 1;

/// The listeners' classes (`scan.contentInterfaces`).
pub const STRINGING_CLASS: &str = "content.global.skill.fletching.stringing.StringingListeners";
pub const ARROW_CLASS: &str = "content.global.skill.fletching.arrow.ArrowListeners";
/// The `onUseWith` lambdas.
pub const STRINGING_LAMBDA: &str =
    "content.global.skill.fletching.stringing.StringingListeners$defineListeners$1";
pub const HEADLESS_LAMBDA: &str =
    "content.global.skill.fletching.arrow.ArrowListeners$defineListeners$1";
pub const TIPPED_LAMBDA: &str =
    "content.global.skill.fletching.arrow.ArrowListeners$defineListeners$2";
pub const OGRE_LAMBDA: &str =
    "content.global.skill.fletching.arrow.ArrowListeners$defineListeners$3";

/// A `StringableCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stringable {
    pub name: &'static str,
    pub crossbow: bool,
    pub unstrung: i32,
    pub strung: i32,
    pub level: i32,
    pub experience: f64,
    pub animation: i32,
}
const fn bow(
    name: &'static str,
    crossbow: bool,
    unstrung: i32,
    strung: i32,
    level: i32,
    experience: f64,
    animation: i32,
) -> Stringable {
    Stringable {
        name,
        crossbow,
        unstrung,
        strung,
        level,
        experience,
        animation,
    }
}
impl Stringable {
    /// `applicableStringId`.
    pub fn string(&self) -> i32 {
        if self.crossbow {
            CROSSBOW_STRING
        } else {
            BOW_STRING
        }
    }
}

/// `StringableCraftInfo.values()`.
pub const STRINGABLES: [Stringable; 20] = [
    bow("SHORT_BOW", false, 50, 841, 5, 5.0, 6678),
    bow("LONG_BOW", false, 48, 839, 10, 10.0, 6684),
    bow("OAK_SHORTBOW", false, 54, 843, 20, 16.5, 6679),
    bow("OAK_LONGBOW", false, 56, 845, 25, 25.0, 6685),
    bow("OGRE_COMP_BOW", false, 4825, 4827, 30, 45.0, -1),
    bow("WILLOW_SHORTBOW", false, 60, 849, 35, 33.3, 6680),
    bow("WILLOW_LONGBOW", false, 58, 847, 40, 41.5, 6686),
    bow("MAPLE_SHORTBOW", false, 64, 853, 50, 50.0, 6681),
    bow("MAPLE_LONGBOW", false, 62, 851, 55, 58.3, 6687),
    bow("YEW_SHORTBOW", false, 68, 857, 65, 67.5, 6682),
    bow("YEW_LONGBOW", false, 66, 855, 70, 75.0, 6688),
    bow("MAGIC_SHORTBOW", false, 72, 861, 80, 83.3, 6683),
    bow("MAGIC_LONGBOW", false, 70, 859, 85, 91.5, 6689),
    bow("BRONZE_CBOW", true, 9454, 9174, 9, 6.0, 6671),
    bow("BLURITE_CBOW", true, 9456, 9176, 24, 16.0, 6672),
    bow("IRON_CBOW", true, 9457, 9177, 39, 22.0, 6673),
    bow("STEEL_CBOW", true, 9459, 9179, 46, 27.0, 6674),
    bow("MITHIRIL_CBOW", true, 9461, 9181, 54, 32.0, 6675),
    bow("ADAMANT_CBOW", true, 9463, 9183, 61, 41.0, 6676),
    bow("RUNITE_CBOW", true, 9465, 9185, 69, 50.0, 6677),
];

/// `StringableCraftInfo.unstrungBowIds`.
pub fn unstrung_ids() -> Vec<i32> {
    STRINGABLES.iter().map(|bow| bow.unstrung).collect()
}
/// `StringableCraftInfo.forUnstrungBowItemId(id)`, as an index.
pub fn for_unstrung(id: i32) -> Option<usize> {
    STRINGABLES.iter().position(|bow| bow.unstrung == id)
}

/// An `ArrowCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrow {
    pub name: &'static str,
    pub tip: i32,
    pub arrow: i32,
    pub level: i32,
    pub experience: f64,
}
const fn arrow(name: &'static str, tip: i32, arrow: i32, level: i32, experience: f64) -> Arrow {
    Arrow {
        name,
        tip,
        arrow,
        level,
        experience,
    }
}
/// `ArrowCraftInfo.values()`.
pub const ARROWS: [Arrow; 8] = [
    arrow("BRONZE_ARROW", 39, 882, 1, 1.3),
    arrow("IRON_ARROW", 40, 884, 15, 2.5),
    arrow("STEEL_ARROW", 41, 886, 30, 5.0),
    arrow("MITHRIL_ARROW", 42, 888, 45, 7.5),
    arrow("ADAMANT_ARROW", 43, 890, 60, 10.0),
    arrow("RUNE_ARROW", 44, 892, 75, 12.5),
    arrow("DRAGON_ARROW", 11237, 11212, 90, 15.0),
    arrow("BROAD_ARROW", 13278, 4160, 52, 15.0),
];
/// `ArrowCraftInfo.arrowTipIds`.
pub fn tip_ids() -> Vec<i32> {
    ARROWS.iter().map(|arrow| arrow.tip).collect()
}
/// `ArrowCraftInfo.fromTipId(id)`, as an index.
pub fn from_tip(id: i32) -> Option<usize> {
    ARROWS.iter().position(|arrow| arrow.tip == id)
}

/// One of the four crafts: what a handler creates and its script runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// `StringItemScript` of `STRINGABLES[n]`.
    String(usize),
    /// `HeadlessArrowCraftScript` with the feather used.
    Headless(i32),
    /// `TippedArrowCraftScript` of `ARROWS[n]`.
    Tipped(usize),
    /// `FlightedOgreArrowCraftScript` with the feather used.
    Ogre(i32),
}
impl Job {
    /// The script's `queueScript` lambda.
    pub fn queued_lambda(self) -> &'static str {
        match self {
            Self::String(_) => "content.global.skill.fletching.stringing.StringItemScript$invoke$1",
            Self::Headless(_) => {
                "content.global.skill.fletching.arrow.HeadlessArrowCraftScript$invoke$1"
            }
            Self::Tipped(_) => {
                "content.global.skill.fletching.arrow.TippedArrowCraftScript$invoke$1"
            }
            Self::Ogre(_) => {
                "content.global.skill.fletching.arrow.FlightedOgreArrowCraftScript$invoke$1"
            }
        }
    }
    /// The item the handler shows.
    pub fn product(self) -> Result<i32> {
        Ok(match self {
            Self::String(n) => STRINGABLES.get(n).ok_or(Error::Index)?.strung,
            Self::Headless(_) => HEADLESS_ARROW,
            Self::Tipped(n) => ARROWS.get(n).ok_or(Error::Index)?.arrow,
            Self::Ogre(_) => FLIGHTED_OGRE_ARROW,
        })
    }
}

impl Job {
    /// The handler's `SkillDialogue`.
    pub fn kind(self) -> SkillDialogue {
        match self {
            Self::String(_) => SkillDialogue::OneOption,
            _ => SkillDialogue::MakeSetOneOption,
        }
    }
}

/// The four `onUseWith(IntType.ITEM, ...)` lambdas, in registration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listener {
    /// A string with an unstrung bow.
    Stringing,
    /// An arrow shaft with a feather.
    Headless,
    /// A headless arrow with arrow tips.
    Tipped,
    /// An ogre arrow shaft with a feather.
    Ogre,
}
impl Listener {
    pub const ALL: [Self; 4] = [Self::Stringing, Self::Headless, Self::Tipped, Self::Ogre];
    /// The ids the listener registers: the used ones, then the others.
    pub fn ids(self) -> (Vec<i32>, Vec<i32>) {
        match self {
            Self::Stringing => (STRINGS.to_vec(), unstrung_ids()),
            Self::Headless => (vec![ARROW_SHAFT], FEATHERS.to_vec()),
            Self::Tipped => (vec![HEADLESS_ARROW], tip_ids()),
            Self::Ogre => (vec![OGRE_ARROW_SHAFT], FEATHERS.to_vec()),
        }
    }
    /// The `onUseWith` lambda.
    pub fn lambda(self) -> &'static str {
        match self {
            Self::Stringing => STRINGING_LAMBDA,
            Self::Headless => HEADLESS_LAMBDA,
            Self::Tipped => TIPPED_LAMBDA,
            Self::Ogre => OGRE_LAMBDA,
        }
    }
}

/// A lambda up to its handler: `None` is the lambda's `false`, `Some(None)`
/// its `true` without a handler and `Some(Some(job))` the job whose handler
/// opens.
pub fn use_with<H: Host>(
    host: &mut H,
    listener: Listener,
    used: i32,
    with: i32,
) -> Result<Option<Option<Job>>> {
    Ok(match listener {
        Listener::Stringing => match string_use(host, used, with)? {
            None => None,
            Some(None) => Some(None),
            Some(Some(bow)) => Some(Some(Job::String(bow))),
        },
        Listener::Headless => Some(Some(Job::Headless(with))),
        Listener::Tipped => from_tip(with).map(|arrow| Some(Job::Tipped(arrow))),
        Listener::Ogre => Some(Some(Job::Ogre(with))),
    })
}

/// What the listeners and their scripts ask of the world, for their one
/// player.
pub trait Host {
    /// `getDynLevel(player, Skills.FLETCHING)`.
    fn level(&mut self) -> Result<i32>;
    /// `sendDialogue(player, message)`.
    fn dialogue(&mut self, message: &str) -> Result<()>;
    /// `sendMessage(player, message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// `getQuestStage(player, quest)`.
    fn quest_stage(&mut self, quest: Quest) -> Result<i32>;
    /// `getSlayerFlags(player).isBroadsUnlocked()`.
    fn broads_unlocked(&mut self) -> Result<bool>;
    /// `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32>;
    /// `hasSpaceFor(player, Item(id))`.
    fn has_space_for(&mut self, id: i32) -> Result<bool>;
    /// `queueScript(player, delay) { ... }` (WEAK, not persistent) of the
    /// job's script with its amount; the script is [`script`].
    fn queue(&mut self, delay: i32, job: Job, amount: i32) -> Result<()>;
    /// `clockReady(player, Clocks.SKILLING)`.
    fn clock_ready(&mut self) -> Result<bool>;
    /// `keepRunning(player)`.
    fn keep_running(&mut self) -> Result<bool>;
    /// `stopExecuting(player)`.
    fn stop_executing(&mut self) -> Result<bool>;
    /// `removeItem(player, Item(id, amount))` (from the inventory).
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `player.animate(Animation(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `addItem(player, id, amount)`; the scripts ignore the answer.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `rewardXP(player, Skills.FLETCHING, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// The player's location.
    fn location(&mut self) -> Result<[i32; 3]>;
    /// `getAttribute(player, key, false)`.
    fn attribute(&mut self, key: &str) -> Result<bool>;
    /// `achievementDiaryManager.finishTask(player, DiaryType.SEERS_VILLAGE,
    /// level, index)`.
    fn finish_seers_task(&mut self, level: i32, index: i32) -> Result<()>;
    /// `delayClock(player, Clocks.SKILLING, ticks, true)`.
    fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool>;
}

/// `StringingListeners.sendLevelCheckFailDialogue(player, level)`.
fn string_level_fail<H: Host>(host: &mut H, level: i32) -> Result<()> {
    host.dialogue(&format!(
        "You need a fletching level of {level} to string this bow."
    ))
}
/// `ArrowListeners.sendLevelCheckFailDialog(player, level)`.
fn arrow_level_fail<H: Host>(host: &mut H, level: i32) -> Result<()> {
    host.dialogue(&format!(
        "You need a fletching level of {level} to do this."
    ))
}

/// The stringing lambda before its handler: `None` is the lambda's `false`
/// (no craft for the bow), `Some(None)` its `true` after a message and
/// `Some(Some(n))` the craft whose handler opens.
pub fn string_use<H: Host>(
    host: &mut H,
    string: i32,
    unstrung: i32,
) -> Result<Option<Option<usize>>> {
    let Some(at) = for_unstrung(unstrung) else {
        return Ok(None);
    };
    let bow = &STRINGABLES[at];
    if string != bow.string() {
        host.message("That's not the right kind of string for this.")?;
        return Ok(Some(None));
    }
    if bow.name == "OGRE_COMP_BOW" && host.quest_stage(Quest::ZogreFleshEaters)? < 8 {
        host.message("You must have started Zogre Flesh Eaters and asked Grish to string this.")?;
        return Ok(Some(None));
    }
    Ok(Some(Some(at)))
}

/// A handler's `create(amount, index)`.
pub fn create<H: Host>(host: &mut H, job: Job, amount: i32) -> Result<()> {
    const NO_SPACE: &str = "You do not have enough inventory space.";
    match job {
        Job::String(n) => {
            let bow = STRINGABLES.get(n).ok_or(Error::Index)?;
            if host.level()? < bow.level {
                return string_level_fail(host, bow.level);
            }
        }
        Job::Headless(_) => {
            if !host.has_space_for(HEADLESS_ARROW)? {
                return host.dialogue(NO_SPACE);
            }
        }
        Job::Tipped(n) => {
            let arrow = ARROWS.get(n).ok_or(Error::Index)?;
            if arrow.name == "BROAD_ARROW" && !host.broads_unlocked()? {
                return host.dialogue("You need to unlock the ability to create broad arrows.");
            }
            if host.level()? < arrow.level {
                return arrow_level_fail(host, arrow.level);
            }
            if !host.has_space_for(arrow.arrow)? {
                return host.dialogue(NO_SPACE);
            }
        }
        Job::Ogre(_) => {
            if host.level()? < FLIGHTED_OGRE_ARROW_LEVEL {
                return arrow_level_fail(host, FLIGHTED_OGRE_ARROW_LEVEL);
            }
            if !host.has_space_for(FLIGHTED_OGRE_ARROW)? {
                return host.dialogue(NO_SPACE);
            }
        }
    }
    host.queue(INITIAL_DELAY, job, amount)
}

/// A handler's `getAll(index)`: the amount of the string used, of headless
/// arrows (the first arrow handler counts its product), of the tip used and
/// of ogre arrow shafts.
pub fn get_all<H: Host>(host: &mut H, job: Job, string: i32) -> Result<i32> {
    let id = match job {
        Job::String(_) => string,
        Job::Headless(_) => HEADLESS_ARROW,
        Job::Tipped(n) => ARROWS.get(n).ok_or(Error::Index)?.tip,
        Job::Ogre(_) => OGRE_ARROW_SHAFT,
    };
    host.amount_in_inventory(id)
}

/// `removeItemsIfPlayerHasEnough(player, items...)`: one of each id first,
/// then each removal in turn.
fn remove_if_enough<H: Host>(host: &mut H, items: &[(i32, i32)]) -> Result<bool> {
    for (id, _) in items {
        if host.amount_in_inventory(*id)? < 1 {
            return Ok(false);
        }
    }
    for (id, amount) in items {
        if !host.remove_item(*id, *amount)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `handleSeersMagicShortbowAchievement()` of StringItemScript.
fn seers_magic_shortbow<H: Host>(host: &mut H) -> Result<()> {
    let [x, y, _] = host.location()?;
    let inside = SEERS_ZONES
        .iter()
        .any(|[x1, y1, x2, y2]| *x1 <= x && *y1 <= y && *x2 >= x && *y2 >= y);
    if inside && host.attribute(ATTRIBUTE_MAGIC_SHORTBOW)? {
        host.finish_seers_task(2, 2)?;
    }
    Ok(())
}

/// The queued script of a job's `invoke()` at `stage`.
pub fn script<H: Host>(host: &mut H, job: Job, amount: i32, stage: i32) -> Result<bool> {
    if !host.clock_ready()? {
        return host.keep_running();
    }
    let delay = match job {
        Job::String(n) => {
            let bow = *STRINGABLES.get(n).ok_or(Error::Index)?;
            if host.level()? < bow.level {
                string_level_fail(host, bow.level)?;
                return host.stop_executing();
            }
            if !remove_if_enough(host, &[(bow.unstrung, 1), (bow.string(), 1)])? {
                return host.stop_executing();
            }
            host.animate(bow.animation)?;
            host.add_item(bow.strung, 1)?;
            host.reward_xp(bow.experience)?;
            host.message("You add a string to the bow.")?;
            if bow.name == "MAGIC_SHORTBOW" {
                seers_magic_shortbow(host)?;
            }
            2
        }
        Job::Headless(feather) => {
            let feathers = host.amount_in_inventory(feather)?;
            let shafts = host.amount_in_inventory(ARROW_SHAFT)?;
            let made = shafts.min(feathers).min(15);
            if !remove_if_enough(host, &[(ARROW_SHAFT, made), (feather, made)])? {
                return host.stop_executing();
            }
            if made == 1 {
                host.message("You attach a feather to a shaft.")?;
            } else {
                host.message(&format!("You attach feathers to {made} arrow shafts."))?;
            }
            host.reward_xp(f64::from(made))?;
            host.add_item(HEADLESS_ARROW, made)?;
            3
        }
        Job::Tipped(n) => {
            let arrow = *ARROWS.get(n).ok_or(Error::Index)?;
            if host.level()? < arrow.level {
                arrow_level_fail(host, arrow.level)?;
                return host.stop_executing();
            }
            let tips = host.amount_in_inventory(arrow.tip)?;
            let headless = host.amount_in_inventory(HEADLESS_ARROW)?;
            let made = headless.min(tips).min(15);
            if !remove_if_enough(host, &[(HEADLESS_ARROW, made), (arrow.tip, made)])? {
                return host.stop_executing();
            }
            if made == 1 {
                host.message("You attach an arrow head to an arrow shaft.")?;
            } else {
                host.message(&format!("You attach arrow heads to {made} arrow shafts."))?;
            }
            host.reward_xp(arrow.experience * f64::from(made))?;
            host.add_item(arrow.arrow, made)?;
            3
        }
        Job::Ogre(feather) => {
            if host.level()? < FLIGHTED_OGRE_ARROW_LEVEL {
                arrow_level_fail(host, FLIGHTED_OGRE_ARROW_LEVEL)?;
                return host.stop_executing();
            }
            let feathers = host.amount_in_inventory(feather)?;
            let shafts = host.amount_in_inventory(OGRE_ARROW_SHAFT)?;
            // Four feathers an arrow, six arrows a stage.
            let made = shafts.min(feathers / 4).min(6);
            let used = made.wrapping_mul(4);
            if !remove_if_enough(host, &[(OGRE_ARROW_SHAFT, made), (feather, used)])? {
                return host.stop_executing();
            }
            if made == 1 {
                host.message("You attach 4 feathers to a shaft.")?;
            } else {
                host.message(&format!(
                    "You attach {used} feathers to {made} arrow shafts."
                ))?;
            }
            host.reward_xp(f64::from(made))?;
            host.add_item(FLIGHTED_OGRE_ARROW, made)?;
            3
        }
    };
    if stage >= amount.wrapping_sub(1) {
        return host.stop_executing();
    }
    host.delay_clock_and_script(delay)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct World {
        calls: Vec<String>,
        level: i32,
        held: HashMap<i32, i32>,
        clock_ready: bool,
        space: bool,
        at: [i32; 3],
        fletched: bool,
        zogre: i32,
        broads: bool,
    }
    fn world(level: i32, held: &[(i32, i32)]) -> World {
        World {
            calls: Vec::new(),
            level,
            held: held.iter().copied().collect(),
            clock_ready: true,
            space: true,
            at: [3200, 3200, 0],
            fletched: false,
            zogre: 0,
            broads: false,
        }
    }
    impl Host for World {
        fn level(&mut self) -> Result<i32> {
            Ok(self.level)
        }
        fn dialogue(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("dialogue {message}"));
            Ok(())
        }
        fn message(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("message {message}"));
            Ok(())
        }
        fn quest_stage(&mut self, _: Quest) -> Result<i32> {
            Ok(self.zogre)
        }
        fn broads_unlocked(&mut self) -> Result<bool> {
            Ok(self.broads)
        }
        fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
            Ok(self.held.get(&id).copied().unwrap_or(0))
        }
        fn has_space_for(&mut self, _: i32) -> Result<bool> {
            Ok(self.space)
        }
        fn queue(&mut self, delay: i32, job: Job, amount: i32) -> Result<()> {
            self.calls.push(format!("queue {delay} {job:?} {amount}"));
            Ok(())
        }
        fn clock_ready(&mut self) -> Result<bool> {
            Ok(self.clock_ready)
        }
        fn keep_running(&mut self) -> Result<bool> {
            self.calls.push("keepRunning".into());
            Ok(false)
        }
        fn stop_executing(&mut self) -> Result<bool> {
            self.calls.push("stopExecuting".into());
            Ok(true)
        }
        fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool> {
            let held = self.held.entry(id).or_insert(0);
            if *held < amount {
                return Ok(false);
            }
            *held -= amount;
            self.calls.push(format!("remove {id} x{amount}"));
            Ok(true)
        }
        fn animate(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("animate {id}"));
            Ok(())
        }
        fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
            self.calls.push(format!("add {id} x{amount}"));
            Ok(true)
        }
        fn reward_xp(&mut self, xp: f64) -> Result<()> {
            self.calls.push(format!("xp {xp}"));
            Ok(())
        }
        fn location(&mut self) -> Result<[i32; 3]> {
            Ok(self.at)
        }
        fn attribute(&mut self, _: &str) -> Result<bool> {
            Ok(self.fletched)
        }
        fn finish_seers_task(&mut self, level: i32, index: i32) -> Result<()> {
            self.calls.push(format!("finishTask {level} {index}"));
            Ok(())
        }
        fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool> {
            self.calls.push(format!("delay {ticks}"));
            Ok(false)
        }
    }
    fn bow_at(name: &str) -> usize {
        STRINGABLES.iter().position(|b| b.name == name).unwrap()
    }

    #[test]
    fn the_tables() {
        assert_eq!(unstrung_ids().len(), 20);
        assert_eq!(for_unstrung(50), Some(0));
        assert_eq!(for_unstrung(9465), Some(19));
        assert_eq!(for_unstrung(51), None);
        assert_eq!(STRINGABLES[0].string(), BOW_STRING);
        assert_eq!(STRINGABLES[13].string(), CROSSBOW_STRING);
        assert_eq!(tip_ids(), [39, 40, 41, 42, 43, 44, 11237, 13278]);
        assert_eq!(from_tip(44), Some(5));
        assert_eq!(from_tip(45), None);
        assert_eq!(Job::Tipped(5).product(), Ok(892));
        assert_eq!(Job::String(99).product(), Err(Error::Index));
    }

    #[test]
    fn stringing_asks_for_the_right_string_and_the_quest() {
        let mut w = world(99, &[]);
        assert_eq!(string_use(&mut w, BOW_STRING, 1), Ok(None));
        assert_eq!(string_use(&mut w, CROSSBOW_STRING, 50), Ok(Some(None)));
        assert_eq!(
            w.calls,
            ["message That's not the right kind of string for this."]
        );
        w.calls.clear();
        assert_eq!(string_use(&mut w, BOW_STRING, 50), Ok(Some(Some(0))));
        assert_eq!(string_use(&mut w, BOW_STRING, 4825), Ok(Some(None)));
        assert_eq!(
            w.calls,
            ["message You must have started Zogre Flesh Eaters and asked Grish to string this."]
        );
        w.zogre = 8;
        assert_eq!(
            string_use(&mut w, BOW_STRING, 4825),
            Ok(Some(Some(bow_at("OGRE_COMP_BOW"))))
        );
    }

    #[test]
    fn create_checks_before_it_queues() {
        let mut w = world(4, &[]);
        assert_eq!(create(&mut w, Job::String(0), 3), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You need a fletching level of 5 to string this bow."]
        );
        let mut w = world(5, &[]);
        assert_eq!(create(&mut w, Job::String(0), 3), Ok(()));
        assert_eq!(w.calls, ["queue 1 String(0) 3"]);
        let mut w = world(1, &[]);
        w.space = false;
        assert_eq!(create(&mut w, Job::Headless(314), 10), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You do not have enough inventory space."]
        );
        // Broad arrows ask the slayer unlock before the level.
        let mut w = world(1, &[]);
        assert_eq!(create(&mut w, Job::Tipped(7), 1), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You need to unlock the ability to create broad arrows."]
        );
        let mut w = world(51, &[]);
        w.broads = true;
        assert_eq!(create(&mut w, Job::Tipped(7), 1), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You need a fletching level of 52 to do this."]
        );
        let mut w = world(4, &[]);
        assert_eq!(create(&mut w, Job::Ogre(314), 1), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You need a fletching level of 5 to do this."]
        );
        let mut w = world(5, &[]);
        assert_eq!(create(&mut w, Job::Ogre(314), 5), Ok(()));
        assert_eq!(w.calls, ["queue 1 Ogre(314) 5"]);
    }

    #[test]
    fn get_all_counts_what_the_kotlin_counts() {
        let mut w = world(1, &[(1777, 4), (53, 7), (40, 9), (2864, 2)]);
        assert_eq!(get_all(&mut w, Job::String(0), 1777), Ok(4));
        assert_eq!(get_all(&mut w, Job::Headless(314), 0), Ok(7));
        assert_eq!(get_all(&mut w, Job::Tipped(1), 0), Ok(9));
        assert_eq!(get_all(&mut w, Job::Ogre(314), 0), Ok(2));
    }

    #[test]
    fn the_string_script() {
        let mut w = world(5, &[(50, 2), (1777, 1)]);
        assert_eq!(script(&mut w, Job::String(0), 2, 0), Ok(false));
        assert_eq!(
            w.calls,
            [
                "remove 50 x1",
                "remove 1777 x1",
                "animate 6678",
                "add 841 x1",
                "xp 5",
                "message You add a string to the bow.",
                "delay 2",
            ]
        );
        // No string left.
        w.calls.clear();
        assert_eq!(script(&mut w, Job::String(0), 2, 1), Ok(true));
        assert_eq!(w.calls, ["stopExecuting"]);
        let mut w = world(4, &[(50, 1), (1777, 1)]);
        assert_eq!(script(&mut w, Job::String(0), 1, 0), Ok(true));
        assert_eq!(
            w.calls,
            [
                "dialogue You need a fletching level of 5 to string this bow.",
                "stopExecuting"
            ]
        );
        let mut w = world(5, &[(50, 1), (1777, 1)]);
        w.clock_ready = false;
        assert_eq!(script(&mut w, Job::String(0), 1, 0), Ok(false));
        assert_eq!(w.calls, ["keepRunning"]);
    }

    #[test]
    fn the_magic_shortbow_finishes_the_seers_task() {
        let at = bow_at("MAGIC_SHORTBOW");
        let mut w = world(80, &[(72, 1), (1777, 1)]);
        (w.at, w.fletched) = ([2722, 3490, 0], true);
        assert_eq!(script(&mut w, Job::String(at), 1, 0), Ok(true));
        assert!(w.calls.contains(&"finishTask 2 2".to_owned()));
        // Not fletched there first, or strung elsewhere: no task.
        let mut w = world(80, &[(72, 1), (1777, 1)]);
        w.at = [2722, 3490, 0];
        assert_eq!(script(&mut w, Job::String(at), 1, 0), Ok(true));
        assert!(!w.calls.contains(&"finishTask 2 2".to_owned()));
        let mut w = world(80, &[(72, 1), (1777, 1)]);
        w.fletched = true;
        assert_eq!(script(&mut w, Job::String(at), 1, 0), Ok(true));
        assert!(!w.calls.contains(&"finishTask 2 2".to_owned()));
    }

    #[test]
    fn headless_arrows_come_in_sets_of_fifteen() {
        let mut w = world(1, &[(52, 16), (314, 40)]);
        assert_eq!(script(&mut w, Job::Headless(314), 10, 0), Ok(false));
        assert_eq!(
            w.calls,
            [
                "remove 52 x15",
                "remove 314 x15",
                "message You attach feathers to 15 arrow shafts.",
                "xp 15",
                "add 53 x15",
                "delay 3",
            ]
        );
        w.calls.clear();
        assert_eq!(script(&mut w, Job::Headless(314), 10, 1), Ok(false));
        assert_eq!(
            w.calls[..3],
            [
                "remove 52 x1",
                "remove 314 x1",
                "message You attach a feather to a shaft."
            ]
        );
        w.calls.clear();
        assert_eq!(script(&mut w, Job::Headless(314), 10, 2), Ok(true));
        assert_eq!(w.calls, ["stopExecuting"]);
    }

    #[test]
    fn tipped_arrows_scale_their_experience() {
        let mut w = world(15, &[(53, 10), (40, 30)]);
        assert_eq!(script(&mut w, Job::Tipped(1), 1, 0), Ok(true));
        assert_eq!(
            w.calls,
            [
                "remove 53 x10",
                "remove 40 x10",
                "message You attach arrow heads to 10 arrow shafts.",
                "xp 25",
                "add 884 x10",
                "stopExecuting",
            ]
        );
        let mut w = world(14, &[(53, 10), (40, 30)]);
        assert_eq!(script(&mut w, Job::Tipped(1), 1, 0), Ok(true));
        assert_eq!(
            w.calls,
            [
                "dialogue You need a fletching level of 15 to do this.",
                "stopExecuting"
            ]
        );
    }

    #[test]
    fn ogre_arrows_take_four_feathers_each() {
        let mut w = world(5, &[(2864, 10), (314, 30)]);
        assert_eq!(script(&mut w, Job::Ogre(314), 2, 0), Ok(false));
        assert_eq!(
            w.calls,
            [
                "remove 2864 x6",
                "remove 314 x24",
                "message You attach 24 feathers to 6 arrow shafts.",
                "xp 6",
                "add 2865 x6",
                "delay 3",
            ]
        );
        w.calls.clear();
        assert_eq!(script(&mut w, Job::Ogre(314), 2, 1), Ok(true));
        assert_eq!(
            w.calls[..3],
            [
                "remove 2864 x1",
                "remove 314 x4",
                "message You attach 4 feathers to a shaft."
            ]
        );
    }
}
