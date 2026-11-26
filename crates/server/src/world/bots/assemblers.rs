//! `CombatBotAssembler.kt` and `SkillingBotAssembler.kt`.
//!
//! Kept as the original has it: `produce` assembles a melee bot for the
//! mage type; `assembleRangedBot` compares `crossbow ?: (Random().nextInt()
//! % 2)` with 0, so a given `crossbow` (true or false) never equals it and
//! only the drawn int can choose the crossbow; `equipHighest` asks for every
//! requirement of every item, compares integer averages of `1 + levels`
//! over `1 + count`, and picks at random among the ties; `equipSet` tests
//! the *inventory* slot before it equips.
use super::{
    AiPlayer, Error, Handler, Host, Item, Location, Result, Which, builder, skill, tables as t,
};
use crate::world::content_api::java_double_to_int;

/// `CombatBotAssembler.Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Range,
    Mage,
    Melee,
}

/// `CombatBotAssembler.Tier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Low,
    Med,
    High,
    Pure,
}

/// `SkillingBotAssembler.Wealth`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wealth {
    Poor,
    Average,
    Rich,
}

const BRONZE_BOLTS: i32 = 877;
const BRONZE_ARROW: i32 = 882;
const ARROWS_SLOT: i32 = 13;

/// `produce(type, tier, location)`.
pub fn produce<H: Host>(
    host: &mut H,
    kind: Type,
    tier: Tier,
    location: Location,
) -> Result<AiPlayer<H>> {
    match kind {
        Type::Range => assemble_ranged_bot(host, tier, location, None),
        Type::Melee | Type::Mage => assemble_melee_bot(host, tier, location),
    }
}

/// `assembleRangedBot(tier, location, crossbow)`.
pub fn assemble_ranged_bot<H: Host>(
    host: &mut H,
    tier: Tier,
    location: Location,
    crossbow: Option<bool>,
) -> Result<AiPlayer<H>> {
    let bot = builder::combat_bot(host, location)?;
    generate_stats(host, &bot, tier, &[skill::RANGE, skill::DEFENCE])?;
    // A Boolean never equals the Int 0.
    let crossbow = match crossbow {
        Some(_) => false,
        None => host.new_random_int()? % 2 == 0,
    };
    gear_ranged_bot(host, &bot, Some(crossbow))?;
    Ok(bot)
}

/// `assembleMeleeBot(tier, location)`.
pub fn assemble_melee_bot<H: Host>(
    host: &mut H,
    tier: Tier,
    location: Location,
) -> Result<AiPlayer<H>> {
    let bot = builder::combat_bot(host, location)?;
    generate_stats(
        host,
        &bot,
        tier,
        &[skill::ATTACK, skill::STRENGTH, skill::DEFENCE],
    )?;
    gear_melee_bot(host, &bot)?;
    Ok(bot)
}

/// Sets a static level and then the dynamic level of each pair, in two
/// passes, as the gear functions do.
fn set_levels<H: Host>(host: &mut H, bot: &H::Node, levels: &[(i32, i32)]) -> Result<()> {
    for (skill, level) in levels {
        host.skills_set_static_level(bot, *skill, *level)?;
    }
    for (skill, level) in levels {
        host.skills_set_level(bot, *skill, *level)?;
    }
    Ok(())
}

