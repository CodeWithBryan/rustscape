//! `Fish`, `FishingOption`, `FishingSpot`, `FishingListener` and the legacy
//! `FishingPulse` (with the `SkillPulse` methods it inherits), statement for
//! statement. The listener and the pulse differ in almost every detail (bait
//! and space checks, the perk roll, the counter attribute, experience hand-off,
//! barehand fishing) and each keeps its own source's behaviour.
//!
//! Spot movement (`FishingNPC`, `FishSpots`), barbarian fishing and the option
//! handlers that start the pulse are not ported.
use super::{
    random,
    service::{Arg, Calls, Context, Error, Handle, Result, Service, Task},
    tables,
    tools::FISHING,
};
use rs09_game_core::{java_lower::lowercase_root, java_string::fold_case_unit};
use std::sync::OnceLock;

const STRENGTH: i32 = 2;
const SKILLING: i32 = 7;
const ANIMATION_END: usize = 1;
const FISHING_BAIT: i32 = 313;
const STRIPY_FEATHER: i32 = 10087;
const HARPOON_ITEM: i32 = 311;
const BARB_TAIL_HARPOON: i32 = 10129;
const RAW_TUNA: i32 = 359;
const RAW_SWORDFISH: i32 = 371;
const RAW_SHARK: i32 = 383;
const SWORDFISH_GLOVES: i32 = 12860;
const SHARK_GLOVES: i32 = 12861;
/// `BarbarianTraining.attributeFist`.
const ATTRIBUTE_FIST: &str = "/save:barbtraining:fist";
const FISH_KEY: &str = "stats_manager:fish_caught";
const SAVED_FISH_KEY: &str = "/save:stats_manager:fish_caught";
/// `FremennikAchievementDiary.HardTasks.CATCH_TUNA_WITHOUT_HARPOON`.
const CATCH_TUNA_WITHOUT_HARPOON: i32 = 5;
const BAREHAND_FISHING_START_ANIM: i32 = 6703;
const BAREHAND_FISHING_LOOP_ANIM: i32 = 6704;
const SECOND_FISH: &str = "%RYour expert aim catches you a second fish.";

#[derive(Clone, Debug, PartialEq)]
pub struct Fish {
    pub name: &'static str,
    pub ordinal: usize,
    pub id: i32,
    pub level: i32,
    pub experience: f64,
    pub low_chance: f64,
    pub high_chance: f64,
}
impl Fish {
    pub fn values() -> &'static [Fish] {
        static ROWS: OnceLock<Vec<Fish>> = OnceLock::new();
        ROWS.get_or_init(|| {
            tables::FISH_ROWS
                .iter()
                .enumerate()
                .map(
                    |(ordinal, &(name, id, level, experience, low_chance, high_chance))| Fish {
                        name,
                        ordinal,
                        id,
                        level,
                        experience,
                        low_chance,
                        high_chance,
                    },
                )
                .collect()
        })
    }
    pub fn named(name: &str) -> Option<&'static Fish> {
        Self::values().iter().find(|f| f.name == name)
    }
    /// `Fish.forItem`: `fishMap` is filled in declaration order, so the last
    /// row with an id owns it.
    pub fn for_item(id: i32) -> Option<&'static Fish> {
        Self::values().iter().rev().find(|f| f.id == id)
    }
    /// `Fish.getBigFish`: the last put for a fish owns its key.
    pub fn big_fish(&self) -> Option<i32> {
        lookup(tables::BIG_FISH_ROWS, self.ordinal)
    }
    pub fn barehand_fishing_req(&self) -> Option<i32> {
        lookup(tables::BAREHAND_FISHING_REQ, self.ordinal)
    }
    pub fn barehand_strength_req(&self) -> Option<i32> {
        lookup(tables::BAREHAND_STRENGTH_REQ, self.ordinal)
    }
    pub fn barehand_strength_xp(&self) -> Option<f64> {
        lookup(tables::BAREHAND_STRENGTH_XP, self.ordinal)
    }
    /// `Fish.getSuccessChance(level)`.
    pub fn success_chance(&self, level: i32) -> f64 {
        (f64::from(level) - 1.0) * ((self.high_chance - self.low_chance) / (99.0 - 1.0))
            + self.low_chance
    }
    fn is(&self, name: &str) -> bool {
        self.name == name
    }
}
fn lookup<T: Copy>(rows: &[(usize, T)], ordinal: usize) -> Option<T> {
    rows.iter().rev().find(|r| r.0 == ordinal).map(|r| r.1)
}

