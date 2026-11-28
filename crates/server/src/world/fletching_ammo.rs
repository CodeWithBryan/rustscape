//! `content.global.skill.fletching.bolts`, `.darts`, `.crossbow`, `.gem` and
//! `.grapple`: feathers on unfinished bolts and dart tips, limbs on crossbow
//! stocks, a chisel on gems for bolt tips, bolt tips on bolts, and the
//! mithril grapple's two steps.
//!
//! Differential corpus: `fixtures/fletching` (oracle/fletching), replayed by
//! `tests/fletching.rs` over these functions with the oracle's declared
//! state. Every outside call is a [`Host`] method.
//!
//! Covered: the four tables, each listener up to its handler or script
//! ([`use_with`]), each handler's `create` and `getAll` ([`create`],
//! [`get_all`]) and every queued script ([`script`]). The
//! SkillDialogueHandler itself is `dialogue_dsl::skill`'s. Not covered: the
//! Zogre Flesh Eaters fletching listeners (`ZogreFletchingListeners`).
use super::{
    dialogue_dsl::skill::SkillDialogue,
    fletching_assembly::FEATHERS,
    fletching_logs::{Error, Result},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const CHISEL: i32 = 1755;
pub const MITHRIL_BOLTS: i32 = 9142;
pub const MITH_GRAPPLE_TIP: i32 = 9416;
pub const ROPE: i32 = 954;
pub const MITH_GRAPPLE: i32 = 9418;
pub const MITH_GRAPPLE_WITH_ROPE: i32 = 9419;
/// The grapple listeners' level.
pub const GRAPPLE_LEVEL: i32 = 59;
/// A quest stage of a completed quest (`isQuestComplete`).
pub const QUEST_COMPLETE: i32 = 100;

/// The listeners' classes (`scan.contentInterfaces`).
pub const BOLT_CLASS: &str = "content.global.skill.fletching.bolts.BoltListeners";
pub const DART_CLASS: &str = "content.global.skill.fletching.darts.DartListeners";
pub const LIMBING_CLASS: &str = "content.global.skill.fletching.crossbow.LimbingListener";
pub const GEM_CLASS: &str = "content.global.skill.fletching.gem.GemBoltListeners";
pub const GRAPPLE_CLASS: &str = "content.global.skill.fletching.grapple.GrappleListeners";

/// A `BoltCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bolt {
    pub name: &'static str,
    pub unfinished: i32,
    pub finished: i32,
    pub level: i32,
    pub experience: f64,
}
const fn bolt(name: &'static str, unfinished: i32, finished: i32, level: i32, xp: f64) -> Bolt {
    Bolt {
        name,
        unfinished,
        finished,
        level,
        experience: xp,
    }
}
/// `BoltCraftInfo.values()`.
pub const BOLTS: [Bolt; 9] = [
    bolt("BRONZE_BOLT", 9375, 877, 9, 0.5),
    bolt("BLURITE_BOLT", 9376, 9139, 24, 1.0),
    bolt("IRON_BOLT", 9377, 9140, 39, 1.5),
    bolt("SILVER_BOLT", 9382, 9145, 43, 2.5),
    bolt("STEEL_BOLT", 9378, 9141, 46, 3.5),
    bolt("MITHRIL_BOLT", 9379, 9142, 54, 5.0),
    bolt("ADAMANTITE_BOLT", 9380, 9143, 61, 7.0),
    bolt("RUNITE_BOLT", 9381, 9144, 69, 10.0),
    bolt("BROAD_BOLT", 13279, 13280, 55, 3.0),
];

/// A `DartCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dart {
    pub name: &'static str,
    pub tip: i32,
    pub dart: i32,
    pub level: i32,
    pub experience: f64,
}
const fn dart(name: &'static str, tip: i32, dart: i32, level: i32, experience: f64) -> Dart {
    Dart {
        name,
        tip,
        dart,
        level,
        experience,
    }
}
/// `DartCraftInfo.values()`.
pub const DARTS: [Dart; 7] = [
    dart("BRONZE_DART", 819, 806, 1, 1.8),
    dart("IRON_DART", 820, 807, 22, 3.8),
    dart("STEEL_DART", 821, 808, 37, 7.5),
    dart("MITHRIL_DART", 822, 809, 52, 11.2),
    dart("ADAMANT_DART", 823, 810, 67, 15.0),
    dart("RUNE_DART", 824, 811, 81, 18.8),
    dart("DRAGON_DART", 11232, 11230, 95, 25.0),
];

