//! `content.global.skill.fletching.log`: a knife used on logs
//! (`LogCraftableListeners`, `CraftItemWithLogScript`, `LogCraftInfo`,
//! `GrammarHelpers`) with the zones and attribute key of
//! `content.global.skill.fletching` they read.
//!
//! Ported from the pinned sources without a JVM corpus: the unit tests below
//! check the calls against the Kotlin by reading, not against a recorded
//! run. Every outside call is a [`Host`] method.
//!
//! Covered: the craft table, the listener's choice of items for a log, the
//! handler's `create` checks ([`create`]), its `getAll` and the queued
//! script's body ([`script`]). The SkillDialogueHandler itself is
//! `dialogue_dsl::skill`'s. Not covered: the rest of fletching (stringing,
//! arrows, bolts, darts, crossbows, gems, grapples).

/// `Skills.FLETCHING`.
pub const FLETCHING: i32 = 9;
/// `Clocks.SKILLING`.
pub const SKILLING: i32 = 7;
pub const KNIFE: i32 = 946;
pub const WOLF_BONES: i32 = 2859;
/// `carveLogAnimation`.
pub const ANIMATION: i32 = 1248;
/// `initialDelay` and `subsequentDelay`.
pub const INITIAL_DELAY: i32 = 1;
pub const SUBSEQUENT_DELAY: i32 = 3;
/// `"/save:" + AchievementDiaryAttributeKeys.FLETCHED_UNSTRUNG_MAGIC_SHORTBOW`.
pub const ATTRIBUTE_MAGIC_SHORTBOW: &str = "/save:diary:seers:fletch-magic-short-bow";
/// `Zones.seersMagicShortbowAchievementZones`, as `ZoneBorders` holds them
/// (corners ordered; the first two name plane 0 without a zero plane check,
/// so no plane is compared for any of the three).
pub const SEERS_ZONES: [[i32; 4]; 3] = [
    [2721, 3489, 2724, 3493],
    [2727, 3487, 2730, 3490],
    [2721, 3487, 2730, 3493],
];
/// The listener's class (`scan.contentInterfaces`).
pub const LISTENER_CLASS: &str = "content.global.skill.fletching.log.LogCraftableListeners";
/// The `onUseWith` lambda.
pub const LAMBDA: &str =
    "content.global.skill.fletching.log.LogCraftableListeners$defineListeners$1";
/// The script's `queueScript` lambda.
pub const QUEUED_LAMBDA: &str =
    "content.global.skill.fletching.log.CraftItemWithLogScript$invoke$1";

/// A `LogCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Craft {
    pub name: &'static str,
    pub log: i32,
    pub finished: i32,
    pub experience: f64,
    pub level: i32,
}
const fn craft(name: &'static str, log: i32, finished: i32, experience: f64, level: i32) -> Craft {
    Craft {
        name,
        log,
        finished,
        experience,
        level,
    }
}

/// `LogCraftInfo.values()`.
pub const CRAFTS: [Craft; 22] = [
    craft("ARROW_SHAFT", 1511, 52, 5.0, 1),
    craft("SHORT_BOW", 1511, 50, 5.0, 5),
    craft("LONG_BOW", 1511, 48, 10.0, 10),
    craft("WOODEN_STOCK", 1511, 9440, 6.0, 9),
    craft("OGRE_ARROW_SHAFT", 2862, 2864, 6.4, 5),
    craft("OGRE_COMP_BOW", 2862, 4825, 45.0, 30),
    craft("OAK_SHORTBOW", 1521, 54, 16.5, 20),
    craft("OAK_LONGBOW", 1521, 56, 25.0, 25),
    craft("OAK_STOCK", 1521, 9442, 16.0, 24),
    craft("WILLOW_SHORTBOW", 1519, 60, 33.3, 35),
    craft("WILLOW_LONGBOW", 1519, 58, 41.5, 40),
    craft("WILLOW_STOCK", 1519, 9444, 22.0, 39),
    craft("MAPLE_SHORTBOW", 1517, 64, 50.0, 50),
    craft("MAPLE_LONGBOW", 1517, 62, 58.3, 55),
    craft("MAPLE_STOCK", 1517, 9448, 32.0, 54),
    craft("YEW_SHORTBOW", 1515, 68, 67.5, 65),
    craft("YEW_LONGBOW", 1515, 66, 75.0, 70),
    craft("YEW_STOCK", 1515, 9452, 50.0, 69),
    craft("MAGIC_SHORTBOW", 1513, 72, 83.3, 80),
    craft("MAGIC_LONGBOW", 1513, 70, 91.5, 85),
    craft("TEAK_STOCK", 6333, 9446, 27.0, 46),
    craft("MAHOGANY_STOCK", 6332, 9450, 41.0, 61),
];