/// `MeleeAdventurer(tier, location)`.
pub fn melee_adventurer<H: Host>(
    host: &mut H,
    tier: Tier,
    location: Location,
) -> Result<AiPlayer<H>> {
    let bot = builder::combat_bot(host, location)?;
    let node = bot.node();
    let level = host.random_between(25, 69)?;
    generate_stats(
        host,
        &bot,
        tier,
        &[skill::ATTACK, skill::STRENGTH, skill::DEFENCE],
    )?;
    set_levels(
        host,
        &node,
        &[
            (skill::HITPOINTS, level),
            (skill::ATTACK, level.wrapping_add(5)),
            (skill::STRENGTH, level.wrapping_add(5)),
        ],
    )?;
    host.skills_update_combat_level(&node)?;
    for set in [
        t::MELEE_HELMS,
        t::MELEE_TOP,
        t::MELEE_LEG,
        t::MELEE_WEP,
        t::MELEE_SHIELD,
        t::CAPE,
        t::NNECK,
        t::NGLOVES,
        t::NBOOTS,
    ] {
        equip_highest(host, &bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    Ok(bot)
}

/// `RangeAdventurer(tier, location)`.
pub fn range_adventurer<H: Host>(
    host: &mut H,
    tier: Tier,
    location: Location,
) -> Result<AiPlayer<H>> {
    let bot = builder::combat_bot(host, location)?;
    let node = bot.node();
    let level = host.random_between(35, 69)?;
    generate_stats(host, &bot, tier, &[skill::ATTACK, skill::STRENGTH])?;
    set_levels(
        host,
        &node,
        &[
            (skill::HITPOINTS, level),
            (skill::DEFENCE, level),
            (skill::RANGE, level.wrapping_add(10)),
        ],
    )?;
    host.skills_update_combat_level(&node)?;
    equip_highest(host, &bot, t::RANGE_HELMS, Some(65))?;
    equip_highest(host, &bot, t::RANGE_TOPS, Some(65))?;
    equip_highest(host, &bot, t::RANGE_LEGS, Some(65))?;
    equip_highest(host, &bot, t::CROSSBOWS, Some(50))?;
    for set in [
        t::CAPE,
        t::NRANGENECK,
        t::NRANGESHIELD,
        t::PCRANGE_BACK,
        t::NGLOVES,
        t::NRBOOTS,
    ] {
        equip_highest(host, &bot, set, None)?;
    }
    host.equipment_add_slot(
        &node,
        Item::new(BRONZE_BOLTS, 100_000),
        ARROWS_SLOT,
        false,
        false,
    )?;
    host.container_refresh(&node, Which::Equipment)?;
    Ok(bot)
}

/// `assembleMeleeDragonBot(tier, location)`.
pub fn assemble_melee_dragon_bot<H: Host>(
    host: &mut H,
    tier: Tier,
    location: Location,
) -> Result<AiPlayer<H>> {
    let bot = builder::combat_bot(host, location)?;
    generate_stats(
        host,
        &bot,
        tier,
        &[skill::ATTACK, skill::STRENGTH, skill::DEFENCE],
    )?;
    equip_highest(host, &bot, t::MELEE_HELMS, Some(50))?;
    equip_highest(host, &bot, t::MELEE_TOP, Some(40))?;
    equip_highest(host, &bot, t::MELEE_LEG, Some(40))?;
    equip_highest(host, &bot, t::MELEE_WEP, Some(60))?;
    equip_highest(host, &bot, t::CAPE, None)?;
    equip_highest(host, &bot, t::NGLOVES, None)?;
    equip_highest(host, &bot, t::NBOOTS, None)?;
    host.container_refresh(&bot.node(), Which::Equipment)?;
    Ok(bot)
}

/// `assembleRangeDragonBot(tier, location)`.
pub fn assemble_range_dragon_bot<H: Host>(
    host: &mut H,
    tier: Tier,
    location: Location,
) -> Result<AiPlayer<H>> {
    let bot = builder::combat_bot(host, location)?;
    let node = bot.node();
    host.full_restore(&node)?;
    generate_stats(host, &bot, tier, &[skill::RANGE, skill::DEFENCE])?;
    equip_highest(host, &bot, t::RANGE_HELMS, Some(50))?;
    equip_highest(host, &bot, t::RANGE_TOPS, Some(50))?;
    equip_highest(host, &bot, t::RANGE_LEGS, Some(50))?;
    equip_highest(host, &bot, t::CROSSBOWS, Some(50))?;
    equip_highest(host, &bot, t::CAPE, None)?;
    equip_highest(host, &bot, t::NGLOVES, None)?;
    equip_highest(host, &bot, t::NRBOOTS, None)?;
    host.equipment_add_slot(
        &node,
        Item::new(BRONZE_BOLTS, 100_000),
        ARROWS_SLOT,
        false,
        false,
    )?;
    host.container_refresh(&node, Which::Equipment)?;
    Ok(bot)
}

/// The weapon and ammunition of a ranged bot: a crossbow with a shield and
/// bolts, or a bow with arrows.
fn ranged_weapon<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    crossbow: Option<bool>,
    crossbows: &[i32],
    bows: &[i32],
) -> Result<()> {
    let node = bot.node();
    if crossbow == Some(true) {
        equip_highest(host, bot, crossbows, None)?;
        equip_highest(host, bot, t::MELEE_SHIELD, None)?;
        host.equipment_add_slot(
            &node,
            Item::new(BRONZE_BOLTS, i32::MAX),
            ARROWS_SLOT,
            false,
            false,
        )?;
    } else {
        equip_highest(host, bot, bows, None)?;
        host.equipment_add_slot(
            &node,
            Item::new(BRONZE_ARROW, i32::MAX),
            ARROWS_SLOT,
            false,
            false,
        )?;
    }
    Ok(())
}

/// `gearRangedBot(bot, crossbow)`.
pub fn gear_ranged_bot<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    crossbow: Option<bool>,
) -> Result<()> {
    for set in [
        t::RANGE_HELMS,
        t::RANGE_TOPS,
        t::RANGE_LEGS,
        t::CAPE,
        t::NGLOVES,
        t::NRBOOTS,
    ] {
        equip_highest(host, bot, set, None)?;
    }
    ranged_weapon(host, bot, crossbow, t::CROSSBOWS, t::BOWS)?;
    host.container_refresh(&bot.node(), Which::Equipment)
}

