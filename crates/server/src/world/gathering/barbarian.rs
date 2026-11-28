//! Barbarian fishing (`content.global.skill.gather.fishing.barbfishing`):
//! `SpotManager` with its `BarbFishingSpot` NPCs, `BarbFishInteractionListeners`
//! (the bed's "search" and the spots' "use-rod"), `BarbFishingPulse`,
//! `KnifeWithFish` and `FishCuttingPulse`.
//!
//! The player bodies are written over the gathering services
//! ([`super::service::Context`]); the spot manager draws through [`Spots`].
//! No JVM corpus covers this module: the unit tests check the port against the
//! source's control flow only.
//!
//! Source facts kept as they are:
//!
//! - `SpotManager.ticks` is never incremented, so `tick()` rebuilds
//!   `usedLocations` from the spots on every tick.
//! - `getNewLoc` picks with Kotlin's `Collection.random()` (`Random.Default`,
//!   the thread-local generator), not `RandomFunction`; the time to live is
//!   `RandomFunction.random(400, 2000)`.
//! - (2504, 3497) is listed twice.
//! - `FishCuttingPulse.checkRequirements` is never called (the class extends
//!   `Pulse`, which has no such hook), so the space check and its message do
//!   not exist at runtime; a fish that cannot be removed leaves the pulse
//!   running, drawing two numbers every tick.
use rs09_game_core::java_lower::lowercase_root;

use super::{
    fishing::Fish,
    random,
    service::{Arg, Calls, Context, Error, Handle, Result, Service, Task},
    tools::FISHING,
};

/// A location as `[x, y, z]`.
pub type Tile = [i32; 3];

/// `NPCs.FISHING_SPOT_2722`.
pub const SPOT: i32 = 2722;
/// `Scenery.BARBARIAN_BED_25268`.
pub const BED: i32 = 25268;
pub const KNIFE: i32 = 946;
pub const BARBARIAN_ROD: i32 = 11323;
pub const FISH_OFFCUTS: i32 = 11334;
pub const ROE: i32 = 11324;
pub const CAVIAR: i32 = 11326;
pub const LEAPING_TROUT: i32 = 11328;
pub const LEAPING_SALMON: i32 = 11330;
pub const LEAPING_STURGEON: i32 = 11332;
/// The fish `KnifeWithFish` registers, in source order.
pub const LEAPING_FISH: [i32; 3] = [LEAPING_TROUT, LEAPING_SALMON, LEAPING_STURGEON];
/// `BarbFishingPulse.baits`.
const BAITS: [i32; 5] = [FISH_OFFCUTS, 314, 313, ROE, CAVIAR];
/// `BarbarianTraining.attributeRod`.
const ATTRIBUTE_ROD: &str = "/save:barbtraining:rod";
const FISH_KEY: &str = "stats_manager:fish_caught";
const SAVED_FISH_KEY: &str = "/save:stats_manager:fish_caught";
const STRENGTH: i32 = 2;
const COOKING: i32 = 7;
const AGILITY: i32 = 16;

/// The listeners' classes (`scan.contentInterfaces`).
pub const LISTENERS_CLASS: &str =
    "content.global.skill.gather.fishing.barbfishing.BarbFishInteractionListeners";
pub const KNIFE_CLASS: &str = "content.global.skill.gather.fishing.barbfishing.KnifeWithFish";
pub const MANAGER_CLASS: &str = "content.global.skill.gather.fishing.barbfishing.SpotManager";

// ------------------------------------------------------------ SpotManager

/// `SpotManager.locations` (and `BarbFishingSpot.locations`).
pub const LOCATIONS: [Tile; 7] = [
    [2506, 3494, 0],
    [2504, 3497, 0],
    [2504, 3497, 0],
    [2500, 3506, 0],
    [2500, 3509, 0],
    [2500, 3512, 0],
    [2504, 3516, 0],
];
/// How many spots `startup()` makes.
pub const SPOTS: usize = 5;

/// What the spot manager draws.
pub trait Spots {
    type Error;
    /// `RandomFunction.random(a, b)`.
    fn random_range(&mut self, a: i32, b: i32) -> std::result::Result<i32, Self::Error>;
    /// `Random.Default.nextInt(until)` (Kotlin's `Collection.random()`).
    fn kotlin_next_int(&mut self, until: i32) -> std::result::Result<i32, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotError<E> {
    /// `possibleLoc.random()` of an empty list.
    NoSuchElement,
    Host(E),
}

/// One `BarbFishingSpot`'s `loc` and `ttl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub loc: Option<Tile>,
    pub ttl: i32,
}