/// An `UnfinishedCrossbowCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossbow {
    pub name: &'static str,
    pub stock: i32,
    pub limb: i32,
    pub unstrung: i32,
    pub level: i32,
    pub experience: f64,
    pub animation: i32,
}
const fn crossbow(
    name: &'static str,
    stock: i32,
    limb: i32,
    unstrung: i32,
    level: i32,
    experience: f64,
    animation: i32,
) -> Crossbow {
    Crossbow {
        name,
        stock,
        limb,
        unstrung,
        level,
        experience,
        animation,
    }
}
/// `UnfinishedCrossbowCraftInfo.values()`. The oak stock's product is the
/// strung blurite crossbow (9176), as the source has it.
pub const CROSSBOWS: [Crossbow; 7] = [
    crossbow("WOODEN_STOCK", 9440, 9420, 9454, 9, 12.0, 4436),
    crossbow("OAK_STOCK", 9442, 9422, 9176, 24, 32.0, 4437),
    crossbow("WILLOW_STOCK", 9444, 9423, 9457, 39, 44.0, 4438),
    crossbow("TEAK_STOCK", 9446, 9425, 9459, 46, 54.0, 4439),
    crossbow("MAPLE_STOCK", 9448, 9427, 9461, 54, 64.0, 4440),
    crossbow("MAHOGANY_STOCK", 9450, 9429, 9463, 61, 82.0, 4441),
    crossbow("YEW_STOCK", 9452, 9431, 9465, 69, 100.0, 4442),
];

/// A `GemBoltsCraftInfo` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gem {
    pub name: &'static str,
    pub untipped: i32,
    pub gem: i32,
    pub animation: i32,
    pub tip: i32,
    pub tipped: i32,
    pub level: i32,
    pub experience: f64,
}
#[allow(clippy::too_many_arguments)]
const fn gem(
    name: &'static str,
    untipped: i32,
    gem: i32,
    animation: i32,
    tip: i32,
    tipped: i32,
    level: i32,
    experience: f64,
) -> Gem {
    Gem {
        name,
        untipped,
        gem,
        animation,
        tip,
        tipped,
        level,
        experience,
    }
}
/// `GemBoltsCraftInfo.values()`.
pub const GEMS: [Gem; 11] = [
    gem("OPAL", 877, 1609, 890, 45, 879, 11, 1.6),
    gem("JADE", 9139, 1611, 891, 9187, 9335, 26, 2.4),
    gem("PEARL", 9140, 411, 4470, 46, 880, 41, 3.2),
    gem("PEARLS", 9140, 413, 4470, 46, 880, 41, 3.2),
    gem("RED_TOPAZ", 9141, 1613, 892, 9188, 9336, 48, 3.9),
    gem("SAPPHIRE", 9142, 1607, 888, 9189, 9337, 56, 4.7),
    gem("EMERALD", 9142, 1605, 889, 9190, 9338, 58, 5.5),
    gem("RUBY", 9143, 1603, 887, 9191, 9339, 63, 6.3),
    gem("DIAMOND", 9143, 1601, 886, 9192, 9340, 65, 7.0),
    gem("DRAGONSTONE", 9144, 1615, 885, 9193, 9341, 71, 8.2),
    gem("ONYX", 9144, 6573, 2717, 9194, 9342, 73, 9.4),
];

/// `GemBoltsCraftInfo.forGemId(id)`, as an index.
pub fn for_gem(id: i32) -> Option<usize> {
    GEMS.iter().position(|gem| gem.gem == id)
}
/// `GemBoltsCraftInfo.forTipId(id)`, as an index: `associateBy` keeps the
/// last constant of a tip, so pearl tips answer `PEARLS`.
pub fn for_tip(id: i32) -> Option<usize> {
    GEMS.iter().rposition(|gem| gem.tip == id)
}
/// `GemBoltsCraftInfo.untippedBoltIds`: distinct, in declaration order.
pub fn untipped_ids() -> Vec<i32> {
    let mut ids = Vec::new();
    for gem in &GEMS {
        if !ids.contains(&gem.untipped) {
            ids.push(gem.untipped);
        }
    }
    ids
}