#[derive(Clone, Debug, PartialEq)]
pub struct FishingOption {
    pub name: &'static str,
    pub ordinal: usize,
    pub tool: i32,
    pub level: i32,
    pub animation: i32,
    pub bait: Option<&'static [i32]>,
    pub option: &'static str,
    /// `Fish` ordinals, in constructor order.
    pub fish: &'static [usize],
}
impl FishingOption {
    pub fn values() -> &'static [FishingOption] {
        static ROWS: OnceLock<Vec<FishingOption>> = OnceLock::new();
        ROWS.get_or_init(|| {
            tables::FISHING_OPTION_ROWS
                .iter()
                .enumerate()
                .map(
                    |(ordinal, &(name, tool, level, animation, bait, option, fish))| {
                        FishingOption {
                            name,
                            ordinal,
                            tool,
                            level,
                            animation,
                            bait,
                            option,
                            fish,
                        }
                    },
                )
                .collect()
        })
    }
    pub fn named(name: &str) -> Option<&'static FishingOption> {
        Self::values().iter().find(|o| o.name == name)
    }
    /// `FishingOption.forName`: `nameMap` is filled in declaration order, so
    /// the last row with an option name owns it.
    pub fn for_name(option: &str) -> Option<&'static FishingOption> {
        Self::values().iter().rev().find(|o| o.option == option)
    }
    fn is(&self, name: &str) -> bool {
        self.name == name
    }
    fn is_harpoon(&self) -> bool {
        self.is("HARPOON") || self.is("SHARK_HARPOON")
    }
    fn catches(&self, fish: &str) -> bool {
        self.fish.iter().any(|&f| Fish::values()[f].is(fish))
    }
    /// `FishingOption.getStartMessage`.
    pub fn start_message(&self) -> &'static str {
        if self.option == "net" {
            "You cast out your net..."
        } else {
            "You attempt to catch a fish."
        }
    }
    /// `FishingOption.rollFish`.
    pub fn roll_fish(&self, ctx: &mut (impl Context + ?Sized)) -> Result<Option<&'static Fish>> {
        if self.is("BIG_NET") {
            match random::randomize(ctx, 100)? {
                0 => return Ok(Fish::named("OYSTER")),
                50 => return Ok(Fish::named("CASKET")),
                90 => return Ok(Fish::named("SEAWEED")),
                _ => {}
            }
        }
        let vlvl = ctx.int(Service::ApiGetDynLevel, &[FISHING.into()])?;
        let ilvl = vlvl.wrapping_add(ctx.int(Service::FamiliarGetBoost, &[FISHING.into()])?);
        for &f in self.fish {
            let f = &Fish::values()[f];
            if f.level > vlvl {
                continue;
            }
            if self.is("LURE") && in_inventory(ctx, STRIPY_FEATHER)? != f.is("RAINBOW_FISH") {
                continue;
            }
            let chance = f.success_chance(ilvl);
            if random::random_between(ctx, 0.0, 1.0)? < chance {
                return Ok(Some(f));
            }
        }
        Ok(None)
    }
    /// `FishingOption.getBaitName`.
    pub fn bait_name(&self, ctx: &mut (impl Context + ?Sized)) -> Result<String> {
        match self.bait {
            Some(bait) if !bait.is_empty() => item_name(ctx, bait[0]),
            _ => Ok("none".into()),
        }
    }
    /// `FishingOption.hasBait`: once one bait is found the rest are not asked for.
    pub fn has_bait(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let Some(bait) = self.bait else {
            return Ok(true);
        };
        let mut any = false;
        for &b in bait {
            any = any || in_inventory(ctx, b)?;
        }
        Ok(any)
    }
    /// `FishingOption.removeBait`: the last bait is tried first.
    pub fn remove_bait(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let Some(bait) = self.bait else {
            return Ok(true);
        };
        for &b in bait.iter().rev() {
            if ctx.boolean(Service::ApiRemoveItem, &[b.into(), "INVENTORY".into()])? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FishingSpot {
    pub name: &'static str,
    pub ordinal: usize,
    pub ids: &'static [i32],
    /// `FishingOption` ordinals, in constructor order.
    pub options: &'static [usize],
}
impl FishingSpot {
    pub fn values() -> &'static [FishingSpot] {
        static ROWS: OnceLock<Vec<FishingSpot>> = OnceLock::new();
        ROWS.get_or_init(|| {
            tables::FISHING_SPOT_ROWS
                .iter()
                .enumerate()
                .map(|(ordinal, &(name, ids, options))| FishingSpot {
                    name,
                    ordinal,
                    ids,
                    options,
                })
                .collect()
        })
    }
    /// `FishingSpot.forId`: `spotMap` is filled in declaration order, so the
    /// last row holding an id owns it.
    pub fn for_id(npc: i32) -> Option<&'static FishingSpot> {
        Self::values().iter().rev().find(|s| s.ids.contains(&npc))
    }
    /// `FishingSpot.getAllIds`: every id of every row, repeats included.
    pub fn all_ids() -> Vec<i32> {
        Self::values()
            .iter()
            .flat_map(|s| s.ids.iter().copied())
            .collect()
    }
    /// `FishingSpot.getOptionByName`: the first option whose name equals the
    /// ROOT-lowered argument.
    pub fn option_by_name(&self, op: &str) -> Option<&'static FishingOption> {
        let lowered = lowercase_root(op);
        self.options
            .iter()
            .map(|&o| &FishingOption::values()[o])
            .find(|o| o.option == lowered)
    }
}