/// `gearMeleeBot(bot)`.
pub fn gear_melee_bot<H: Host>(host: &mut H, bot: &AiPlayer<H>) -> Result<()> {
    for set in [
        t::MELEE_HELMS,
        t::MELEE_LEG,
        t::MELEE_SHIELD,
        t::MELEE_TOP,
        t::CAPE,
        t::NGLOVES,
        t::NBOOTS,
        t::MELEE_WEP,
    ] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&bot.node(), Which::Equipment)
}

/// `gearPCnRangedBot(bot, crossbow)`: a novice pest control ranger.
pub fn gear_pc_n_ranged_bot<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    crossbow: Option<bool>,
) -> Result<()> {
    let node = bot.node();
    let level = host.random_between(30, 70)?;
    host.full_restore(&node)?;
    set_levels(
        host,
        &node,
        &[
            (skill::RANGE, 50),
            (skill::DEFENCE, 50),
            (skill::ATTACK, level),
            (skill::STRENGTH, level),
            (skill::HITPOINTS, level),
        ],
    )?;
    host.skills_update_combat_level(&node)?;
    for set in [
        t::PCRANGE_HELMS,
        t::PCRANGE_TOPS,
        t::PCRANGE_BACK,
        t::PCRANGE_LEGS,
    ] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    for set in [t::NECK, t::GLOVES, t::BOOTS, t::RING_ARCH] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    ranged_weapon(host, bot, crossbow, t::PCCROSSBOWS, t::PCBOWS)?;
    host.skills_set_static_level(&node, skill::RANGE, 99)?;
    host.skills_set_level(&node, skill::RANGE, 99)?;
    host.container_refresh(&node, Which::Equipment)
}

/// The shared body of `gearPCnMeleeBot` and `gearPCiMeleeBot`.
fn gear_pc_melee<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    first: &[(i32, i32)],
    last: &[(i32, i32)],
) -> Result<()> {
    let node = bot.node();
    host.full_restore(&node)?;
    set_levels(host, &node, first)?;
    host.skills_update_combat_level(&node)?;
    for set in [
        t::PCMELEE_HELMS,
        t::PCMELEE_LEG,
        t::PCMELEE_SHIELD,
        t::PCMELEE_TOP,
        t::PCMELEE_WEP,
    ] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    for set in [t::CAPE, t::NECK, t::GLOVES, t::BOOTS, t::RING_BERS] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    set_levels(host, &node, last)?;
    host.full_restore(&node)
}

/// `gearPCnMeleeBot(bot)`: a novice pest control fighter.
pub fn gear_pc_n_melee_bot<H: Host>(host: &mut H, bot: &AiPlayer<H>) -> Result<()> {
    let level = host.random_between(30, 75)?;
    gear_pc_melee(
        host,
        bot,
        &[
            (skill::STRENGTH, level),
            (skill::DEFENCE, level),
            (skill::ATTACK, level),
            (skill::HITPOINTS, level),
            (skill::PRAYER, 70),
            (skill::RANGE, 10),
            (skill::MAGIC, 10),
        ],
        &[
            (skill::DEFENCE, 70),
            (skill::ATTACK, 99),
            (skill::STRENGTH, 99),
            (skill::HITPOINTS, 80),
        ],
    )
}