/// The seven `onUseWith(IntType.ITEM, ...)` lambdas, by class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listener {
    /// BoltListeners: a feather with unfinished bolts.
    Bolts,
    /// DartListeners: a feather with dart tips.
    Darts,
    /// LimbingListener: limbs with a stock.
    Limbs,
    /// GemBoltListeners' first: a chisel with a gem.
    GemCut,
    /// GemBoltListeners' second: bolts with bolt tips.
    GemTip,
    /// GrappleListeners' first: mithril bolts with a grapple tip.
    GrappleTip,
    /// GrappleListeners' second: a rope with a mithril grapple.
    GrappleRope,
}
impl Listener {
    pub const ALL: [Self; 7] = [
        Self::Bolts,
        Self::Darts,
        Self::Limbs,
        Self::GemCut,
        Self::GemTip,
        Self::GrappleTip,
        Self::GrappleRope,
    ];
    /// The listener's class.
    pub fn class(self) -> &'static str {
        match self {
            Self::Bolts => BOLT_CLASS,
            Self::Darts => DART_CLASS,
            Self::Limbs => LIMBING_CLASS,
            Self::GemCut | Self::GemTip => GEM_CLASS,
            Self::GrappleTip | Self::GrappleRope => GRAPPLE_CLASS,
        }
    }
    /// The `onUseWith` lambda.
    pub fn lambda(self) -> &'static str {
        match self {
            Self::Bolts => "content.global.skill.fletching.bolts.BoltListeners$defineListeners$1",
            Self::Darts => "content.global.skill.fletching.darts.DartListeners$defineListeners$1",
            Self::Limbs => {
                "content.global.skill.fletching.crossbow.LimbingListener$defineListeners$1"
            }
            Self::GemCut => "content.global.skill.fletching.gem.GemBoltListeners$defineListeners$1",
            Self::GemTip => "content.global.skill.fletching.gem.GemBoltListeners$defineListeners$2",
            Self::GrappleTip => {
                "content.global.skill.fletching.grapple.GrappleListeners$defineListeners$1"
            }
            Self::GrappleRope => {
                "content.global.skill.fletching.grapple.GrappleListeners$defineListeners$2"
            }
        }
    }
    /// The ids the listener registers: the used ones, then the others.
    pub fn ids(self) -> (Vec<i32>, Vec<i32>) {
        match self {
            Self::Bolts => (
                FEATHERS.to_vec(),
                BOLTS.iter().map(|bolt| bolt.unfinished).collect(),
            ),
            Self::Darts => (
                FEATHERS.to_vec(),
                DARTS.iter().map(|dart| dart.tip).collect(),
            ),
            Self::Limbs => (
                CROSSBOWS.iter().map(|bow| bow.limb).collect(),
                CROSSBOWS.iter().map(|bow| bow.stock).collect(),
            ),
            Self::GemCut => (vec![CHISEL], GEMS.iter().map(|gem| gem.gem).collect()),
            Self::GemTip => (untipped_ids(), GEMS.iter().map(|gem| gem.tip).collect()),
            Self::GrappleTip => (vec![MITHRIL_BOLTS], vec![MITH_GRAPPLE_TIP]),
            Self::GrappleRope => (vec![ROPE], vec![MITH_GRAPPLE]),
        }
    }
}

