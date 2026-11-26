//! `AIPBuilder.java`, `CombatBot.kt`, `PvMBots.java` and
//! `PvMBotsBuilder.kt`.
//!
//! `AIPBuilder`'s private `ITEMS` list is an instance field of a class
//! nothing instantiates; it is not carried here.
use super::{
    AiPlayer, BotClass, Error, Food, Gender, Host, Item, Location, NodeKind, Result, Which, skill,
};

/// `AIPBuilder.create(l)`.
pub fn create<H: Host>(host: &mut H, l: Option<Location>) -> Result<AiPlayer<H>> {
    AiPlayer::new(host, l)
}

/// `AIPBuilder.copy(player)`: at the player's location.
pub fn copy<H: Host>(host: &mut H, player: &H::Node) -> Result<AiPlayer<H>> {
    let location = host.location(player);
    copy_at(host, player, location)
}

/// `AIPBuilder.copy(player, l)`: a new artificial player with the player's
/// skills, inventory, equipment, bank and appearance, controlled by it.
pub fn copy_at<H: Host>(
    host: &mut H,
    player: &H::Node,
    l: Option<Location>,
) -> Result<AiPlayer<H>> {
    let p = AiPlayer::new(host, l)?;
    let node = p.node();
    host.skills_copy(&node, player)?;
    host.container_copy(&node, Which::Inventory, player)?;
    host.container_copy(&node, Which::Equipment, player)?;
    host.container_copy(&node, Which::Bank, player)?;
    host.appearance_copy(&node, player)?;
    p.set_controler(Some(player.clone()));
    Ok(p)
}

/// `AIPBuilder.RandomAfkPlayer(loc)`: a standing bot with a drawn gender
/// and six drawn combat levels.
pub fn random_afk_player<H: Host>(host: &mut H, location: Option<Location>) -> Result<()> {
    let bot = AiPlayer::new(host, location)?.node();
    let female = host.random(3)? == 1;
    host.appearance_set_gender(&bot, if female { Gender::Female } else { Gender::Male })?;
    for skill in [
        skill::MAGIC,
        skill::DEFENCE,
        skill::ATTACK,
        skill::STRENGTH,
        skill::RANGE,
        skill::HITPOINTS,
    ] {
        let level = host.get_random(99)?;
        host.skills_set_static_level(&bot, skill, level)?;
    }
    host.skills_update_combat_level(&bot)?;
    host.appearance_sync(&bot)
}

/// Where `AIPBuilder.immersiveSpawns()` puts its standing bots: Edgeville,
/// the Grand Exchange, the Varrock bank beside it and Lunar Isle's bank.
pub const IMMERSIVE_SPAWNS: [(i32, i32); 11] = [
    (3094, 3491),
    (3094, 3493),
    (3092, 3497),
    (3160, 3492),
    (3161, 3491),
    (3165, 3483),
    (3168, 3485),
    (3165, 3485),
    (3164, 3493),
    (3189, 3439),
    (2099, 3918),
];

/// `AIPBuilder.immersiveSpawns()`.
pub fn immersive_spawns<H: Host>(host: &mut H) -> Result<()> {
    for (x, y) in IMMERSIVE_SPAWNS {
        random_afk_player(host, Some(Location::new(x, y, 0)))?;
    }
    Ok(())
}

/// `CombatBot(location)`: the Kotlin constructor's location is not nullable.
pub fn combat_bot<H: Host>(host: &mut H, location: Location) -> Result<AiPlayer<H>> {
    AiPlayer::with_class(host, BotClass::Combat, Some(location))
}

/// `PvMBots(l)`.
pub fn pvm_bots<H: Host>(host: &mut H, l: Option<Location>) -> Result<AiPlayer<H>> {
    AiPlayer::with_class(host, BotClass::Pvm, l)
}

/// `PvMBots(copyFromFile, l)`.
pub fn pvm_bots_from_file<H: Host>(
    host: &mut H,
    copy_from_file: &str,
    l: Option<Location>,
) -> Result<AiPlayer<H>> {
    AiPlayer::with_class_from_file(host, BotClass::Pvm, copy_from_file, l)
}

/// `CombatBot.CheckPrayer(type)` and `PvMBots.CheckPrayer(type)`: every
/// element is toggled; an element is a `PrayerType` name or null.
pub fn check_prayer<H: Host>(
    host: &mut H,
    bot: &AiPlayer<H>,
    types: &[Option<&str>],
) -> Result<()> {
    let node = bot.node();
    for prayer in types {
        host.prayer_toggle(&node, *prayer)?;
    }
    Ok(())
}