/// `gearPCiRangedBot(bot, crossbow)`: an intermediate pest control ranger.
pub fn gear_pc_i_ranged_bot<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    crossbow: Option<bool>,
) -> Result<()> {
    let node = bot.node();
    let level = host.random_between(50, 80)?;
    host.full_restore(&node)?;
    set_levels(
        host,
        &node,
        &[
            (skill::RANGE, level),
            (skill::DEFENCE, 80),
            (skill::ATTACK, level),
            (skill::STRENGTH, level),
            (skill::HITPOINTS, 70),
            (skill::SUMMONING, 99),
        ],
    )?;
    host.skills_update_combat_level(&node)?;
    for set in [
        t::PCRANGE_HELMS,
        t::PCRANGE_TOPS,
        t::PCRANGE_BACK,
        t::PCRANGE_LEGS,
    ] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    for set in [t::GLOVES, t::NECK, t::BOOTS, t::RING_ARCH] {
        equip_highest(host, bot, set, None)?;
    }
    host.container_refresh(&node, Which::Equipment)?;
    ranged_weapon(host, bot, crossbow, t::PCCROSSBOWS, t::PCBOWS)?;
    host.skills_set_static_level(&node, skill::RANGE, 99)?;
    host.skills_set_level(&node, skill::RANGE, 99)?;
    host.container_refresh(&node, Which::Equipment)
}

/// `gearPCiMeleeBot(bot)`: an intermediate pest control fighter.
pub fn gear_pc_i_melee_bot<H: Host>(host: &mut H, bot: &AiPlayer<H>) -> Result<()> {
    let level = host.random_between(55, 95)?;
    gear_pc_melee(
        host,
        bot,
        &[
            (skill::STRENGTH, level),
            (skill::DEFENCE, level),
            (skill::ATTACK, level),
            (skill::HITPOINTS, level),
            (skill::PRAYER, 99),
            (skill::RANGE, level),
            (skill::MAGIC, level),
            (skill::SUMMONING, 99),
        ],
        &[
            (skill::DEFENCE, 99),
            (skill::ATTACK, 99),
            (skill::STRENGTH, 99),
            (skill::HITPOINTS, 99),
        ],
    )
}

/// `generateStats(bot, tier, skills...)`: each named skill gets the tier's
/// drawn level moved by up to half of 33 either way and held in `1..=max`;
/// a pure's six fixed levels follow; a fifth of the skills' mean experience
/// goes to hitpoints, whose static level becomes 10 plus that
/// experience's level.
pub fn generate_stats<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    tier: Tier,
    skills: &[i32],
) -> Result<()> {
    let node = bot.node();
    let mut total_xp_add = 0.0_f64;
    let mut skill_amt = 0.0_f64;
    let variance = 0.5_f64;
    let (initial, max) = match tier {
        Tier::Low => (host.random(33)?, 33),
        Tier::Med => (host.random_between(33, 66)?, 66),
        Tier::High => (host.random_between(66, 99)?, 99),
        Tier::Pure => (host.random_between(90, 99)?, 99),
    };
    for skill in skills {
        let perc = host.random_double(-variance, variance)?;
        let mut level = initial.wrapping_add(java_double_to_int(perc * 33.0));
        if level < 1 {
            level = 1;
        }
        if level > max {
            level = max;
        }
        host.skills_set_level(&node, *skill, level)?;
        host.skills_set_static_level(&node, *skill, level)?;
        total_xp_add += host.skills_experience(&node, *skill)?;
        skill_amt += 1.0;
    }
    if tier == Tier::Pure {
        for (skill, level) in [
            (skill::DEFENCE, 10),
            (skill::STRENGTH, 99),
            (skill::ATTACK, 90),
            (skill::PRAYER, 43),
            (skill::RANGE, 1),
            (skill::MAGIC, 1),
        ] {
            host.skills_set_static_level(&node, skill, level)?;
        }
    }
    host.skills_add_experience(&node, skill::HITPOINTS, (total_xp_add / skill_amt) * 0.2)?;
    let new_hp = host.skills_level_from_xp(&node, (total_xp_add / skill_amt) * 0.2)?;
    host.skills_set_static_level(&node, skill::HITPOINTS, 10_i32.wrapping_add(new_hp))?;
    host.skills_update_combat_level(&node)?;
    host.full_restore(&node)
}

/// `handlers.getOrDefault("requirements", null) as HashMap<Int, Int>?`.
fn requirements<H: Host>(host: &mut H, id: i32) -> Result<Option<Vec<(i32, i32)>>> {
    match host.item_handler(id, "requirements")? {
        None => Ok(None),
        Some(Handler::Requirements(map)) => Ok(Some(map)),
        Some(_) => Err(Error::Cast),
    }
}