/// What a listener's SkillDialogueHandler makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Craft {
    /// `BOLTS[n]` with the feather used.
    Bolt(usize, i32),
    /// `DARTS[n]` with the feather used.
    Dart(usize, i32),
    /// `CROSSBOWS[n]`.
    Limb(usize),
    /// `GEMS[n]` cut into tips.
    GemCut(usize),
    /// `GEMS[n]`'s tips on its bolts.
    GemTip(usize),
}
impl Craft {
    /// The handler's `SkillDialogue`.
    pub fn kind(self) -> SkillDialogue {
        match self {
            Self::Limb(_) | Self::GemCut(_) => SkillDialogue::OneOption,
            _ => SkillDialogue::MakeSetOneOption,
        }
    }
    /// The item the handler shows (the gem itself for a cut).
    pub fn product(self) -> Result<i32> {
        Ok(match self {
            Self::Bolt(n, _) => BOLTS.get(n).ok_or(Error::Index)?.finished,
            Self::Dart(n, _) => DARTS.get(n).ok_or(Error::Index)?.dart,
            Self::Limb(n) => CROSSBOWS.get(n).ok_or(Error::Index)?.unstrung,
            Self::GemCut(n) => GEMS.get(n).ok_or(Error::Index)?.gem,
            Self::GemTip(n) => GEMS.get(n).ok_or(Error::Index)?.tipped,
        })
    }
    /// The RepositionChild the listener sends after its dialogue, if any:
    /// interface, child, x and y.
    pub fn reposition(self) -> Option<[i32; 4]> {
        matches!(self, Self::Limb(_)).then_some([309, 2, 210, 10])
    }
}

/// `CutGemsIntoBoltTipsScript.craftingFinished`, shared by its two loops.
pub type Finished = Arc<AtomicBool>;