/// `CombatBot.eat(foodId)` and `PvMBots.eat(foodId)`: the food is eaten
/// while the hitpoints level is at least three times the lifepoints, and a
/// bot that is not fighting a player and holds none is given five.
///
/// The two differ where the item is not a known consumable: `CombatBot`
/// eats it as `Food(IntArray(food.id), HealingEffect(1))` (an array of that
/// many zeros); `PvMBots` fails on the missing entry.
pub fn eat<H: Host>(host: &mut H, bot: &AiPlayer<H>, food_id: i32) -> Result<()> {
    let node = bot.node();
    let food_item = Item::one(food_id);
    let level = host.skills_static_level(&node, skill::HITPOINTS)?;
    if level >= host.skills_lifepoints(&node)?.wrapping_mul(3)
        && host.container_contains_item(&node, Which::Inventory, food_item)?
    {
        host.lock_for(&node, 3)?;
        let food = host.container_get_item(&node, Which::Inventory, food_item)?;
        let found = food.ok_or(Error::Null)?;
        let consumable = match (host.consumable(found.id)?, bot.class()) {
            (Some(known), _) => Food::Known(known),
            (None, BotClass::Combat) => Food::Plain {
                ids: vec![0; found.id as usize],
                heal: 1,
            },
            (None, _) => return Err(Error::Null),
        };
        host.consume(consumable, food, &node)?;
        host.combat_delay_next_attack(&node, 3)?;
    }
    if !bot.check_victim_is_player(host)?
        && !host.container_contains(&node, Which::Inventory, food_id, 1)?
    {
        host.container_add(&node, Which::Inventory, Item::new(food_id, 5))?;
    }
    Ok(())
}

/// `PvMBots.FindTargets(entity, radius)`: of the first five local NPCs,
/// those `checkValidTargets` passes; `None` when there is none.
pub fn find_targets<H: Host>(
    host: &mut H,
    entity: &H::Node,
    radius: i32,
) -> Result<Option<Vec<H::Node>>> {
    let mut targets = Vec::new();
    let location = host.location(entity);
    let local = host.local_npcs(location, radius)?;
    for npc in local.iter().take(5) {
        if host.kind(npc) != NodeKind::Npc {
            return Err(Error::Cast);
        }
        if check_valid_targets(host, npc)? {
            targets.push(npc.clone());
        }
    }
    Ok(if targets.is_empty() {
        None
    } else {
        Some(targets)
    })
}

/// `PvMBots.checkValidTargets(target)`.
pub fn check_valid_targets<H: Host>(host: &mut H, target: &H::Node) -> Result<bool> {
    if !host.is_active(target) {
        return Ok(false);
    }
    if !host.properties_is_multi_zone(target)? && host.in_combat(target)? {
        return Ok(false);
    }
    host.npc_has_action(target, "attack")
}

/// `PvMBots.AttackNpcsInRadius(bot, radius)` (`AttackNpcsInRadius(radius)`
/// passes the bot itself): true when the player is or will be fighting.
pub fn attack_npcs_in_radius<H: Host>(host: &mut H, bot: &H::Node, radius: i32) -> Result<bool> {
    if host.in_combat(bot)? {
        return Ok(true);
    }
    let Some(creatures) = find_targets(host, bot, radius)? else {
        return Ok(false);
    };
    let index = host.get_random(creatures.len() as i32 - 1)?;
    let target = usize::try_from(index)
        .ok()
        .and_then(|i| creatures.get(i))
        .ok_or(Error::Index)?;
    host.attack(bot, target)?;
    Ok(true)
}

/// `PvMBotsBuilder.create(l)`.
pub fn pvm_create<H: Host>(host: &mut H, l: Option<Location>) -> Result<AiPlayer<H>> {
    host.statics()
        .write(|s| s.bots_spawned = s.bots_spawned.wrapping_add(1));
    pvm_bots(host, l)
}

/// `PvMBotsBuilder.createPestControlTestBot(l)` and, with `second`,
/// `createPestControlTestBot2(l)`: the count goes up before the null check
/// of the location.
pub fn pvm_create_pest_control_test_bot<H: Host>(
    host: &mut H,
    second: bool,
    l: Option<Location>,
) -> Result<H::PestControlBot> {
    host.statics()
        .write(|s| s.bots_spawned = s.bots_spawned.wrapping_add(1));
    host.new_pest_control_test_bot(second, l.ok_or(Error::Null)?)
}