/// `LogCraftInfo.logIds`: the distinct log ids in declaration order.
pub fn log_ids() -> Vec<i32> {
    let mut ids = Vec::new();
    for craft in &CRAFTS {
        if !ids.contains(&craft.log) {
            ids.push(craft.log);
        }
    }
    ids
}

/// `LogCraftInfo.forLogId(logId)`: the constants of a log, by their index in
/// [`CRAFTS`].
pub fn for_log_id(log: i32) -> Option<Vec<usize>> {
    let found: Vec<usize> = (0..CRAFTS.len())
        .filter(|at| CRAFTS[*at].log == log)
        .collect();
    (!found.is_empty()).then_some(found)
}

/// The quests the listener asks about, with their index in the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quest {
    BigChompyBirdHunting,
    ZogreFleshEaters,
}
impl Quest {
    /// `Quests`' name of the quest.
    pub fn name(self) -> &'static str {
        match self {
            Self::BigChompyBirdHunting => "Big Chompy Bird Hunting",
            Self::ZogreFleshEaters => "Zogre Flesh Eaters",
        }
    }
    /// The quest class's index (ChompyBird.kt, ZogreFleshEaters.kt).
    pub fn index(self) -> usize {
        match self {
            Self::BigChompyBirdHunting => 35,
            Self::ZogreFleshEaters => 40,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// An IndexOutOfBoundsException (a list index, or `charAt(0)` of an
    /// empty name).
    Index,
    /// A ClassCastException.
    Cast,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// What the listener and its script ask of the world, for their one player.
pub trait Host {
    /// `getDynLevel(player, Skills.FLETCHING)`.
    fn level(&mut self) -> Result<i32>;
    /// `Item(id).name`.
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `sendDialogue(player, message)`.
    fn dialogue(&mut self, message: &str) -> Result<()>;
    /// `sendMessage(player, message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// `getQuestStage(player, quest)`.
    fn quest_stage(&mut self, quest: Quest) -> Result<i32>;
    /// `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32>;
    /// `queueScript(player, delay) { ... }` (WEAK, not persistent) of a
    /// `CraftItemWithLogScript(player, CRAFTS[craft], amount)`; the script
    /// is [`script`].
    fn queue(&mut self, delay: i32, craft: usize, amount: i32) -> Result<()>;
    /// `clockReady(player, Clocks.SKILLING)`.
    fn clock_ready(&mut self) -> Result<bool>;
    /// `keepRunning(player)`.
    fn keep_running(&mut self) -> Result<bool>;
    /// `stopExecuting(player)`.
    fn stop_executing(&mut self) -> Result<bool>;
    /// `removeItem(player, id)` (one, from the inventory).
    fn remove_item(&mut self, id: i32) -> Result<bool>;
    /// `player.animate(Animation(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `addItem(player, id, amount)`; the script ignores the answer.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `rewardXP(player, Skills.FLETCHING, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// The player's location.
    fn location(&mut self) -> Result<[i32; 3]>;
    /// `achievementDiaryManager.hasCompletedTask(DiaryType.SEERS_VILLAGE,
    /// level, index)`.
    fn seers_task_complete(&mut self, level: i32, index: i32) -> Result<bool>;
    /// `setAttribute(player, key, true)`.
    fn set_attribute_true(&mut self, key: &str) -> Result<()>;
    /// `delayClock(player, Clocks.SKILLING, ticks, true)`.
    fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool>;
}

/// `GrammarHelpers.makeFriendlyName(name)`: without "(u)", then Kotlin's
/// `trim { it <= ' ' }`.
pub fn make_friendly_name(name: &str) -> String {
    name.replace("(u)", "")
        .trim_matches(|c: char| c <= ' ')
        .to_owned()
}

/// `GrammarHelpers.aOrAn(word)`: `StringUtils.isPlusN`.
pub fn a_or_an(word: &str) -> Result<&'static str> {
    let lower = word.to_lowercase();
    let mut chars = lower.chars();
    // `s.charAt(0)` of an empty word throws.
    let first = chars.next().ok_or(Error::Index)?;
    let plus_n = matches!(first, 'a' | 'e' | 'i' | 'o' | 'u')
        || (first == 'h' && chars.next().is_some_and(|second| second != 'e'));
    Ok(if plus_n { "an" } else { "a" })
}

/// `LogCraftableListeners.sendLevelCheckFailDialog(player, logCraftInfo)`.
pub fn send_level_check_fail_dialog<H: Host>(host: &mut H, craft: &Craft) -> Result<()> {
    let name = make_friendly_name(&host.item_name(craft.finished)?);
    let article = a_or_an(&name)?;
    host.dialogue(&format!(
        "You need a Fletching skill of {} or above to make {article} {name}.",
        craft.level
    ))
}

/// The handler's private `playerMeetsInitialRequirements(logCraftInfo)`.
fn meets_initial_requirements<H: Host>(host: &mut H, craft: &Craft) -> Result<bool> {
    if host.level()? < craft.level {
        send_level_check_fail_dialog(host, craft)?;
        return Ok(false);
    }
    if craft.name == "OGRE_ARROW_SHAFT" && host.quest_stage(Quest::BigChompyBirdHunting)? <= 0 {
        host.dialogue(&format!(
            "You must have started {} to make those.",
            Quest::BigChompyBirdHunting.name()
        ))?;
        return Ok(false);
    }
    if craft.name == "OGRE_COMP_BOW" {
        if host.quest_stage(Quest::ZogreFleshEaters)? < 8 {
            host.message(
                "You must have started Zogre Flesh Eaters and asked Grish to string this.",
            )?;
            return Ok(false);
        }
        if host.amount_in_inventory(WOLF_BONES)? < 1 {
            host.message("You need to have Wolf Bones in order to make this.")?;
            return Ok(false);
        }
    }
    Ok(true)
}

/// The handler's `create(amount, index)` for the log's craft list.
pub fn create<H: Host>(host: &mut H, crafts: &[usize], amount: i32, index: i32) -> Result<()> {
    let at = usize::try_from(index).or(Err(Error::Index))?;
    let craft = *crafts.get(at).ok_or(Error::Index)?;
    if !meets_initial_requirements(host, &CRAFTS[craft])? {
        return Ok(());
    }
    // CraftItemWithLogScript(player, info, amount).invoke().
    host.queue(INITIAL_DELAY, craft, amount)
}

/// `handleSeersMagicShortbowAchievement()`.
fn seers_magic_shortbow<H: Host>(host: &mut H) -> Result<()> {
    let [x, y, _] = host.location()?;
    let inside = SEERS_ZONES
        .iter()
        .any(|[x1, y1, x2, y2]| *x1 <= x && *y1 <= y && *x2 >= x && *y2 >= y);
    if inside && !host.seers_task_complete(2, 2)? {
        host.set_attribute_true(ATTRIBUTE_MAGIC_SHORTBOW)?;
    }
    Ok(())
}

/// The queued script of `CraftItemWithLogScript.invoke()` for `CRAFTS[craft]`
/// at `stage`.
pub fn script<H: Host>(host: &mut H, craft: usize, amount: i32, stage: i32) -> Result<bool> {
    let craft = CRAFTS.get(craft).ok_or(Error::Index)?;
    if !host.clock_ready()? {
        return host.keep_running();
    }
    if host.level()? < craft.level {
        send_level_check_fail_dialog(host, craft)?;
        return host.stop_executing();
    }
    if !host.remove_item(craft.log)? {
        return host.stop_executing();
    }
    host.animate(ANIMATION)?;
    let name = make_friendly_name(&host.item_name(craft.finished)?);
    let made = match craft.name {
        // RandomFunction.random(2, 6): `2 + random(4)`.
        "OGRE_ARROW_SHAFT" => 2i32.wrapping_add(host.random(4)?),
        "ARROW_SHAFT" => 15,
        _ => 1,
    };
    if craft.name == "OGRE_COMP_BOW" && !host.remove_item(WOLF_BONES)? {
        return host.stop_executing();
    }
    match craft.name {
        "ARROW_SHAFT" | "OGRE_ARROW_SHAFT" => {
            host.message(&format!(
                "You carefully cut the wood into {made} arrow shafts."
            ))?;
        }
        _ => {
            let article = a_or_an(&name)?;
            host.message(&format!(
                "You carefully cut the wood into {article} {name}."
            ))?;
        }
    }
    host.add_item(craft.finished, made)?;
    host.reward_xp(craft.experience)?;
    if craft.name == "MAGIC_SHORTBOW" {
        seers_magic_shortbow(host)?;
    }
    if stage >= amount.wrapping_sub(1) {
        return host.stop_executing();
    }
    host.delay_clock_and_script(SUBSEQUENT_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        calls: Vec<String>,
        level: i32,
        logs: i32,
        clock_ready: bool,
        at: [i32; 3],
        task_done: bool,
        chompy: i32,
        zogre: i32,
        wolf_bones: i32,
    }
    fn world(level: i32, logs: i32) -> World {
        World {
            calls: Vec::new(),
            level,
            logs,
            clock_ready: true,
            at: [3200, 3200, 0],
            task_done: false,
            chompy: 0,
            zogre: 0,
            wolf_bones: 0,
        }
    }
    impl Host for World {
        fn level(&mut self) -> Result<i32> {
            Ok(self.level)
        }
        fn item_name(&mut self, id: i32) -> Result<String> {
            Ok(match id {
                52 => "Arrow shaft",
                50 => "Shortbow (u)",
                54 => "Oak shortbow (u)",
                72 => "Magic shortbow (u)",
                2864 => "Ogre arrow shaft",
                4825 => "Unstrung comp bow",
                _ => "Thing",
            }
            .to_owned())
        }
        fn dialogue(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("dialogue {message}"));
            Ok(())
        }
        fn message(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("message {message}"));
            Ok(())
        }
        fn quest_stage(&mut self, quest: Quest) -> Result<i32> {
            Ok(match quest {
                Quest::BigChompyBirdHunting => self.chompy,
                Quest::ZogreFleshEaters => self.zogre,
            })
        }
        fn amount_in_inventory(&mut self, _: i32) -> Result<i32> {
            Ok(self.wolf_bones)
        }
        fn queue(&mut self, delay: i32, craft: usize, amount: i32) -> Result<()> {
            self.calls
                .push(format!("queue {delay} {} {amount}", CRAFTS[craft].name));
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
        fn remove_item(&mut self, id: i32) -> Result<bool> {
            if id == WOLF_BONES {
                self.calls.push("remove wolf bones".into());
                if self.wolf_bones < 1 {
                    return Ok(false);
                }
                self.wolf_bones -= 1;
                return Ok(true);
            }
            if self.logs < 1 {
                return Ok(false);
            }
            self.logs -= 1;
            self.calls.push(format!("remove {id}"));
            Ok(true)
        }
        fn animate(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("animate {id}"));
            Ok(())
        }
        fn random(&mut self, max: i32) -> Result<i32> {
            Ok(max - 1)
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
        fn seers_task_complete(&mut self, _: i32, _: i32) -> Result<bool> {
            Ok(self.task_done)
        }
        fn set_attribute_true(&mut self, key: &str) -> Result<()> {
            self.calls.push(format!("attribute {key}"));
            Ok(())
        }
        fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool> {
            self.calls.push(format!("delay {ticks}"));
            Ok(false)
        }
    }
    fn at(name: &str) -> usize {
        CRAFTS.iter().position(|c| c.name == name).unwrap()
    }

    #[test]
    fn the_table_groups_by_log() {
        assert_eq!(
            log_ids(),
            [1511, 2862, 1521, 1519, 1517, 1515, 1513, 6333, 6332]
        );
        assert_eq!(for_log_id(1511), Some(vec![0, 1, 2, 3]));
        assert_eq!(for_log_id(1513).map(|c| c.len()), Some(2));
        assert_eq!(for_log_id(6333), Some(vec![at("TEAK_STOCK")]));
        assert_eq!(for_log_id(1), None);
    }

    #[test]
    fn grammar() {
        assert_eq!(make_friendly_name("Oak shortbow (u)"), "Oak shortbow");
        assert_eq!(a_or_an("Oak shortbow"), Ok("an"));
        assert_eq!(a_or_an("Shortbow"), Ok("a"));
        assert_eq!(a_or_an("Unstrung comp bow"), Ok("an"));
        assert_eq!(a_or_an("Hat"), Ok("an"));
        assert_eq!(a_or_an("Helm"), Ok("a"));
        assert_eq!(a_or_an(""), Err(Error::Index));
    }

    #[test]
    fn create_checks_the_level_and_queues() {
        let logs = for_log_id(1511).unwrap();
        let mut w = world(1, 5);
        assert_eq!(create(&mut w, &logs, 5, 0), Ok(()));
        assert_eq!(w.calls, ["queue 1 ARROW_SHAFT 5"]);
        let mut w = world(1, 5);
        assert_eq!(create(&mut w, &logs, 1, 1), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You need a Fletching skill of 5 or above to make a Shortbow."]
        );
        let mut w = world(99, 5);
        assert_eq!(create(&mut w, &logs, 1, 4), Err(Error::Index));
    }

    #[test]
    fn the_ogre_crafts_ask_their_quests() {
        let achey = for_log_id(2862).unwrap();
        let mut w = world(99, 5);
        assert_eq!(create(&mut w, &achey, 1, 0), Ok(()));
        assert_eq!(
            w.calls,
            ["dialogue You must have started Big Chompy Bird Hunting to make those."]
        );
        let mut w = world(99, 5);
        w.chompy = 10;
        assert_eq!(create(&mut w, &achey, 1, 0), Ok(()));
        assert_eq!(w.calls, ["queue 1 OGRE_ARROW_SHAFT 1"]);
        let mut w = world(99, 5);
        assert_eq!(create(&mut w, &achey, 1, 1), Ok(()));
        assert_eq!(
            w.calls,
            ["message You must have started Zogre Flesh Eaters and asked Grish to string this."]
        );
        let mut w = world(99, 5);
        w.zogre = 8;
        assert_eq!(create(&mut w, &achey, 1, 1), Ok(()));
        assert_eq!(
            w.calls,
            ["message You need to have Wolf Bones in order to make this."]
        );
        let mut w = world(99, 5);
        (w.zogre, w.wolf_bones) = (8, 1);
        assert_eq!(create(&mut w, &achey, 1, 1), Ok(()));
        assert_eq!(w.calls, ["queue 1 OGRE_COMP_BOW 1"]);
    }

    #[test]
    fn the_script_cuts_shafts_and_delays() {
        let mut w = world(1, 2);
        assert_eq!(script(&mut w, at("ARROW_SHAFT"), 2, 0), Ok(false));
        assert_eq!(
            w.calls,
            [
                "remove 1511",
                "animate 1248",
                "message You carefully cut the wood into 15 arrow shafts.",
                "add 52 x15",
                "xp 5",
                "delay 3",
            ]
        );
        w.calls.clear();
        assert_eq!(script(&mut w, at("ARROW_SHAFT"), 2, 1), Ok(true));
        assert_eq!(w.calls.last().unwrap(), "stopExecuting");
        // No log left.
        w.calls.clear();
        assert_eq!(script(&mut w, at("ARROW_SHAFT"), 5, 2), Ok(true));
        assert_eq!(w.calls, ["stopExecuting"]);
    }

    #[test]
    fn the_script_waits_for_the_clock_and_rechecks_the_level() {
        let mut w = world(20, 1);
        w.clock_ready = false;
        assert_eq!(script(&mut w, at("OAK_SHORTBOW"), 1, 0), Ok(false));
        assert_eq!(w.calls, ["keepRunning"]);
        let mut w = world(19, 1);
        assert_eq!(script(&mut w, at("OAK_SHORTBOW"), 1, 0), Ok(true));
        assert_eq!(
            w.calls,
            [
                "dialogue You need a Fletching skill of 20 or above to make an Oak shortbow.",
                "stopExecuting"
            ]
        );
        let mut w = world(20, 1);
        assert_eq!(script(&mut w, at("OAK_SHORTBOW"), 1, 0), Ok(true));
        assert!(
            w.calls
                .contains(&"message You carefully cut the wood into an Oak shortbow.".to_string())
        );
        assert!(w.calls.contains(&"add 54 x1".to_string()));
    }

    #[test]
    fn ogre_shafts_roll_and_the_comp_bow_takes_bones() {
        let mut w = world(99, 1);
        assert_eq!(script(&mut w, at("OGRE_ARROW_SHAFT"), 1, 0), Ok(true));
        assert!(w.calls.contains(&"add 2864 x5".to_string()));
        // The log is gone before the bones are asked for.
        let mut w = world(99, 1);
        assert_eq!(script(&mut w, at("OGRE_COMP_BOW"), 1, 0), Ok(true));
        assert_eq!(
            w.calls,
            [
                "remove 2862",
                "animate 1248",
                "remove wolf bones",
                "stopExecuting"
            ]
        );
        let mut w = world(99, 1);
        w.wolf_bones = 1;
        assert_eq!(script(&mut w, at("OGRE_COMP_BOW"), 1, 0), Ok(true));
        assert!(w.calls.contains(
            &"message You carefully cut the wood into an Unstrung comp bow.".to_string()
        ));
    }

    #[test]
    fn a_magic_shortbow_in_seers_sets_the_diary_attribute() {
        let mut w = world(99, 1);
        w.at = [2722, 3490, 0];
        assert_eq!(script(&mut w, at("MAGIC_SHORTBOW"), 1, 0), Ok(true));
        assert!(
            w.calls
                .contains(&format!("attribute {ATTRIBUTE_MAGIC_SHORTBOW}"))
        );
        let mut w = world(99, 1);
        w.at = [2722, 3490, 0];
        w.task_done = true;
        assert_eq!(script(&mut w, at("MAGIC_SHORTBOW"), 1, 0), Ok(true));
        assert!(!w.calls.iter().any(|c| c.starts_with("attribute")));
        let mut w = world(99, 1);
        assert_eq!(script(&mut w, at("MAGIC_SHORTBOW"), 1, 0), Ok(true));
        assert!(!w.calls.iter().any(|c| c.starts_with("attribute")));
    }
}