/// A queued script with what its lambda captured.
#[derive(Debug, Clone)]
pub enum Script {
    Bolt(usize, i32),
    Dart(usize, i32),
    Limb(usize),
    /// `CutGemsIntoBoltTipsScript.invoke()`'s script, which queues the two
    /// loops.
    GemStart(usize, Finished),
    GemAnimation(usize, Finished),
    GemCraft(usize, Finished),
    GemTip(usize),
    GrappleTip,
    GrappleRope,
}
impl Script {
    /// The script's `queueScript` lambda.
    pub fn queued_lambda(&self) -> &'static str {
        match self {
            Self::Bolt(..) => "content.global.skill.fletching.bolts.BoltCraftScript$invoke$1",
            Self::Dart(..) => "content.global.skill.fletching.darts.DartCraftScript$invoke$1",
            Self::Limb(_) => "content.global.skill.fletching.crossbow.LimbingScript$invoke$1",
            Self::GemStart(..) => {
                "content.global.skill.fletching.gem.CutGemsIntoBoltTipsScript$invoke$1"
            }
            Self::GemAnimation(..) => {
                "content.global.skill.fletching.gem.CutGemsIntoBoltTipsScript$invokeAnimationLoop$1"
            }
            Self::GemCraft(..) => {
                "content.global.skill.fletching.gem.CutGemsIntoBoltTipsScript$invokeCraftLoop$1"
            }
            Self::GemTip(_) => {
                "content.global.skill.fletching.gem.AttachGemTipToBoltScript$invoke$1"
            }
            Self::GrappleTip => {
                "content.global.skill.fletching.grapple.GrappleListeners$defineListeners$1$1"
            }
            Self::GrappleRope => {
                "content.global.skill.fletching.grapple.GrappleListeners$defineListeners$2$1"
            }
        }
    }
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
    /// `player.questRepository.getStage(Quests.THE_TOURIST_TRAP)`.
    fn tourist_trap_stage(&mut self) -> Result<i32>;
    /// `getSlayerFlags(player).isBroadsUnlocked()`.
    fn broads_unlocked(&mut self) -> Result<bool>;
    /// `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32>;
    /// `hasSpaceFor(player, Item(id))`.
    fn has_space_for(&mut self, id: i32) -> Result<bool>;
    /// `queueScript(player, delay) { ... }` (WEAK, not persistent); the
    /// script is [`script`] with this amount.
    fn queue(&mut self, delay: i32, script: Script, amount: i32) -> Result<()>;
    /// `clockReady(player, Clocks.SKILLING)`.
    fn clock_ready(&mut self) -> Result<bool>;
    /// `keepRunning(player)`.
    fn keep_running(&mut self) -> Result<bool>;
    /// `stopExecuting(player)`.
    fn stop_executing(&mut self) -> Result<bool>;
    /// `removeItem(player, Item(id, amount))` (from the inventory).
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `animate(player, id)`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `addItem(player, id, amount)`; the scripts ignore the answer.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `rewardXP(player, Skills.FLETCHING, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// `delayClock(player, Clocks.SKILLING, ticks, delayScript)`.
    fn delay_clock(&mut self, ticks: i32, delay_script: bool) -> Result<bool>;
    /// `delayScript(player, ticks)`.
    fn delay_script(&mut self, ticks: i32) -> Result<bool>;
}

const NO_SPACE: &str = "You do not have enough inventory space.";

fn gem_level_fail<H: Host>(host: &mut H, level: i32) -> Result<()> {
    host.dialogue(&format!(
        "You need a Fletching level of {level} or above to do that."
    ))
}
fn bolt_level_fail<H: Host>(host: &mut H, level: i32) -> Result<()> {
    host.dialogue(&format!(
        "You need a fletching level of {level} in order to do this."
    ))
}
fn dart_level_fail<H: Host>(host: &mut H, level: i32) -> Result<()> {
    host.dialogue(&format!(
        "You need a fletching level of {level} to do this."
    ))
}
fn limb_level_fail<H: Host>(host: &mut H, level: i32) -> Result<()> {
    host.dialogue(&format!(
        "You need a fletching level of {level} to attach these limbs."
    ))
}

/// A lambda up to its handler: `None` is the lambda's `false`, `Some(None)`
/// its `true` without a handler (a message, or a grapple script queued) and
/// `Some(Some(craft))` the craft whose handler opens.
pub fn use_with<H: Host>(
    host: &mut H,
    listener: Listener,
    used: i32,
    with: i32,
) -> Result<Option<Option<Craft>>> {
    Ok(match listener {
        Listener::Bolts => BOLTS
            .iter()
            .position(|bolt| bolt.unfinished == with)
            .map(|n| Some(Craft::Bolt(n, used))),
        Listener::Darts => DARTS
            .iter()
            .position(|dart| dart.tip == with)
            .map(|n| Some(Craft::Dart(n, used))),
        Listener::Limbs => {
            let Some(n) = CROSSBOWS.iter().position(|bow| bow.stock == with) else {
                return Ok(None);
            };
            if CROSSBOWS[n].limb != used {
                host.message("That's not the right limb to attach to that stock.")?;
                return Ok(Some(None));
            }
            Some(Some(Craft::Limb(n)))
        }
        // A gem without a constant answers true.
        Listener::GemCut => Some(for_gem(with).map(Craft::GemCut)),
        Listener::GemTip => for_tip(with)
            .filter(|n| GEMS[*n].untipped == used)
            .map(|n| Some(Craft::GemTip(n))),
        Listener::GrappleTip | Listener::GrappleRope => {
            if host.level()? < GRAPPLE_LEVEL {
                host.message("You need a fletching level of 59 to make this.")?;
                return Ok(Some(None));
            }
            let script = match listener {
                Listener::GrappleTip => Script::GrappleTip,
                _ => Script::GrappleRope,
            };
            host.queue(0, script, 0)?;
            Some(None)
        }
    })
}

/// A handler's `create(amount, index)`.
pub fn create<H: Host>(host: &mut H, craft: Craft, amount: i32) -> Result<()> {
    let script = match craft {
        Craft::Bolt(n, feather) => {
            let bolt = BOLTS.get(n).ok_or(Error::Index)?;
            if bolt.name == "BROAD_BOLT" && !host.broads_unlocked()? {
                return host.dialogue("You need to unlock the ability to create broad bolts.");
            }
            if host.level()? < bolt.level {
                return bolt_level_fail(host, bolt.level);
            }
            if !host.has_space_for(bolt.finished)? {
                return host.dialogue(NO_SPACE);
            }
            Script::Bolt(n, feather)
        }
        Craft::Dart(n, feather) => {
            let dart = DARTS.get(n).ok_or(Error::Index)?;
            if host.level()? < dart.level {
                return dart_level_fail(host, dart.level);
            }
            if host.tourist_trap_stage()? != QUEST_COMPLETE {
                return host.dialogue("You need to have completed Tourist Trap to fletch darts.");
            }
            if !host.has_space_for(dart.dart)? {
                return host.dialogue(NO_SPACE);
            }
            Script::Dart(n, feather)
        }
        Craft::Limb(n) => {
            let bow = CROSSBOWS.get(n).ok_or(Error::Index)?;
            if host.level()? < bow.level {
                return limb_level_fail(host, bow.level);
            }
            if !host.has_space_for(bow.unstrung)? {
                return host.dialogue(NO_SPACE);
            }
            Script::Limb(n)
        }
        Craft::GemCut(n) => {
            let gem = GEMS.get(n).ok_or(Error::Index)?;
            if host.level()? < gem.level {
                return gem_level_fail(host, gem.level);
            }
            Script::GemStart(n, Finished::default())
        }
        Craft::GemTip(n) => {
            let gem = GEMS.get(n).ok_or(Error::Index)?;
            if host.level()? < gem.level {
                return gem_level_fail(host, gem.level);
            }
            if !host.has_space_for(gem.tipped)? {
                return host.dialogue(NO_SPACE);
            }
            Script::GemTip(n)
        }
    };
    host.queue(1, script, amount)
}

/// A handler's `getAll(index)`, with the two items its lambda was given.
pub fn get_all<H: Host>(host: &mut H, craft: Craft, used: i32, with: i32) -> Result<i32> {
    match craft {
        // The smaller of the two items used.
        Craft::Bolt(..) | Craft::Dart(..) | Craft::GemTip(_) => {
            let first = host.amount_in_inventory(used)?;
            Ok(first.min(host.amount_in_inventory(with)?))
        }
        // `player.inventory.getAmount(stock.asItem())`.
        Craft::Limb(_) => host.amount_in_inventory(with),
        Craft::GemCut(n) => host.amount_in_inventory(GEMS.get(n).ok_or(Error::Index)?.gem),
    }
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

/// A set script's shared tail: remove the pair, tell, reward and add.
#[allow(clippy::too_many_arguments)]
fn set<H: Host>(
    host: &mut H,
    first: i32,
    second: i32,
    product: i32,
    experience: f64,
    one: &str,
    many: impl Fn(i32) -> String,
) -> Result<bool> {
    let held = host.amount_in_inventory(first)?;
    let made = held.min(host.amount_in_inventory(second)?).min(10);
    if !remove_if_enough(host, &[(first, made), (second, made)])? {
        return Ok(false);
    }
    if made == 1 {
        host.message(one)?;
    } else {
        host.message(&many(made))?;
    }
    host.reward_xp(experience * f64::from(made))?;
    host.add_item(product, made)?;
    Ok(true)
}

/// A queued script at `stage`, with the amount (or sets) its class was
/// given.
pub fn script<H: Host>(host: &mut H, script: &Script, amount: i32, stage: i32) -> Result<bool> {
    let delay = match script {
        Script::GemStart(n, finished) => {
            // invokeAnimationLoop(), invokeCraftLoop().
            host.queue(0, Script::GemAnimation(*n, finished.clone()), amount)?;
            host.queue(0, Script::GemCraft(*n, finished.clone()), amount)?;
            return host.stop_executing();
        }
        Script::GemAnimation(n, finished) => {
            if finished.load(Ordering::Relaxed) {
                return host.stop_executing();
            }
            host.animate(GEMS.get(*n).ok_or(Error::Index)?.animation)?;
            return host.delay_script(6);
        }
        _ if !host.clock_ready()? => return host.keep_running(),
        Script::Bolt(n, feather) => {
            let bolt = *BOLTS.get(*n).ok_or(Error::Index)?;
            if host.level()? < bolt.level {
                bolt_level_fail(host, bolt.level)?;
                return host.stop_executing();
            }
            let made = set(
                host,
                *feather,
                bolt.unfinished,
                bolt.finished,
                bolt.experience,
                "You attach a feather to a bolt.",
                |made| format!("You fletch {made} bolts"),
            )?;
            if !made {
                return host.stop_executing();
            }
            3
        }
        Script::Dart(n, feather) => {
            let dart = *DARTS.get(*n).ok_or(Error::Index)?;
            if host.level()? < dart.level {
                dart_level_fail(host, dart.level)?;
                return host.stop_executing();
            }
            let made = set(
                host,
                *feather,
                dart.tip,
                dart.dart,
                dart.experience,
                "You attach a feather to a dart.",
                |made| format!("You attach feathers to {made} darts."),
            )?;
            if !made {
                return host.stop_executing();
            }
            3
        }
        Script::GemTip(n) => {
            let gem = *GEMS.get(*n).ok_or(Error::Index)?;
            if host.level()? < gem.level {
                gem_level_fail(host, gem.level)?;
                return host.stop_executing();
            }
            let made = set(
                host,
                gem.untipped,
                gem.tip,
                gem.tipped,
                gem.experience,
                "You attach the tip to the bolt.",
                |made| format!("You fletch {made} bolts."),
            )?;
            if !made {
                return host.stop_executing();
            }
            3
        }
        Script::Limb(n) => {
            let bow = *CROSSBOWS.get(*n).ok_or(Error::Index)?;
            if host.level()? < bow.level {
                limb_level_fail(host, bow.level)?;
                return host.stop_executing();
            }
            if !remove_if_enough(host, &[(bow.stock, 1), (bow.limb, 1)])? {
                return host.stop_executing();
            }
            host.add_item(bow.unstrung, 1)?;
            host.reward_xp(bow.experience)?;
            host.message("You attach the metal limbs to the stock.")?;
            host.animate(bow.animation)?;
            6
        }
        Script::GemCraft(n, finished) => {
            let gem = *GEMS.get(*n).ok_or(Error::Index)?;
            if host.level()? < gem.level {
                finished.store(true, Ordering::Relaxed);
                gem_level_fail(host, gem.level)?;
                return host.stop_executing();
            }
            let tips = match gem.name {
                "PEARLS" | "ONYX" => 24,
                "PEARL" => 6,
                _ => 12,
            };
            if !host.remove_item(gem.gem, 1)? {
                finished.store(true, Ordering::Relaxed);
                return host.stop_executing();
            }
            host.add_item(gem.tip, tips)?;
            host.reward_xp(gem.experience)?;
            if stage >= amount.wrapping_sub(1) {
                finished.store(true, Ordering::Relaxed);
                return host.stop_executing();
            }
            return host.delay_clock(5, true);
        }
        Script::GrappleTip | Script::GrappleRope => {
            let (items, product) = match script {
                Script::GrappleTip => ([(MITHRIL_BOLTS, 1), (MITH_GRAPPLE_TIP, 1)], MITH_GRAPPLE),
                _ => ([(ROPE, 1), (MITH_GRAPPLE, 1)], MITH_GRAPPLE_WITH_ROPE),
            };
            if remove_if_enough(host, &items)? {
                host.add_item(product, 1)?;
            }
            host.delay_clock(3, false)?;
            return host.stop_executing();
        }
    };
    if stage >= amount.wrapping_sub(1) {
        return host.stop_executing();
    }
    host.delay_clock(delay, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables() {
        assert_eq!(for_gem(413), Some(3));
        assert_eq!(for_gem(1), None);
        // Pearl tips answer the last constant that names them.
        assert_eq!(for_tip(46), Some(3));
        assert_eq!(for_tip(9194), Some(10));
        assert_eq!(untipped_ids(), [877, 9139, 9140, 9141, 9142, 9143, 9144]);
        assert_eq!(Listener::GemTip.ids().1.len(), 11);
        assert_eq!(Listener::Limbs.ids().0[0], 9420);
        assert_eq!(Craft::GemCut(0).product(), Ok(1609));
        assert_eq!(Craft::Limb(1).product(), Ok(9176));
        assert_eq!(Craft::Limb(0).reposition(), Some([309, 2, 210, 10]));
        assert_eq!(Craft::Bolt(0, 314).reposition(), None);
        assert_eq!(Craft::Dart(9, 314).product(), Err(Error::Index));
    }
}