/// `SpotManager`: its instance fields and the companion's `usedLocations`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manager {
    pub ticks: i32,
    pub spots: Vec<Spot>,
    pub used: Vec<Tile>,
}

/// `getNewTTL()`.
pub fn new_ttl<H: Spots>(h: &mut H) -> std::result::Result<i32, SpotError<H::Error>> {
    h.random_range(400, 2000).map_err(SpotError::Host)
}

/// `getNewLoc()`.
pub fn new_loc<H: Spots>(
    h: &mut H,
    used: &mut Vec<Tile>,
) -> std::result::Result<Tile, SpotError<H::Error>> {
    let possible: Vec<Tile> = LOCATIONS
        .iter()
        .filter(|loc| !used.contains(loc))
        .copied()
        .collect();
    if possible.is_empty() {
        return Err(SpotError::NoSuchElement);
    }
    let index = h
        .kotlin_next_int(possible.len() as i32)
        .map_err(SpotError::Host)?;
    let loc = possible[index as usize];
    used.push(loc);
    Ok(loc)
}

impl Manager {
    /// `SpotManager.tick()`.
    pub fn tick(&mut self) {
        if self.ticks % 50 == 0 {
            self.used.clear();
            for spot in &self.spots {
                self.used.push(spot.loc.unwrap_or([0, 0, 0]));
            }
        }
    }

    /// One iteration of `startup()`'s loop up to the spot's `init()`:
    /// `BarbFishingSpot(getNewLoc(), getNewTTL())` added to `spots`. The
    /// owner makes the NPC at the spot's `loc` and inits it.
    pub fn add_spot<H: Spots>(
        &mut self,
        h: &mut H,
    ) -> std::result::Result<Spot, SpotError<H::Error>> {
        let loc = new_loc(h, &mut self.used)?;
        let spot = Spot {
            loc: Some(loc),
            ttl: new_ttl(h)?,
        };
        self.spots.push(spot);
        Ok(spot)
    }

    /// `BarbFishingSpot.handleTickActions()` of the spot at `index`, standing
    /// at `location`: the value assigned to `properties.teleportLocation`, if
    /// any.
    pub fn handle_tick_actions<H: Spots>(
        &mut self,
        h: &mut H,
        index: usize,
        location: Option<Tile>,
    ) -> std::result::Result<Option<Option<Tile>>, SpotError<H::Error>> {
        let mut teleport = None;
        if location != self.spots[index].loc {
            // `loc.also { ttl = getNewTTL() }`.
            teleport = Some(self.spots[index].loc);
            self.spots[index].ttl = new_ttl(h)?;
        }
        let ttl = self.spots[index].ttl;
        self.spots[index].ttl = ttl.wrapping_sub(1);
        if ttl <= 0 {
            // `usedLocations.remove(location)`: the first equal element.
            if let Some(location) = location
                && let Some(at) = self.used.iter().position(|l| *l == location)
            {
                self.used.remove(at);
            }
            self.spots[index].loc = Some(new_loc(h, &mut self.used)?);
        }
        Ok(teleport)
    }
}

// ------------------------------------------------------ player bodies