/// What `FishingListener.defineListeners` registers.
pub struct Registration {
    /// `FishingSpot.values().flatMap { it.ids.toList() }`.
    pub ids: Vec<i32>,
    pub options: [&'static str; 6],
    pub persistent: bool,
    pub allowed_distance: i32,
}
pub fn registration() -> Registration {
    Registration {
        ids: FishingSpot::all_ids(),
        options: ["net", "lure", "bait", "harpoon", "cage", "fish"],
        persistent: true,
        allowed_distance: 1,
    }
}
/// `defineListeners`: one NPC interaction registration.
pub fn define_listeners(ctx: &mut (impl Context + ?Sized)) -> Result<()> {
    let r = registration();
    ctx.unit(
        Service::ListenersDefineInteraction,
        &[
            "NPC".into(),
            Arg::List(r.ids.iter().map(|&i| i.into()).collect()),
            Arg::List(r.options.iter().map(|&o| o.into()).collect()),
            r.persistent.into(),
            r.allowed_distance.into(),
        ],
    )
}

fn in_inventory(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<bool> {
    ctx.boolean(Service::ApiInInventory, &[id.into(), 1.into()])
}
fn in_equipment(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<bool> {
    ctx.boolean(Service::ApiInEquipment, &[id.into(), 1.into()])
}
fn has_level_dyn(ctx: &mut (impl Context + ?Sized), skill: i32, level: i32) -> Result<bool> {
    ctx.boolean(Service::ApiHasLevelDyn, &[skill.into(), level.into()])
}
fn item_name(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<String> {
    ctx.text(Service::ApiGetItemName, &[id.into()])
}
fn message(ctx: &mut (impl Context + ?Sized), player: Handle, text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendMessage, &[player.into(), text.into()])
}
fn dialogue(ctx: &mut (impl Context + ?Sized), text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendDialogue, &[text.into()])
}
fn clear_scripts(ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
    ctx.boolean(Service::ApiClearScripts, &[])
}
fn delay_skilling(ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
    ctx.boolean(
        Service::ApiDelayClock,
        &[SKILLING.into(), 5.into(), false.into()],
    )
}
fn api_animate(ctx: &mut (impl Context + ?Sized), animation: i32) -> Result<()> {
    ctx.unit(
        Service::ApiAnimate,
        &[animation.into(), Arg::Null, false.into()],
    )
}
fn add_item_or_drop(ctx: &mut (impl Context + ?Sized), id: i32, amount: i32) -> Result<()> {
    ctx.unit(Service::ApiAddItemOrDrop, &[id.into(), amount.into()])
}
/// The spot's location and the identity of the `Location` object holding it.
fn npc_location(ctx: &mut (impl Context + ?Sized), node: Handle) -> Result<([i32; 3], i64)> {
    match ctx.call(Service::NpcLocation, &[node.into()])? {
        Arg::List(v) => match v.as_slice() {
            [Arg::Int(x), Arg::Int(y), Arg::Int(z), Arg::Int(identity)] => {
                Ok(([*x as i32, *y as i32, *z as i32], *identity))
            }
            _ => Err(Error::Reply),
        },
        _ => Err(Error::Reply),
    }
}
/// `player.familiarManager.familiar`: `None` is null, otherwise whether the
/// object is a `Forager` and the owner's token for it.
fn familiar(ctx: &mut (impl Context + ?Sized)) -> Result<Option<(bool, Handle)>> {
    match ctx.call(Service::FamiliarGetFamiliar, &[])? {
        Arg::Null => Ok(None),
        Arg::List(v) => match v.as_slice() {
            [Arg::Bool(forager), Arg::Int(handle)] => Ok(Some((*forager, *handle))),
            _ => Err(Error::Reply),
        },
        _ => Err(Error::Reply),
    }
}
/// `hasFamiliar() && familiar is Forager`, then `familiar as Forager`: the
/// field is read a second time for the cast.
fn forager(ctx: &mut (impl Context + ?Sized)) -> Result<Option<Handle>> {
    if ctx.boolean(Service::FamiliarHasFamiliar, &[])? && familiar(ctx)?.is_some_and(|f| f.0) {
        return match familiar(ctx)? {
            None => Err(Error::NullPointer),
            Some((false, _)) => Err(Error::ClassCast),
            Some((true, handle)) => Ok(Some(handle)),
        };
    }
    Ok(None)
}
/// `player.location.transform(player.direction)`: one step, same plane.
fn step_ahead(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<[i32; 3]> {
    let at = ctx.location(Service::PlayerLocation, &[player.into()])?;
    let (dx, dy) = match ctx.call(Service::PlayerDirection, &[])? {
        Arg::List(v) => match v.as_slice() {
            [Arg::Int(x), Arg::Int(y)] => (*x as i32, *y as i32),
            _ => return Err(Error::Reply),
        },
        _ => return Err(Error::Reply),
    };
    Ok([at[0].wrapping_add(dx), at[1].wrapping_add(dy), at[2]])
}
/// `Location.withinMaxnormDistance`, with Kotlin's wrapping `abs`.
fn within_maxnorm_distance(this: [i32; 3], other: [i32; 3], dist: i32) -> bool {
    if other[2] != this[2] {
        return false;
    }
    let dx = other[0].wrapping_sub(this[0]).wrapping_abs();
    let dy = other[1].wrapping_sub(this[1]).wrapping_abs();
    dx.max(dy) <= dist
}
/// Kotlin `contains(other, ignoreCase = true)`: `regionMatches` at every
/// UTF-16 offset, comparing the JDK's per-unit case folding.
fn contains_ignore_case(text: &str, other: &str) -> bool {
    let hay: Vec<u16> = text.encode_utf16().map(fold_case_unit).collect();
    let needle: Vec<u16> = other.encode_utf16().map(fold_case_unit).collect();
    needle.is_empty() || hay.windows(needle.len()).any(|w| w == needle.as_slice())
}
/// `core.tools.colorize(line)`.
pub fn colorize(line: &str) -> String {
    let mut text = line.to_owned();
    for (key, colour) in [
        ("%R", "<col=ff0000>"),
        ("%O", "<col=ff6600>"),
        ("%Y", "<col=ffff00>"),
        ("%G", "<col=66ff33>"),
        ("%B", "<col=3366ff>"),
        ("%P", "<col=cc00ff>"),
    ] {
        text = text.replace(key, colour);
    }
    // Regex("%[0-9a-fA-F]{6}") -> "<col=${value.substring(1)}>", left to right.
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%'
            && i + 6 < chars.len()
            && chars[i + 1..i + 7].iter().all(char::is_ascii_hexdigit)
        {
            out.push_str("<col=");
            out.extend(&chars[i + 1..i + 7]);
            out.push('>');
            i += 7;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out.push_str("</col>");
    out.push(' ');
    out
}
/// The catch message shared by the listener and `FishingPulse.message(2)`.
fn catch_message(ctx: &mut (impl Context + ?Sized), fish: &Fish) -> Result<String> {
    let mut msg = String::from(match fish.name {
        "ANCHOVIE" | "SHRIMP" | "SEAWEED" => "You catch some ",
        "OYSTER" => "You catch an ",
        _ => "You catch a ",
    });
    msg += &lowercase_root(&item_name(ctx, fish.id)?)
        .replace("raw ", "")
        .replace("big ", "");
    msg += if fish.is("SHARK") { "!" } else { "." };
    Ok(msg)
}
/// The missing-tool dialogue text shared by both `checkRequirements`.
fn missing_tool(ctx: &mut (impl Context + ?Sized), option: &FishingOption) -> Result<String> {
    let mut msg = String::from("You need a ");
    if contains_ignore_case(&item_name(ctx, option.tool)?, "net") {
        msg += "net to ";
    } else {
        msg += &lowercase_root(&item_name(ctx, option.tool)?);
        msg += " to ";
    }
    if option.option == "lure" || option.option == "bait" {
        msg += option.option;
        msg += " these fish.";
    } else {
        msg += "catch these fish.";
    }
    Ok(msg)
}
/// The missing-bait dialogue text shared by both `checkRequirements`.
fn missing_bait(ctx: &mut (impl Context + ?Sized), option: &FishingOption) -> Result<String> {
    let mut msg = String::from("You don't have any ");
    msg += &lowercase_root(&option.bait_name(ctx)?);
    msg += if option.bait_name(ctx)? == item_name(ctx, FISHING_BAIT)? {
        " left."
    } else {
        "s left."
    };
    Ok(msg)
}
fn level_text(option: &FishingOption) -> String {
    format!(
        "You need a Fishing level of at least {} to {} these fish.",
        option.level, option.option
    )
}
/// `hasBarbTail`: the same body in the listener and the pulse.
fn has_barb_tail(ctx: &mut (impl Context + ?Sized), option: &FishingOption) -> Result<bool> {
    Ok(option.is_harpoon()
        && (in_inventory(ctx, BARB_TAIL_HARPOON)? || in_equipment(ctx, BARB_TAIL_HARPOON)?))
}

/// `FishingListener.canBarehandFish`.
fn can_barehand_fish(ctx: &mut (impl Context + ?Sized), option: &FishingOption) -> Result<bool> {
    if !option.is_harpoon() {
        return Ok(false);
    }
    // hasAnyHarpoon
    if in_inventory(ctx, HARPOON_ITEM)?
        || in_inventory(ctx, BARB_TAIL_HARPOON)?
        || in_equipment(ctx, BARB_TAIL_HARPOON)?
    {
        return Ok(false);
    }
    // hasBarehandUnlock
    if fist(ctx)? < 1 {
        return Ok(false);
    }
    let fish = Fish::named(if option.is("HARPOON") {
        "TUNA"
    } else {
        "SHARK"
    })
    .ok_or(Error::NullPointer)?;
    Ok(has_level_dyn(
        ctx,
        FISHING,
        fish.barehand_fishing_req().ok_or(Error::NullPointer)?,
    )? && has_level_dyn(
        ctx,
        STRENGTH,
        fish.barehand_strength_req().ok_or(Error::NullPointer)?,
    )?)
}
/// `getAttribute(player, BarbarianTraining.attributeFist, 0)` used as an Int.
fn fist(ctx: &mut (impl Context + ?Sized)) -> Result<i32> {
    match ctx.call(Service::ApiGetAttribute, &[ATTRIBUTE_FIST.into(), 0.into()])? {
        Arg::Int(v) => i32::try_from(v).map_err(|_| Error::Reply),
        _ => Err(Error::ClassCast),
    }
}
/// `FishingListener.meetsBarehandRequirements`.
fn meets_barehand_requirements(ctx: &mut (impl Context + ?Sized), fish: &Fish) -> Result<bool> {
    let Some(fishing) = fish.barehand_fishing_req() else {
        return Ok(false);
    };
    let Some(strength) = fish.barehand_strength_req() else {
        return Ok(false);
    };
    Ok(has_level_dyn(ctx, FISHING, fishing)? && has_level_dyn(ctx, STRENGTH, strength)?)
}
/// `FishingListener.anim`.
fn listener_anim(
    ctx: &mut (impl Context + ?Sized),
    option: &FishingOption,
    barehand: bool,
) -> Result<()> {
    if barehand {
        let clocks = match ctx.call(Service::PlayerClocks, &[])? {
            Arg::List(v) => v,
            _ => return Err(Error::Reply),
        };
        let end = match clocks.get(ANIMATION_END) {
            Some(Arg::Int(v)) => *v,
            Some(_) => return Err(Error::Reply),
            None => return Err(Error::IndexOutOfBounds),
        };
        if end <= i64::from(ctx.int(Service::GameWorldTicks, &[])?) {
            api_animate(ctx, BAREHAND_FISHING_LOOP_ANIM)?;
        }
    } else if ctx.boolean(Service::ApiAnimationFinished, &[])? {
        api_animate(ctx, option.animation)?;
    }
    Ok(())
}
/// `FishingListener.checkRequirements`.
pub fn check_fishing_requirements(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    option: &FishingOption,
    node: Handle,
    barehand: bool,
) -> Result<bool> {
    if !in_inventory(ctx, option.tool)? && !has_barb_tail(ctx, option)? && !barehand {
        let msg = missing_tool(ctx, option)?;
        dialogue(ctx, &msg)?;
        return Ok(false);
    }
    if !option.has_bait(ctx)? {
        let msg = missing_bait(ctx, option)?;
        dialogue(ctx, &msg)?;
        return Ok(false);
    }
    if !barehand && !has_level_dyn(ctx, FISHING, option.level)? {
        dialogue(ctx, &level_text(option))?;
        return Ok(false);
    }
    if ctx.int(Service::ApiFreeSlots, &[])? == 0 {
        dialogue(
            ctx,
            if option.catches("LOBSTER") {
                "You can't carry any more lobsters."
            } else {
                "You can't carry any more fish."
            },
        )?;
        return Ok(false);
    }
    if !ctx.boolean(Service::NodeIsActive, &[node.into()])? {
        return Ok(false);
    }
    let (spot, _) = npc_location(ctx, node)?;
    let at = ctx.location(Service::PlayerLocation, &[player.into()])?;
    Ok(within_maxnorm_distance(spot, at, 1))
}

/// `FishingListener.handleFishing`. `npc` is `None` when the interacted node
/// is not an NPC (`node as? NPC`).
pub fn handle_fishing(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    npc: Option<Handle>,
    state: i32,
) -> Result<bool> {
    let Some(node) = npc else {
        return clear_scripts(ctx);
    };
    let Some(spot) = FishingSpot::for_id(ctx.int(Service::NodeId, &[node.into()])?) else {
        return clear_scripts(ctx);
    };
    let used = ctx.text(Service::ApiGetUsedOption, &[])?;
    let Some(op) = spot.option_by_name(&used) else {
        return clear_scripts(ctx);
    };
    let barehand = can_barehand_fish(ctx, op)?;
    let forager = forager(ctx)?;
    if !ctx.boolean(Service::ApiFinishedMoving, &[])? {
        return ctx.boolean(Service::ApiRestartScript, &[]);
    }
    if state == 0 {
        if !check_fishing_requirements(ctx, player, op, node, barehand)? {
            return clear_scripts(ctx);
        }
        if let Some(it) = forager {
            let dest = step_ahead(ctx, player)?;
            let path = ctx.handle(Service::PathfinderFind, &[it.into(), dest.into()])?;
            ctx.unit(Service::PathWalk, &[path.into(), it.into()])?;
        }
        if barehand {
            api_animate(ctx, BAREHAND_FISHING_START_ANIM)?;
        }
        match op.option {
            "cage" => message(
                ctx,
                player,
                if spot.name == "CAGE_HARPOON" {
                    "You attempt to catch a lobster."
                } else {
                    "You attempt to catch a crayfish."
                },
            )?,
            "harpoon" => message(
                ctx,
                player,
                if barehand {
                    "You start to lure the fish."
                } else {
                    "You start harpooning fish."
                },
            )?,
            "net" => message(ctx, player, "You cast out your net...")?,
            "bait" | "lure" => {
                message(ctx, player, "You cast out your line...")?;
                message(ctx, player, "You attempt to catch a fish.")?;
            }
            _ => message(ctx, player, "You attempt to catch some fish...")?,
        }
    }

    if ctx.boolean(Service::ApiClockReady, &[SKILLING.into()])? {
        listener_anim(ctx, op, barehand)?;
        if let Some(it) = forager {
            ctx.unit(Service::ForagerHandlePassiveAction, &[it.into()])?;
        }
        let Some(fish) = op.roll_fish(ctx)? else {
            return delay_skilling(ctx);
        };
        if barehand && !meets_barehand_requirements(ctx, fish)? {
            return delay_skilling(ctx);
        }
        if !ctx.boolean(Service::ApiHasSpaceFor, &[fish.id.into(), 1.into()])?
            || !op.remove_bait(ctx)?
        {
            return ctx.boolean(Service::ApiRestartScript, &[]);
        }
        if barehand {
            api_animate(
                ctx,
                match fish.name {
                    "TUNA" => 6710,
                    "SWORDFISH" => 6707,
                    "SHARK" => 6705,
                    _ => 6703,
                },
            )?;
            if fish.is("TUNA") {
                ctx.unit(
                    Service::DiaryFinishTask,
                    &[
                        "FREMENNIK".into(),
                        2.into(),
                        CATCH_TUNA_WITHOUT_HARPOON.into(),
                    ],
                )?;
            }
        }
        // `fish.getItem()` is `Item(fish.id)`: amount one.
        ctx.unit(
            Service::PlayerDispatch,
            &[fish.id.into(), 1.into(), node.into(), (-1).into()],
        )?;
        let (item, amount) = (fish.id, 1);
        let big = fish.big_fish();
        let big_chance = if ctx.boolean(Service::GameWorldSettings, &[])?
            && ctx.boolean(Service::GameSettingsIsDevMode, &[])?
        {
            10
        } else {
            5000
        };
        match big {
            Some(big) if random::roll(ctx, big_chance)? => {
                let text = format!(
                    "You catch an enormous{}!",
                    lowercase_root(&item_name(ctx, fish.id)?).replace("raw", "")
                );
                message(ctx, player, &text)?;
                add_item_or_drop(ctx, big, 1)?;
            }
            _ => {
                let msg = catch_message(ctx, fish)?;
                message(ctx, player, &msg)?;
                add_item_or_drop(ctx, item, amount)?;
            }
        }

        if ctx.boolean(Service::SkillcapePerksIsActive, &["GREAT_AIM".into()])?
            && random::roll(ctx, 20)?
        {
            add_item_or_drop(ctx, item, amount)?;
            message(ctx, player, &colorize(SECOND_FISH))?;
            ctx.unit(Service::PlayerIncrementAttribute, &[SAVED_FISH_KEY.into()])?;
        }
        ctx.unit(Service::PlayerIncrementAttribute, &[SAVED_FISH_KEY.into()])?;
        let mut xp = fish.experience;
        if (item == RAW_SWORDFISH && in_equipment(ctx, SWORDFISH_GLOVES)?)
            || (item == RAW_SHARK && in_equipment(ctx, SHARK_GLOVES)?)
        {
            xp += 100.0;
            ctx.unit(Service::FogGlovesUpdateCharges, &[])?;
        }
        ctx.unit(Service::ApiRewardXp, &[FISHING.into(), xp.into()])?;
        if barehand {
            let bonus = fish.barehand_strength_xp().ok_or(Error::NullPointer)?;
            ctx.unit(Service::ApiRewardXp, &[STRENGTH.into(), bonus.into()])?;
            if fist(ctx)? == 1 {
                ctx.unit(Service::ApiSetAttribute, &[ATTRIBUTE_FIST.into(), 2.into()])?;
                dialogue(
                    ctx,
                    "You feel you have learned more of barbarian ways. Otto might wish to talk to you more.",
                )?;
            }
        }

        delay_skilling(ctx)?;
        if !check_fishing_requirements(ctx, player, op, node, barehand)? {
            return clear_scripts(ctx);
        }
    } else if barehand {
        listener_anim(ctx, op, barehand)?;
    }
    ctx.boolean(Service::ApiKeepRunning, &[])
}

/// `FishingPulse.checkFish`: every comparison reads the level again.
fn check_fish(ctx: &mut (impl Context + ?Sized)) -> Result<i32> {
    fn at_least(ctx: &mut (impl Context + ?Sized), fishing: i32, strength: i32) -> Result<bool> {
        Ok(
            ctx.int(Service::SkillsGetLevel, &[FISHING.into()])? >= fishing
                && ctx.int(Service::SkillsGetLevel, &[STRENGTH.into()])? >= strength,
        )
    }
    Ok(if at_least(ctx, 55, 35)? {
        if at_least(ctx, 70, 50)? {
            if at_least(ctx, 96, 76)? { 3 } else { 2 }
        } else {
            1
        }
    } else {
        0
    })
}
fn player_animate(ctx: &mut (impl Context + ?Sized), animation: i32) -> Result<()> {
    ctx.unit(Service::PlayerAnimate, &[animation.into(), Arg::Null])
}
/// `FishingPulse.getCatchAnimationAndLoot`.
fn catch_animation_and_loot(ctx: &mut (impl Context + ?Sized), node: Handle) -> Result<i32> {
    fn loot(
        ctx: &mut (impl Context + ?Sized),
        animation: i32,
        fishing: f64,
        strength: f64,
        item: i32,
    ) -> Result<()> {
        player_animate(ctx, animation)?;
        ctx.unit(
            Service::SkillsAddExperience2,
            &[FISHING.into(), fishing.into()],
        )?;
        ctx.unit(
            Service::SkillsAddExperience2,
            &[STRENGTH.into(), strength.into()],
        )?;
        ctx.boolean(Service::InventoryAdd, &[item.into(), 1.into()])
            .map(|_| ())
    }
    let fishing_for = check_fish(ctx)?;
    match ctx.int(Service::NodeId, &[node.into()])? {
        324 => match fishing_for {
            1 => loot(ctx, 6710, 80.0, 8.0, RAW_TUNA)?,
            // `RandomFunction.random(1)` draws nextInt(1), which is never one.
            2 | 3 => {
                if random::random(ctx, 1)? == 1 {
                    loot(ctx, 6710, 80.0, 8.0, RAW_TUNA)?;
                } else {
                    loot(ctx, 6707, 100.0, 10.0, RAW_SWORDFISH)?;
                }
            }
            _ => {}
        },
        313 => loot(ctx, 6705, 110.0, 11.0, RAW_SHARK)?,
        _ => {}
    }
    Ok(0)
}
/// Runs a deferred fishing body once; `BarehandCatch` never finishes.
pub fn run_task(ctx: &mut (impl Context + ?Sized), task: &mut Task) -> Result<bool> {
    match task {
        Task::BarehandCatch { node, counter, .. } => {
            let seen = *counter;
            *counter = counter.wrapping_add(1);
            if seen == 5 {
                catch_animation_and_loot(ctx, *node)?;
            }
            Ok(false)
        }
        _ => Err(Error::ClassCast),
    }
}

/// `SkillTasks` constants `FishingPulse.updateSkillTask` decreases, by fish.
fn skill_tasks(fish: &Fish) -> &'static [&'static str] {
    match fish.name {
        "ANCHOVIE" => &["FANCHOVIES1", "FANCHOVIES2"],
        "HERRING" => &["FHERRING1", "FHERRING2"],
        "LOBSTER" => &["FLOBSTER1", "FLOBSTER2"],
        "SALMON" => &["FSALMON1", "FSALMON2"],
        "SHARK" => &["FSHARK1", "FSHARK2", "FSHARK3"],
        "SHRIMP" => &["FSHRIMP1", "FSHRIMP2"],
        "SWORDFISH" => &["FSWORD1", "FSWORD2"],
        "TROUT" => &["FTROUT1", "FTROUT2"],
        "TUNA" => &["FTUNA1", "FTUNA2"],
        _ => &[],
    }
}

/// The legacy `FishingPulse` over `SkillPulse`/`Pulse` fields. Base
/// `Pulse.update` is not part of this port.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse {
    pub player: Handle,
    pub node: Handle,
    pub option: Option<&'static FishingOption>,
    pub fish: Option<&'static Fish>,
    /// `npc.location` as read by the constructor: coordinates and the identity
    /// of the `Location` object, which `checkRequirements` compares with `!==`.
    pub location: ([i32; 3], i64),
    pub running: bool,
    pub delay: i32,
}
impl Pulse {
    /// `FishingPulse(player, npc, option)`: `SkillPulse` stops the new pulse,
    /// then the location initialiser reads the spot.
    pub fn new(
        ctx: &mut (impl Context + ?Sized),
        player: Handle,
        node: Handle,
        option: Option<&'static FishingOption>,
    ) -> Result<Self> {
        Ok(Self {
            player,
            node,
            option,
            fish: None,
            location: npc_location(ctx, node)?,
            running: false,
            delay: 1,
        })
    }
    /// `FishingPulse.start`, then `SkillPulse.start`.
    pub fn start(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        if let Some(forager) = forager(ctx)? {
            let dest = step_ahead(ctx, self.player)?;
            let from = ctx.location(Service::NodeLocation, &[forager.into()])?;
            let path = ctx.handle(Service::PathfinderFindFrom, &[from.into(), dest.into()])?;
            ctx.unit(Service::PathWalk, &[path.into(), forager.into()])?;
        }
        if self.check_requirements(ctx)? {
            self.running = true;
            self.message(ctx, 0)?;
        }
        Ok(())
    }
    /// `SkillPulse.pulse`.
    pub fn pulse(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        if !self.check_requirements(ctx)? {
            return Ok(true);
        }
        self.animate(ctx)?;
        self.reward(ctx)
    }
    /// `SkillPulse.stop`: `resetAnimation` is never cleared by FishingPulse.
    pub fn stop(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        ctx.unit(Service::PlayerAnimate, &[(-1).into(), "HIGH".into()])?;
        self.running = false;
        self.message(ctx, 1)
    }
    /// `FishingPulse.checkRequirements`.
    pub fn check_requirements(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let Some(option) = self.option else {
            return Ok(false);
        };
        let held = in_inventory(ctx, option.tool)?;
        ctx.unit(
            Service::PlayerDebug,
            &[if held { "true" } else { "false" }.into()],
        )?;
        if !in_inventory(ctx, option.tool)? && !has_barb_tail(ctx, option)? {
            let msg = missing_tool(ctx, option)?;
            dialogue(ctx, &msg)?;
            self.stop(ctx)?;
            return Ok(false);
        }
        if !option.has_bait(ctx)? {
            let msg = missing_bait(ctx, option)?;
            dialogue(ctx, &msg)?;
            self.stop(ctx)?;
            return Ok(false);
        }
        if !has_level_dyn(ctx, FISHING, option.level)? {
            dialogue(ctx, &level_text(option))?;
            self.stop(ctx)?;
            return Ok(false);
        }
        if ctx.int(Service::ApiFreeSlots, &[])? == 0 {
            dialogue(
                ctx,
                if option.catches("LOBSTER") {
                    "You can't carry any more lobsters."
                } else {
                    "You can't carry any more fish."
                },
            )?;
            self.stop(ctx)?;
            return Ok(false);
        }
        if self.location.1 != npc_location(ctx, self.node)?.1
            || !ctx.boolean(Service::NodeIsActive, &[self.node.into()])?
            || ctx.boolean(Service::NpcIsInvisible, &[self.node.into()])?
        {
            self.stop(ctx)?;
            return Ok(false);
        }
        Ok(true)
    }
    /// `FishingPulse.isBareHanded`.
    fn is_bare_handed(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let Some(option) = self.option.filter(|o| o.is_harpoon()) else {
            return Ok(false);
        };
        for above in [0, 2] {
            if check_fish(ctx)? > above
                && !(in_inventory(ctx, option.tool)?
                    || in_equipment(ctx, option.tool)?
                    || has_barb_tail(ctx, option)?)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// `FishingPulse.animate`.
    pub fn animate(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        if self.is_bare_handed(ctx)? {
            player_animate(ctx, 6709)?;
            ctx.defer(
                Service::PulserSubmit,
                &[1.into()],
                Task::BarehandCatch {
                    player: self.player,
                    node: self.node,
                    counter: 0,
                },
            )
        } else {
            player_animate(ctx, self.option.ok_or(Error::NullPointer)?.animation)
        }
    }
    /// `FishingPulse.reward`.
    pub fn reward(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        if self.delay == 1 {
            self.delay = 5;
            return Ok(false);
        }
        if let Some(forager) = forager(ctx)? {
            ctx.unit(Service::ForagerHandlePassiveAction, &[forager.into()])?;
        }
        // success(): the delay is no longer one here.
        self.fish = self.option.ok_or(Error::NullPointer)?.roll_fish(ctx)?;
        if let Some(fish) = self.fish
            && ctx.boolean(Service::InventoryHasSpaceFor, &[fish.id.into(), 1.into()])?
            && self.option.ok_or(Error::NullPointer)?.remove_bait(ctx)?
        {
            if ctx.boolean(Service::SkillTasksHasTask, &[])? {
                for &task in skill_tasks(fish) {
                    ctx.unit(Service::SkillTasksDecreaseTask, &[task.into()])?;
                }
            }
            ctx.unit(
                Service::PlayerDispatch,
                &[fish.id.into(), 1.into(), self.node.into(), (-1).into()],
            )?;
            if ctx.boolean(Service::SkillcapePerksIsActive, &["GREAT_AIM".into()])?
                && random::random(ctx, 100)? <= 5
            {
                add_item_or_drop(ctx, fish.id, 1)?;
                ctx.unit(Service::PlayerSendMessage, &[colorize(SECOND_FISH).into()])?;
            }
            add_item_or_drop(ctx, fish.id, 1)?;
            let caught =
                match ctx.call(Service::PlayerGetAttribute, &[FISH_KEY.into(), 0.into()])? {
                    Arg::Int(v) => i32::try_from(v).map_err(|_| Error::Reply)?,
                    _ => return Err(Error::ClassCast),
                };
            ctx.unit(
                Service::PlayerSetAttribute,
                &[SAVED_FISH_KEY.into(), caught.wrapping_add(1).into()],
            )?;
            ctx.unit(
                Service::SkillsAddExperience3,
                &[FISHING.into(), fish.experience.into(), true.into()],
            )?;
            self.message(ctx, 2)?;
        }
        Ok(ctx.int(Service::InventoryFreeSlots, &[])? == 0)
    }
    /// `FishingPulse.message`.
    pub fn message(&mut self, ctx: &mut (impl Context + ?Sized), kind: i32) -> Result<()> {
        match kind {
            0 => message(
                ctx,
                self.player,
                self.option.ok_or(Error::NullPointer)?.start_message(),
            ),
            2 => {
                // The prefix is chosen before `fish!!` is dereferenced.
                let fish = self.fish.ok_or(Error::NullPointer)?;
                let msg = catch_message(ctx, fish)?;
                message(ctx, self.player, &msg)?;
                if ctx.int(Service::InventoryFreeSlots, &[])? == 0 {
                    dialogue(
                        ctx,
                        if fish.is("LOBSTER") {
                            "You can't carry any more lobsters."
                        } else {
                            "You can't carry any more fish."
                        },
                    )?;
                    self.stop(ctx)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
