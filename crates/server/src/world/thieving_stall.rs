//! `content.global.skill.thieving.Stall`, `StallThiefPulse` (with
//! `SkillPulse`'s `start`, `pulse` and `stop` and `Pulse.update`) and
//! `ThievingOptionPlugin`.
//!
//! Differential corpus: `fixtures/thieving-stalls` (oracle/thieving-stalls),
//! replayed by `tests/thieving_stalls.rs` over these functions with the
//! oracle's declared state. Every outside call is a [`Host`] method.
//!
//! Chests are `super::thieving_chest` and doors `super::thieving_door`.
use rs09_game_core::java_lower::lowercase_root;

pub const PLUGIN_CLASS: &str = "content.global.skill.thieving.ThievingOptionPlugin";
pub const PULSE_CLASS: &str = "content.global.skill.thieving.StallThiefPulse";
/// The option names `SceneryDefinition.setOptionHandler` gets, in order.
pub const OPTIONS: [&str; 3] = ["steal-from", "steal from", "steal"];
/// `StallThiefPulse.ANIMATION`.
pub const ANIMATION: i32 = 832;
/// `Sounds.PICK_2581`.
pub const PICK_SOUND: i32 = 2581;
pub const THIEVE_DELAY: &str = "thieveDelay";
/// `Quests.THE_FREMENNIK_TRIALS`.
pub const FREMENNIK_TRIALS: &str = "The Fremennik Trials";
/// The Rellekka market's region.
pub const RELLEKKA: i32 = 10553;
pub const GRAPES: i32 = 1987;
/// The guards a failed attempt calls: ids 32 and 2236.
pub const GUARDS: [i32; 2] = [32, 2236];

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// An IndexOutOfBoundsException.
    Index,
    /// A NullPointerException.
    Null,
    /// A ClassCastException.
    Cast,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A `Stall` constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stall {
    pub name: &'static str,
    pub full_ids: &'static [i32],
    pub empty_ids: &'static [i32],
    pub level: i32,
    /// `rewards`: each item's id and amount.
    pub rewards: &'static [(i32, i32)],
    pub experience: f64,
    pub delay: i32,
    pub msg_item: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn stall(
    name: &'static str,
    full_ids: &'static [i32],
    empty_ids: &'static [i32],
    level: i32,
    rewards: &'static [(i32, i32)],
    experience: f64,
    delay: i32,
    msg_item: &'static str,
) -> Stall {
    Stall {
        name,
        full_ids,
        empty_ids,
        level,
        rewards,
        experience,
        delay,
        msg_item,
    }
}

pub const STALLS: [Stall; 18] = [
    stall(
        "VEGETABLE_STALL",
        &[4706, 4708],
        &[634],
        2,
        &[(1957, 1), (1965, 1), (1942, 1), (1982, 1), (1550, 1)],
        10.0,
        4,
        "vegetables",
    ),
    stall(
        "BAKER_STALL",
        &[2561, 6163, 34384],
        &[634, 6984, 34381],
        5,
        &[(1891, 1), (2309, 1), (1901, 1)],
        16.0,
        4,
        "pastries",
    ),
    stall(
        "CRAFTING_STALL",
        &[4874, 6166],
        &[4797, 6984],
        5,
        &[(1592, 1), (1597, 1), (1755, 1)],
        16.0,
        12,
        "crafting supplies",
    ),
    stall(
        "TEA_STALL",
        &[635, 6574],
        &[634, 6573],
        5,
        &[(712, 1)],
        16.0,
        12,
        "tea",
    ),
    stall(
        "SILK_STALL",
        &[34383, 2560],
        &[34381, 634],
        20,
        &[(950, 1)],
        24.0,
        13,
        "silk",
    ),
    stall(
        "WINE_STALL",
        &[14011],
        &[634],
        22,
        &[(1935, 1), (1937, 1), (1993, 1), (7919, 1)],
        27.0,
        27,
        "wine",
    ),
    stall(
        "MARKET_SEED_STALL",
        &[7053],
        &[634],
        27,
        &[
            (5096, 1),
            (5097, 1),
            (5101, 1),
            (5318, 1),
            (5319, 1),
            (5324, 1),
        ],
        10.0,
        19,
        "seeds",
    ),
    stall(
        "FUR_STALL",
        &[34387, 2563, 4278],
        &[34381, 634, 634],
        35,
        &[(6814, 1), (958, 1)],
        36.0,
        25,
        "fur",
    ),
    stall(
        "FISH_STALL",
        &[4277, 4705, 4707],
        &[634, 634, 634],
        42,
        &[(331, 1), (359, 1), (377, 1)],
        42.0,
        27,
        "fish",
    ),
    stall(
        "CROSSBOW_STALL",
        &[17031],
        &[6984],
        49,
        &[(877, 3), (9420, 1), (9440, 1)],
        52.0,
        19,
        "equipment",
    ),
    stall(
        "SILVER_STALL",
        &[2565, 6164, 34382],
        &[634, 6984, 34381],
        50,
        &[(442, 1)],
        54.0,
        50,
        "jewellery",
    ),
    stall(
        "SPICE_STALL",
        &[34386, 6166],
        &[34381, 6984],
        65,
        &[(2007, 1)],
        81.0,
        134,
        "spices",
    ),
    stall(
        "GEM_STALL",
        &[2562, 6162, 34385],
        &[634, 6984, 34381],
        75,
        &[(1623, 1), (1605, 1), (1603, 1), (1601, 1)],
        160.0,
        300,
        "gems",
    ),
    stall(
        "SCIMITAR_STALL",
        &[4878],
        &[4797],
        65,
        &[(1323, 1)],
        100.0,
        134,
        "equipment",
    ),
    stall(
        "MAGIC_STALL",
        &[4877],
        &[4797],
        65,
        &[(556, 1), (557, 1), (554, 1), (555, 1), (563, 1)],
        100.0,
        134,
        "equipment",
    ),
    stall(
        "GENERAL_STALL",
        &[4876],
        &[4797],
        5,
        &[(1931, 1), (2347, 1), (590, 1)],
        16.0,
        12,
        "goods",
    ),
    stall(
        "FOOD_STALL",
        &[4875],
        &[4797],
        5,
        &[(1963, 1)],
        16.0,
        12,
        "food",
    ),
    stall(
        "CANDLES",
        &[19127],
        &[19127],
        20,
        &[(36, 1)],
        20.0,
        0,
        "candles",
    ),
];
pub const SILK_STALL: usize = 4;
pub const CANDLES: usize = 17;

impl Stall {
    /// `getEmpty(id)`: `empty_ids.get(full_ids.indexOf(id))`.
    pub fn empty(&self, id: i32) -> Result<i32> {
        let at = self.full_ids.iter().position(|full| *full == id);
        at.and_then(|at| self.empty_ids.get(at).copied())
            .ok_or(Error::Index)
    }
    /// `name().toLowerCase().replace('_', ' ')`.
    pub fn spoken(&self) -> String {
        self.name.to_ascii_lowercase().replace('_', " ")
    }
}

/// `Stall.forObject(object)` by the object's id: `idMap` is filled with
/// `putIfAbsent` in declaration order, so the first constant listing an id
/// holds it (6166 is the crafting stall's, never the spice stall's).
pub fn for_object(id: i32) -> Option<usize> {
    STALLS.iter().position(|s| s.full_ids.contains(&id))
}

/// `StringUtils.isPlusN(word)` of a word that is not null.
pub fn is_plus_n(word: &str) -> Result<bool> {
    let lower: Vec<u16> = lowercase_root(word).encode_utf16().collect();
    // `s.charAt(0)` of an empty word.
    let first = *lower.first().ok_or(Error::Index)?;
    let is = |c: char| first == c as u16;
    Ok(is('a')
        || is('e')
        || is('i')
        || is('o')
        || is('u')
        || (is('h') && lower.len() > 1 && lower[1] != 'e' as u16))
}

/// What the plugin and the pulse ask of the world, for their player and
/// the scenery of the option.
pub trait Host {
    /// An NPC of `RegionManager.getLocalNPCs`.
    type Npc: Clone;

    /// `GameWorld.getTicks()`.
    fn world_ticks(&mut self) -> Result<i32>;
    /// `player.setAttribute("thieveDelay", ticks)`.
    fn set_thieve_delay(&mut self, ticks: i32) -> Result<()>;
    /// `player.getAttribute("thieveDelay", 0)`.
    fn thieve_delay(&mut self) -> Result<i32>;
    /// `Pulse.hasInactiveNode()`: the player or the scenery is not active.
    fn checks_inactive(&mut self) -> Result<bool>;
    /// `player.inCombat()`.
    fn in_combat(&mut self) -> Result<bool>;
    /// `player.getPacketDispatch().sendMessage(message)` (and
    /// `player.sendMessage`).
    fn message(&mut self, message: &str) -> Result<()>;
    /// `player.getSkills().getLevel(Skills.THIEVING)`.
    fn level(&mut self) -> Result<i32>;
    /// `node.getName()`.
    fn node_name(&mut self) -> Result<String>;
    /// `player.getInventory().freeSlots()`.
    fn free_slots(&mut self) -> Result<i32>;
    /// `player.getLocation().isInRegion(region)`.
    fn in_region(&mut self, region: i32) -> Result<bool>;
    /// ContentAPI `isQuestComplete(player, quest)`.
    fn quest_complete(&mut self, quest: &str) -> Result<bool>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn dialogue(&mut self, message: &str) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))` of
    /// `SkillPulse.stop`.
    fn reset_animation(&mut self) -> Result<()>;
    /// `player.animate(ANIMATION)`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.getLocks().lockInteractions(ticks)`.
    fn lock_interactions(&mut self, ticks: i32) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `player.getSavedData().getGlobalData().setSilkSteal(
    /// System.currentTimeMillis() + 1800000)`.
    fn set_silk_steal(&mut self, after_millis: i64) -> Result<()>;
    /// `node.isActive()`.
    fn node_active(&mut self) -> Result<bool>;
    /// `node.getId()`.
    fn node_id(&mut self) -> Result<i32>;
    /// `SceneryBuilder.replace(node, node.transform(id), delay)`.
    fn replace(&mut self, id: i32, delay: i32) -> Result<()>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `player.getInventory().add(item)`; its answer is not read.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `player.getSkills().addExperience(Skills.THIEVING, xp, true)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `item.getName()`.
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `player.dispatch(new ResourceProducedEvent(id, amount, node, 0))`.
    fn resource_produced(&mut self, id: i32, amount: i32) -> Result<()>;
    /// ContentAPI `stun(player, ticks, false)`.
    fn stun(&mut self, ticks: i32) -> Result<()>;
    /// ContentAPI `impact(player, amount, HitsplatType.NORMAL)`.
    fn impact(&mut self, amount: i32) -> Result<()>;
    /// `RegionManager.getLocalNPCs(player.getLocation(), distance)`.
    fn local_npcs(&mut self, distance: i32) -> Result<Vec<Self::Npc>>;
    /// `npc.getProperties().getCombatPulse().isAttacking()`.
    fn npc_attacking(&mut self, npc: &Self::Npc) -> Result<bool>;
    /// `npc.getId()`.
    fn npc_id(&mut self, npc: &Self::Npc) -> Result<i32>;
    /// `npc.sendChat(text)`.
    fn npc_chat(&mut self, npc: &Self::Npc, text: &str) -> Result<()>;
    /// `npc.getProperties().getCombatPulse().attack(player)`.
    fn npc_attack(&mut self, npc: &Self::Npc) -> Result<()>;
    /// `player.getPulseManager().run(pulse)`.
    fn run_pulse(&mut self, pulse: StallThiefPulse) -> Result<()>;
}

/// `ThievingOptionPlugin.handle(player, node, option)` for a scenery of id
/// `node`: its answer.
pub fn handle<H: Host + ?Sized>(h: &mut H, node: i32, option: &str) -> Result<bool> {
    if OPTIONS.contains(&option) {
        h.run_pulse(StallThiefPulse::new(for_object(node)))?;
        h.lock_interactions(6)?;
    }
    Ok(true)
}

/// `StallThiefPulse` over `SkillPulse<Scenery>`: a `Pulse(1, player, node)`
/// that `SkillPulse`'s constructor stops, so it runs only once `start()`
/// passed the requirements.
#[derive(Clone, Debug, PartialEq)]
pub struct StallThiefPulse {
    /// The stall's ordinal (null for a scenery `idMap` does not hold).
    pub stall: Option<usize>,
    pub ticks: i32,
    /// `Pulse.running`.
    pub running: bool,
}

impl StallThiefPulse {
    pub fn new(stall: Option<usize>) -> Self {
        Self {
            stall,
            ticks: 0,
            running: false,
        }
    }

    /// `start()`: the attribute, then `SkillPulse.start()`.
    pub fn start<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        let ticks = h.world_ticks()?;
        h.set_thieve_delay(ticks)?;
        if self.check_requirements(h)? {
            self.running = true;
            self.message(h, 0)?;
        }
        Ok(())
    }

    /// `SkillPulse.stop()`: the animation reset, `Pulse.stop()` and
    /// `message(1)` (which says nothing).
    pub fn stop<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        h.reset_animation()?;
        self.running = false;
        self.message(h, 1)
    }

    /// `hasInactiveNode()`.
    pub fn has_inactive_node<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if h.thieve_delay()? <= h.world_ticks()? {
            return Ok(false);
        }
        h.checks_inactive()
    }

    /// `SkillPulse.pulse()` (`animate()` is empty).
    pub fn pulse<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if !self.check_requirements(h)? {
            return Ok(true);
        }
        self.reward(h)
    }

    /// `Pulse.update()` (the delay is one tick): whether the runner drops
    /// the pulse. A body that threw is the error (the original prints it
    /// and drops the pulse).
    pub fn update<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if self.has_inactive_node(h)? {
            self.stop(h)?;
            return Ok(true);
        }
        if !self.running {
            return Ok(true);
        }
        match self.pulse(h) {
            Ok(true) => {
                self.stop(h)?;
                Ok(true)
            }
            Ok(false) => Ok(!self.running),
            Err(error) => {
                self.stop(h)?;
                Err(error)
            }
        }
    }

    /// `checkRequirements()`.
    pub fn check_requirements<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        let Some(stall) = self.stall else {
            return Ok(false);
        };
        let stall = STALLS.get(stall).ok_or(Error::Index)?;
        if h.in_combat()? {
            h.message("You cant steal from the market stall during combat!")?;
            return Ok(false);
        }
        if h.level()? < stall.level {
            let name = lowercase_root(&h.node_name()?);
            h.message(&format!(
                "You need to be level {} to steal from the {name}.",
                stall.level
            ))?;
            return Ok(false);
        }
        if h.free_slots()? == 0 {
            h.message("You don't have enough inventory space.")?;
            return Ok(false);
        }
        for (id, keeper) in [(4278, "fur trader"), (4277, "fishmonger")] {
            if h.in_region(RELLEKKA)?
                && !h.quest_complete(FREMENNIK_TRIALS)?
                && stall.full_ids.contains(&id)
            {
                h.dialogue(&format!(
                    "The {keeper} is staring at you suspiciously. You cannot steal from his stall while he distrusts you."
                ))?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `reward()`.
    pub fn reward<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if self.ticks == 0 {
            h.animate(ANIMATION)?;
            h.lock_interactions(2)?;
        }
        self.ticks += 1;
        if self.ticks % 3 != 0 {
            return Ok(false);
        }
        let ordinal = self.stall.ok_or(Error::Null)?;
        let stall = STALLS.get(ordinal).ok_or(Error::Index)?;
        if self.success(h)? {
            h.play_audio(PICK_SOUND)?;
            if ordinal == SILK_STALL {
                h.set_silk_steal(1_800_000)?;
            }
            if h.node_active()? {
                let empty = stall.empty(h.node_id()?)?;
                h.replace(empty, stall.delay)?;
            }
            // `getRandomLoot()`.
            let drawn = h.random(stall.rewards.len() as i32)?;
            let (id, amount) = *usize::try_from(drawn)
                .ok()
                .and_then(|at| stall.rewards.get(at))
                .ok_or(Error::Index)?;
            h.add_item(id, amount)?;
            h.add_experience(stall.experience)?;
            if id == GRAPES {
                h.message("You steal grapes from the grape stall.")?;
                return Ok(true);
            }
            if ordinal == CANDLES {
                return Ok(true);
            }
            let name = h.item_name(id)?;
            let article = if is_plus_n(&name)? { "an" } else { "a" };
            h.message(&format!(
                "You steal {article} {} from the {}.",
                lowercase_root(&name),
                stall.spoken()
            ))?;
            h.resource_produced(id, amount)?;
        }
        Ok(true)
    }

    /// `message(type)`.
    pub fn message<H: Host + ?Sized>(&mut self, h: &mut H, kind: i32) -> Result<()> {
        if self.stall == Some(CANDLES) {
            return Ok(());
        }
        if kind == 0 {
            // `stall.msgItem` of a null stall.
            let ordinal = self.stall.ok_or(Error::Null)?;
            let stall = STALLS.get(ordinal).ok_or(Error::Index)?;
            h.message(&format!(
                "You attempt to steal some {} from the {}",
                stall.msg_item,
                stall.spoken()
            ))?;
        }
        Ok(())
    }

    /// `success()`.
    fn success<H: Host + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if h.random(15)? < 4 {
            if self.stall == Some(CANDLES) {
                h.stun(15)?;
                h.impact(1)?;
                h.message("A higher power smites you")?;
                return Ok(false);
            }
            for npc in h.local_npcs(8)? {
                if !h.npc_attacking(&npc)? && GUARDS.contains(&h.npc_id(&npc)?) {
                    h.npc_chat(&npc, "Hey! Get your hands off there!")?;
                    h.npc_attack(&npc)?;
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_constant_listing_an_id_holds_it() {
        assert_eq!(STALLS[SILK_STALL].name, "SILK_STALL");
        assert_eq!(STALLS[CANDLES].name, "CANDLES");
        assert_eq!(for_object(6166), Some(2));
        assert_eq!(STALLS[2].empty(6166), Ok(6984));
        assert_eq!(for_object(634), None);
        assert_eq!(is_plus_n("Hat"), Ok(true));
        assert_eq!(is_plus_n("Herring"), Ok(false));
        assert_eq!(is_plus_n(""), Err(Error::Index));
    }
}