/// `Item.lvlAvg()`: `(1 + sum of the required levels) / (1 + their count)`.
pub fn lvl_avg<H: Host>(host: &mut H, item: Item) -> Result<i32> {
    let mut total: i32 = 1;
    let mut count: i32 = 1;
    if let Some(map) = requirements(host, item.definition)? {
        for (_, level) in map {
            total = total.wrapping_add(level);
            count += 1;
        }
    }
    Ok(total / count)
}

/// The private `equipHighest(bot, set, levelcap)`: of the items the bot's
/// levels (and the cap) allow, one of those with the highest `lvlAvg`, into
/// the slot the first of them names.
pub fn equip_highest<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    set: &[i32],
    levelcap: Option<i32>,
) -> Result<()> {
    let node = bot.node();
    let mut highest_items: Vec<Item> = Vec::new();
    let mut highest: Option<Item> = None;
    for id in set {
        let item = Item::one(*id);
        let mut can_equip = true;
        if let Some(map) = requirements(host, item.definition)? {
            for (skill, level) in map {
                let short = host.skills_level(&node, skill)? < level;
                if short || levelcap.is_some_and(|cap| level > cap) {
                    can_equip = false;
                }
            }
        }
        if can_equip {
            let Some(best) = highest else {
                highest = Some(item);
                highest_items.push(item);
                continue;
            };
            if lvl_avg(host, item)? > lvl_avg(host, best)? {
                highest = Some(item);
                highest_items.clear();
                highest_items.push(item);
            } else if lvl_avg(host, item)? == lvl_avg(host, best)? {
                highest_items.push(item);
            }
        }
    }
    if highest_items.is_empty() {
        return Err(Error::NoSuchElement);
    }
    let index = host.kotlin_random_index(highest_items.len() as i32)?;
    let picked = usize::try_from(index)
        .ok()
        .and_then(|i| highest_items.get(i).copied())
        .ok_or(Error::Index)?;
    let highest = highest.ok_or(Error::Null)?;
    let slot = match host.item_handler(highest.definition, "equipment_slot")? {
        Some(Handler::Int(slot)) => slot,
        Some(_) => return Err(Error::Cast),
        None => return Err(Error::Null),
    };
    host.equipment_add_slot(&node, picked, slot, false, false)?;
    Ok(())
}

/// `SkillingBotAssembler.produce(type, loc)`.
pub fn skilling_produce<H: Host>(
    host: &mut H,
    wealth: Wealth,
    location: Location,
) -> Result<AiPlayer<H>> {
    let bot = AiPlayer::new(host, Some(location))?;
    assemble_bot(host, &bot, wealth)?;
    Ok(bot)
}

/// `SkillingBotAssembler.assembleBot(bot, type)`: a random set of the
/// wealth's table.
pub fn assemble_bot<H: Host>(host: &mut H, bot: &AiPlayer<H>, wealth: Wealth) -> Result<()> {
    let sets = match wealth {
        Wealth::Poor => t::POORSETS,
        Wealth::Average => t::AVGSETS,
        Wealth::Rich => t::RICHSETS,
    };
    let index = host.kotlin_random_index(sets.len() as i32)?;
    let set = usize::try_from(index)
        .ok()
        .and_then(|i| sets.get(i))
        .ok_or(Error::Index)?;
    equip_set(host, bot, set)
}

/// `SkillingBotAssembler.equipSet(bot, set)`: each item with an equipment
/// slot is equipped while the *inventory* slot of that number is empty, and
/// its requirements become the bot's static levels either way.
pub fn equip_set<H: Host>(host: &mut H, bot: &AiPlayer<H>, set: &[i32]) -> Result<()> {
    let node = bot.node();
    for id in set {
        let item = Item::one(*id);
        let Some(slot) = host.item_handler(item.definition, "equipment_slot")? else {
            continue;
        };
        let Handler::Int(slot) = slot else {
            return Err(Error::Cast);
        };
        if host.container_get(&node, Which::Inventory, slot)?.is_none() {
            host.equipment_add_slot(&node, item, slot, false, false)?;
        }
        if host
            .item_handler(item.definition, "requirements")?
            .is_some()
        {
            match host.item_handler(item.definition, "requirements")? {
                Some(Handler::Requirements(requirements)) => {
                    for (skill, level) in requirements {
                        host.skills_set_static_level(&node, skill, level)?;
                    }
                }
                Some(_) => return Err(Error::Cast),
                None => return Err(Error::Null),
            }
        }
    }
    host.skills_update_combat_level(&node)
}