fn message(ctx: &mut (impl Context + ?Sized), player: Handle, text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendMessage, &[player.into(), text.into()])
}
fn dialogue(ctx: &mut (impl Context + ?Sized), text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendDialogue, &[text.into()])
}
fn in_inventory(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<bool> {
    ctx.boolean(Service::ApiInInventory, &[id.into(), 1.into()])
}
fn has_level_dyn(ctx: &mut (impl Context + ?Sized), skill: i32, level: i32) -> Result<bool> {
    ctx.boolean(Service::ApiHasLevelDyn, &[skill.into(), level.into()])
}
fn dyn_level(ctx: &mut (impl Context + ?Sized), skill: i32) -> Result<i32> {
    ctx.int(Service::ApiGetDynLevel, &[skill.into()])
}
fn free_slots(ctx: &mut (impl Context + ?Sized)) -> Result<i32> {
    ctx.int(Service::ApiFreeSlots, &[])
}
fn remove_item(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<bool> {
    ctx.boolean(Service::ApiRemoveItem, &[id.into(), "INVENTORY".into()])
}
fn add_item_or_drop(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<()> {
    ctx.unit(Service::ApiAddItemOrDrop, &[id.into(), 1.into()])
}
fn reward_xp(ctx: &mut (impl Context + ?Sized), skill: i32, amount: f64) -> Result<()> {
    ctx.unit(Service::ApiRewardXp, &[skill.into(), amount.into()])
}
fn animate(ctx: &mut (impl Context + ?Sized), animation: i32) -> Result<()> {
    ctx.unit(
        Service::ApiAnimate,
        &[animation.into(), Arg::Null, false.into()],
    )
}
/// `getAttribute(player, key, 0)` used as an Int.
fn int_attribute(ctx: &mut (impl Context + ?Sized), key: &str) -> Result<i32> {
    match ctx.call(Service::ApiGetAttribute, &[key.into(), 0.into()])? {
        Arg::Int(v) => i32::try_from(v).map_err(|_| Error::Reply),
        _ => Err(Error::ClassCast),
    }
}

/// The bed's `on(Scenery.BARBARIAN_BED_25268, IntType.SCENERY, "search")`.
pub fn search_bed(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<bool> {
    if int_attribute(ctx, ATTRIBUTE_ROD)? >= 1 {
        if !in_inventory(ctx, BARBARIAN_ROD)? {
            add_item_or_drop(ctx, BARBARIAN_ROD)?;
            message(
                ctx,
                player,
                "You find a heavy fishing rod under the bed and take it.",
            )?;
        } else {
            message(ctx, player, "You find nothing under the bed.")?;
        }
    } else {
        message(
            ctx,
            player,
            "Maybe I should speak to Otto before looking under his bed.",
        )?;
    }
    Ok(true)
}

/// The spots' `on(NPCs.FISHING_SPOT_2722, IntType.NPC, "use-rod")`.
pub fn use_rod(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<bool> {
    // submitIndividualPulse(player, BarbFishingPulse(player)).
    ctx.defer(
        Service::PulseManagerRun,
        &[player.into()],
        Task::BarbarianFishing { player },
    )?;
    message(ctx, player, "You cast out your line...")?;
    message(ctx, player, "You attempt to catch a fish.")?;
    Ok(true)
}

/// `KnifeWithFish`'s `onUseWith` lambda: `fish` is the id of the item the
/// knife was used with.
pub fn knife_with_fish(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    fish: i32,
) -> Result<bool> {
    // submitIndividualPulse(player, FishCuttingPulse(player, fish.id)).
    ctx.defer(
        Service::PulseManagerRun,
        &[player.into()],
        Task::FishCutting { player, fish },
    )?;
    Ok(true)
}

/// `BarbFishingPulse`: a `SkillPulse` whose constructor leaves it stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct FishingPulse {
    pub player: Handle,
    /// `Pulse.delay`.
    pub delay: i32,
    pub running: bool,
}
impl FishingPulse {
    /// `SkillPulse(player, NPC(2722))`: `super(1, player, node)`, then
    /// `super.stop()`.
    pub fn new(player: Handle) -> Self {
        Self {
            player,
            delay: 1,
            running: false,
        }
    }
    /// `SkillPulse.start()`.
    pub fn start(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        if self.check_requirements(ctx)? {
            self.running = true;
        }
        Ok(())
    }
    /// `SkillPulse.pulse()`.
    pub fn pulse(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        if !self.check_requirements(ctx)? {
            return Ok(true);
        }
        animate(ctx, 622)?;
        self.reward(ctx)
    }
    /// `SkillPulse.stop()`.
    pub fn stop(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        ctx.unit(Service::PlayerAnimate, &[(-1).into(), "HIGH".into()])?;
        self.running = false;
        Ok(())
    }
    pub fn check_requirements(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        if !has_level_dyn(ctx, FISHING, 48)? {
            dialogue(
                ctx,
                "You need a Fishing level of at least 48 to catch these fish.",
            )?;
            return Ok(false);
        }
        if !has_level_dyn(ctx, AGILITY, 15)? || !has_level_dyn(ctx, STRENGTH, 15)? {
            dialogue(
                ctx,
                "You need a strength and agility level of at least 15 to fish here.",
            )?;
            return Ok(false);
        }
        if !in_inventory(ctx, BARBARIAN_ROD)? {
            dialogue(ctx, "You need a barbarian fishing rod to fish here.")?;
            return Ok(false);
        }
        if free_slots(ctx)? == 0 {
            dialogue(ctx, "You can't carry any more fish.")?;
            return Ok(false);
        }
        let mut any = false;
        for bait in BAITS {
            if in_inventory(ctx, bait)? {
                any = true;
                break;
            }
        }
        if !any {
            dialogue(ctx, "You don't have any bait with which to fish.")?;
            return Ok(false);
        }
        Ok(true)
    }
    fn reward(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        if self.delay == 1 {
            self.delay = 5;
            return Ok(false);
        }
        let fishing_level = dyn_level(ctx, FISHING)?;
        let boosted_level =
            fishing_level.wrapping_add(ctx.int(Service::ApiGetFamiliarBoost, &[FISHING.into()])?);
        let strength = dyn_level(ctx, STRENGTH)?;
        let agility = dyn_level(ctx, AGILITY)?;

        let mut caught = None;
        for name in ["LEAPING_TROUT", "LEAPING_SALMON", "LEAPING_STURGEON"] {
            let f = Fish::named(name).ok_or(Error::NullPointer)?;
            if f.level > fishing_level {
                continue;
            }
            if name == "LEAPING_SALMON" && (strength < 30 || agility < 30) {
                continue;
            }
            if name == "LEAPING_STURGEON" && (strength < 45 || agility < 45) {
                continue;
            }
            if random::random_between(ctx, 0.0, 1.0)? < f.success_chance(boosted_level) {
                caught = Some(f);
                break;
            }
        }

        if let Some(caught) = caught {
            // baits.firstOrNull { removeItem(player, Item(it, 1)) }.
            for bait in BAITS {
                if remove_item(ctx, bait)? {
                    break;
                }
            }
            ctx.unit(
                Service::ApiAddItem,
                &[caught.id.into(), 1.into(), "INVENTORY".into()],
            )?;
            reward_xp(ctx, FISHING, caught.experience)?;
            let str_agi_xp = match caught.name {
                "LEAPING_SALMON" => 6.0,
                "LEAPING_STURGEON" => 7.0,
                _ => 5.0,
            };
            reward_xp(ctx, AGILITY, str_agi_xp)?;
            reward_xp(ctx, STRENGTH, str_agi_xp)?;
            let fish_caught = int_attribute(ctx, FISH_KEY)?.wrapping_add(1);
            ctx.unit(
                Service::ApiSetAttribute,
                &[SAVED_FISH_KEY.into(), fish_caught.into()],
            )?;
            let name = ctx.text(Service::ApiGetItemName, &[caught.id.into()])?;
            let text = format!("You catch a {}.", lowercase_root(&name));
            message(ctx, self.player, &text)?;
            if int_attribute(ctx, ATTRIBUTE_ROD)? == 1 {
                ctx.unit(Service::ApiSetAttribute, &[ATTRIBUTE_ROD.into(), 2.into()])?;
                dialogue(
                    ctx,
                    "You feel you have learned more of barbarian ways. Otto might wish to talk to you more.",
                )?;
            }
        }
        self.delay = 5;
        Ok(free_slots(ctx)? == 0)
    }
}

/// `FishCuttingPulse.pulse()` (a `Pulse(0)`).
pub fn fish_cutting_pulse(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    fish: i32,
) -> Result<bool> {
    let odds = match fish {
        LEAPING_TROUT => 100,
        LEAPING_SALMON => 85,
        LEAPING_STURGEON => 70,
        _ => return Ok(false),
    };
    let offcut_success = random::random(ctx, odds)? < 50;
    let roe_success = random::random(ctx, 100)? < 50;
    if !remove_item(ctx, fish)? {
        return Ok(false);
    }
    animate(ctx, 5244)?;
    let sturgeon = fish == LEAPING_STURGEON;
    let (extract, xp) = if sturgeon {
        (CAVIAR, 15.0)
    } else {
        (ROE, 10.0)
    };
    if offcut_success && roe_success {
        add_item_or_drop(ctx, FISH_OFFCUTS)?;
        add_item_or_drop(ctx, extract)?;
        reward_xp(ctx, COOKING, xp)?;
        message(
            ctx,
            player,
            if sturgeon {
                "You cut open the fish and extract some fish cuts and caviar."
            } else {
                "You cut open the fish and extract some fish cuts and roe."
            },
        )?;
    } else if roe_success {
        add_item_or_drop(ctx, extract)?;
        reward_xp(ctx, COOKING, xp)?;
        message(
            ctx,
            player,
            if sturgeon {
                "You cut open the fish and extract caviar, but the rest of the fish is reduced to"
            } else {
                "You cut open the fish and extract roe, but the rest of the fish is reduced to"
            },
        )?;
        message(ctx, player, "useless fragments, which you discard.")?;
    } else {
        message(
            ctx,
            player,
            "You fail to gain anything useful and reduce the fish to fragments, not even",
        )?;
        message(ctx, player, "usable as bait.")?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Draws {
        ranges: Vec<i32>,
        picks: Vec<i32>,
        asked: Vec<i32>,
    }
    impl Spots for Draws {
        type Error = ();
        fn random_range(&mut self, a: i32, b: i32) -> std::result::Result<i32, ()> {
            assert_eq!((a, b), (400, 2000));
            Ok(self.ranges.remove(0))
        }
        fn kotlin_next_int(&mut self, until: i32) -> std::result::Result<i32, ()> {
            self.asked.push(until);
            Ok(self.picks.remove(0))
        }
    }

    #[test]
    fn startup_takes_free_locations_in_list_order() {
        let mut draws = Draws {
            ranges: vec![500, 600, 700, 800, 900],
            picks: vec![1, 1, 0, 0, 0],
            asked: vec![],
        };
        let mut m = Manager::default();
        for _ in 0..SPOTS {
            m.add_spot(&mut draws).unwrap();
        }
        // Seven entries; taking (2504,3497) removes both of its entries.
        assert_eq!(draws.asked, vec![7, 5, 4, 3, 2]);
        let locs: Vec<Tile> = m.spots.iter().map(|s| s.loc.unwrap()).collect();
        assert_eq!(
            locs,
            vec![
                [2504, 3497, 0],
                [2500, 3506, 0],
                [2506, 3494, 0],
                [2500, 3509, 0],
                [2500, 3512, 0],
            ]
        );
        assert_eq!(m.used, locs);
        assert_eq!(m.spots[4].ttl, 900);
        // No location is left for a sixth pick but (2504,3516).
        assert_eq!(
            new_loc(&mut draws_of(vec![0]), &mut m.used),
            Ok([2504, 3516, 0])
        );
        assert_eq!(
            new_loc(&mut draws_of(vec![]), &mut m.used),
            Err(SpotError::NoSuchElement)
        );
    }
    fn draws_of(picks: Vec<i32>) -> Draws {
        Draws {
            ranges: vec![],
            picks,
            asked: vec![],
        }
    }

    #[test]
    fn a_spot_counts_down_then_moves_on_the_next_tick() {
        let at = [2506, 3494, 0];
        let mut m = Manager {
            ticks: 0,
            spots: vec![Spot {
                loc: Some(at),
                ttl: 1,
            }],
            used: vec![at],
        };
        let mut draws = Draws {
            ranges: vec![1234],
            picks: vec![2],
            asked: vec![],
        };
        // ttl 1: `ttl-- <= 0` is false.
        assert_eq!(m.handle_tick_actions(&mut draws, 0, Some(at)), Ok(None));
        assert_eq!(m.spots[0].ttl, 0);
        // ttl 0: the location is freed and a new one picked from all seven.
        assert_eq!(m.handle_tick_actions(&mut draws, 0, Some(at)), Ok(None));
        assert_eq!(draws.asked, vec![7]);
        assert_eq!(
            m.spots[0],
            Spot {
                loc: Some([2504, 3497, 0]),
                ttl: -1
            }
        );
        assert_eq!(m.used, vec![[2504, 3497, 0]]);
        // The tick listener rebuilds the used list from the spots.
        m.tick();
        assert_eq!(m.used, vec![[2504, 3497, 0]]);
        // Standing elsewhere: teleport and a new time to live, then the
        // decrement.
        assert_eq!(
            m.handle_tick_actions(&mut draws, 0, Some(at)),
            Ok(Some(Some([2504, 3497, 0])))
        );
        assert_eq!(m.spots[0].ttl, 1233);
    }

    #[derive(Default)]
    struct World {
        levels: HashMap<i32, i32>,
        items: HashMap<i32, i32>,
        attributes: HashMap<String, i32>,
        free: i32,
        doubles: Vec<f64>,
        ints: Vec<i32>,
        log: Vec<String>,
    }
    fn int(a: &Arg) -> i32 {
        match a {
            Arg::Int(v) => *v as i32,
            other => panic!("not an int: {other:?}"),
        }
    }
    fn text(a: &Arg) -> &str {
        match a {
            Arg::Text(v) => v,
            other => panic!("not text: {other:?}"),
        }
    }
    impl Context for World {
        fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
            use Service as S;
            Ok(match service {
                S::ApiHasLevelDyn => {
                    (self.levels.get(&int(&args[0])).copied().unwrap_or(1) >= int(&args[1])).into()
                }
                S::ApiGetDynLevel => self.levels.get(&int(&args[0])).copied().unwrap_or(1).into(),
                S::ApiGetFamiliarBoost => 0.into(),
                S::ApiInInventory => {
                    (self.items.get(&int(&args[0])).copied().unwrap_or(0) >= int(&args[1])).into()
                }
                S::ApiFreeSlots => self.free.into(),
                S::ApiRemoveItem => {
                    let held = self.items.entry(int(&args[0])).or_default();
                    let removed = *held > 0;
                    if removed {
                        *held -= 1;
                        self.log.push(format!("remove {}", int(&args[0])));
                    }
                    removed.into()
                }
                S::ApiAddItem | S::ApiAddItemOrDrop => {
                    *self.items.entry(int(&args[0])).or_default() += int(&args[1]);
                    self.log.push(format!("add {}", int(&args[0])));
                    true.into()
                }
                S::ApiRewardXp => {
                    self.log.push(format!("xp {} {:?}", int(&args[0]), args[1]));
                    Arg::Null
                }
                S::ApiSendMessage => {
                    self.log.push(format!("message {}", text(&args[1])));
                    Arg::Null
                }
                S::ApiSendDialogue => {
                    self.log.push(format!("dialogue {}", text(&args[0])));
                    Arg::Null
                }
                S::ApiAnimate | S::PlayerAnimate => {
                    self.log.push(format!("animate {}", int(&args[0])));
                    Arg::Null
                }
                S::ApiGetAttribute => self
                    .attributes
                    .get(text(&args[0]).trim_start_matches("/save:"))
                    .copied()
                    .unwrap_or(int(&args[1]))
                    .into(),
                S::ApiSetAttribute => {
                    let key = text(&args[0]).trim_start_matches("/save:").to_owned();
                    self.attributes.insert(key, int(&args[1]));
                    Arg::Null
                }
                S::ApiGetItemName => "Leaping trout".into(),
                S::RngSharedNextDouble => self.doubles.remove(0).into(),
                S::RngSharedNextInt => self.ints.remove(0).into(),
                other => panic!("unexpected service {other:?}"),
            })
        }
        fn defer(&mut self, service: Service, _: &[Arg], task: Task) -> Result<()> {
            assert_eq!(service, Service::PulseManagerRun);
            self.log.push(format!("run {task:?}"));
            Ok(())
        }
    }
    fn angler() -> World {
        World {
            levels: HashMap::from([(FISHING, 48), (AGILITY, 15), (STRENGTH, 15)]),
            items: HashMap::from([(BARBARIAN_ROD, 1), (314, 2)]),
            free: 5,
            ..World::default()
        }
    }

    #[test]
    fn requirements_are_refused_in_source_order() {
        let mut pulse = FishingPulse::new(1);
        let mut w = angler();
        w.levels.insert(FISHING, 47);
        pulse.start(&mut w).unwrap();
        assert!(!pulse.running);
        assert_eq!(
            w.log,
            vec!["dialogue You need a Fishing level of at least 48 to catch these fish."]
        );
        let mut w = angler();
        w.items.remove(&314);
        pulse.start(&mut w).unwrap();
        assert_eq!(
            w.log,
            vec!["dialogue You don't have any bait with which to fish."]
        );
        let mut w = angler();
        pulse.start(&mut w).unwrap();
        assert!(pulse.running && w.log.is_empty());
    }

    #[test]
    fn the_first_pulse_only_sets_the_delay_then_a_trout_is_caught() {
        let mut pulse = FishingPulse::new(1);
        let mut w = angler();
        w.attributes.insert("barbtraining:rod".into(), 1);
        assert_eq!(pulse.pulse(&mut w), Ok(false));
        assert_eq!(
            (pulse.delay, w.log.clone()),
            (5, vec!["animate 622".to_owned()])
        );
        // Level 48: only the trout is tried; a draw below its chance catches.
        w.log.clear();
        w.doubles = vec![0.0];
        assert_eq!(pulse.pulse(&mut w), Ok(false));
        assert_eq!(
            w.log,
            vec![
                "animate 622",
                "remove 314",
                "add 11328",
                "xp 10 Double(50.0)",
                "xp 16 Double(5.0)",
                "xp 2 Double(5.0)",
                "message You catch a leaping trout.",
                "dialogue You feel you have learned more of barbarian ways. Otto might wish to talk to you more.",
            ]
        );
        assert_eq!(w.attributes["stats_manager:fish_caught"], 1);
        assert_eq!(w.attributes["barbtraining:rod"], 2);
        // A miss gives nothing; a full pack ends the pulse.
        w.log.clear();
        w.doubles = vec![0.999];
        w.free = 0;
        assert_eq!(pulse.pulse(&mut w), Ok(true));
        assert_eq!(w.log, vec!["dialogue You can't carry any more fish."]);
    }

    #[test]
    fn listeners_run_their_pulses() {
        let mut w = angler();
        assert_eq!(use_rod(&mut w, 1), Ok(true));
        assert_eq!(
            w.log,
            vec![
                "run BarbarianFishing { player: 1 }",
                "message You cast out your line...",
                "message You attempt to catch a fish.",
            ]
        );
        w.log.clear();
        assert_eq!(knife_with_fish(&mut w, 1, LEAPING_SALMON), Ok(true));
        assert_eq!(w.log, vec!["run FishCutting { player: 1, fish: 11330 }"]);
    }

    #[test]
    fn the_bed_gives_one_rod_after_otto() {
        let mut w = World::default();
        search_bed(&mut w, 1).unwrap();
        w.attributes.insert("barbtraining:rod".into(), 1);
        search_bed(&mut w, 1).unwrap();
        search_bed(&mut w, 1).unwrap();
        assert_eq!(
            w.log,
            vec![
                "message Maybe I should speak to Otto before looking under his bed.",
                "add 11323",
                "message You find a heavy fishing rod under the bed and take it.",
                "message You find nothing under the bed.",
            ]
        );
    }

    #[test]
    fn cutting_a_fish() {
        let mut w = World::default();
        w.items.insert(LEAPING_STURGEON, 3);
        // Both rolls pass: offcuts and caviar.
        w.ints = vec![49, 49, 49, 50, 50, 49, 0, 0];
        assert_eq!(fish_cutting_pulse(&mut w, 1, LEAPING_STURGEON), Ok(true));
        // Only the offcut roll passes: nothing.
        assert_eq!(fish_cutting_pulse(&mut w, 1, LEAPING_STURGEON), Ok(true));
        // Only the roe roll passes: caviar.
        assert_eq!(fish_cutting_pulse(&mut w, 1, LEAPING_STURGEON), Ok(true));
        // No fish left: the pulse keeps running after its two draws.
        assert_eq!(fish_cutting_pulse(&mut w, 1, LEAPING_STURGEON), Ok(false));
        assert!(w.ints.is_empty());
        assert_eq!(
            w.log,
            vec![
                "remove 11332",
                "animate 5244",
                "add 11334",
                "add 11326",
                "xp 7 Double(15.0)",
                "message You cut open the fish and extract some fish cuts and caviar.",
                "remove 11332",
                "animate 5244",
                "message You fail to gain anything useful and reduce the fish to fragments, not even",
                "message usable as bait.",
                "remove 11332",
                "animate 5244",
                "add 11326",
                "xp 7 Double(15.0)",
                "message You cut open the fish and extract caviar, but the rest of the fish is reduced to",
                "message useless fragments, which you discard.",
            ]
        );
        // Another item is no fish: no draw.
        assert_eq!(fish_cutting_pulse(&mut w, 1, 317), Ok(false));
    }
}
