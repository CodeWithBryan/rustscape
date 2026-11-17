//! The `MagicSpell`, `CombatSpell` and `DefaultCombatSpell` members and the
//! members the combat spell classes override. Each function dispatches on
//! the spell object's class the way the original's virtual call does.
use super::{
    AUTOCAST_ANIMATION, Animation, Audio, DEFAULT_AUDIO_RADIUS, Error, Graphics, Host, NodeKind,
    Projectile, ProjectileTarget, Result, SPELL_FAIL_AUDIO, SPLASH_GRAPHIC, Spell, SpellBook,
    SpellClass, SpellType, StateView, Value, runes, skill, slot,
};

fn read<H: Host>(h: &mut H, spell: &H::Spell) -> Spell {
    h.spell(spell).clone()
}

fn is_player<H: Host>(h: &H, node: &H::Node) -> bool {
    h.kind(node) == NodeKind::Player
}

fn is_npc<H: Host>(h: &H, node: &H::Node) -> bool {
    h.kind(node) == NodeKind::Npc
}

/// `playGlobalAudio(node.getLocation(), id, delay)`: the location is read
/// first and a null location fails Kotlin's parameter check before the call.
fn audio_at<H: Host>(h: &mut H, node: &H::Node, id: i32, delay: i32) -> Result<()> {
    let location = h.location(node)?.ok_or(Error::Null)?;
    h.global_audio(location, id, delay, 1, DEFAULT_AUDIO_RADIUS)
}

/// `getDelay()`: 3 for every spell.
pub fn delay() -> i32 {
    3
}

/// `getExperience(player)`: the constructor's experience.
pub fn experience<H: Host>(h: &mut H, spell: &H::Spell) -> f64 {
    h.spell(spell).experience
}

/// `checkLevelRequirement(caster, message)`. Every spell here is a
/// `CombatSpell`, so the level is read with the assist flag set; only a
/// player's level is read.
pub fn check_level_requirement<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    caster: &H::Node,
    message: bool,
) -> Result<bool> {
    let level = h.spell(spell).level;
    if is_player(h, caster) && h.level_with(caster, skill::MAGIC, true)? < level {
        if message {
            h.packet_message(
                caster,
                &format!("You need a Magic level of {level} to cast this spell."),
            )?;
        }
        return Ok(false);
    }
    Ok(true)
}

/// `GodSpells.getSpellIndex()`: by the amount of the second rune.
fn god_index(s: &Spell) -> Result<i32> {
    let runes = s.runes.as_ref().ok_or(Error::Null)?;
    let second = runes.get(1).ok_or(Error::Index)?;
    Ok(match second.1 {
        2 => 0,
        1 => 1,
        4 => 2,
        _ => -1,
    })
}

const GOD_NAMES: [&str; 3] = ["Saradomin strike", "Guthix claws", "Flames of Zamorak"];
/// `Items.SARADOMIN_STAFF_2415`, `GUTHIX_STAFF_2416`, `ZAMORAK_STAFF_2417`.
const GOD_STAVES: [i32; 3] = [2415, 2416, 2417];

/// `getGodCasts()[index]`.
fn god_cast<H: Host>(h: &mut H, player: &H::Node, index: i32) -> Result<i32> {
    let casts = h.god_casts(player)?.ok_or(Error::Null)?;
    usize::try_from(index)
        .ok()
        .and_then(|i| casts.get(i).copied())
        .ok_or(Error::Index)
}

/// `meetsRequirements(caster, message, remove)`, with `GodSpells`' override.
pub fn meets_requirements<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    caster: &H::Node,
    message: bool,
    remove: bool,
) -> Result<bool> {
    let s = read(h, spell);
    if s.class == SpellClass::God {
        if is_npc(h, caster) {
            return Ok(true);
        }
        if is_player(h, caster) {
            let staff = h.equipment_new(caster, slot::WEAPON)?;
            let index = god_index(&s)?;
            if index < 0 {
                return Ok(false);
            }
            let required = GOD_STAVES[index as usize];
            if god_cast(h, caster, index)? < 100 && !h.in_zone(caster, "mage arena")? {
                let left = 100i32.wrapping_sub(god_cast(h, caster, index)?);
                h.send_message(
                    caster,
                    &format!(
                        "You need to cast {} {left} more times inside the Mage Arena.",
                        GOD_NAMES[index as usize]
                    ),
                )?;
                return Ok(false);
            }
            if staff != required && !(index == 1 && staff == 8841) {
                if message {
                    let name = h.item_name(required);
                    h.packet_message(
                        caster,
                        &format!("You need to wear a {name}  to cast this spell."),
                    )?;
                }
                return Ok(false);
            }
        }
    }
    magic_meets_requirements(h, spell, caster, message, remove)
}

/// `MagicSpell.meetsRequirements`.
fn magic_meets_requirements<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    caster: &H::Node,
    message: bool,
    remove: bool,
) -> Result<bool> {
    if !check_level_requirement(h, spell, caster, message)? {
        return Ok(false);
    }
    let player = is_player(h, caster);
    if player && let Some(auto) = h.autocast(caster)? {
        let first = h.equipment(caster, slot::WEAPON)?.ok_or(Error::Null)?;
        let slayer = h.item_name(first).contains("layer's staff");
        let second = h.equipment(caster, slot::WEAPON)?.ok_or(Error::Null)?;
        let void_knight = h.item_name(second).contains("knight mace");
        let id = h.spell(&auto).spell_id;
        if (id == 31 && !slayer) || (id == 42 && !void_knight) {
            h.packet_message(caster, "You need the proper staff to autocast this spell.")?;
            return Ok(false);
        }
    }
    let id = h.spell(spell).spell_id;
    if (id == 12 || id == 30 || id == 56) && player {
        let delay = attribute_int(h, caster, "entangleDelay", 0)?;
        if delay > h.ticks()? {
            h.send_message(caster, "You have recently cast a binding spell.")?;
            return Ok(false);
        }
    }
    if !player {
        return Ok(true);
    }
    if h.equipment(caster, slot::WEAPON)?.is_some()
        && h.equipment(caster, slot::WEAPON)?.ok_or(Error::Null)? == 14726
    {
        // RandomFunction.getRandom(100): (int) (Math.random() * 101).
        let draw = (h.math_random()? * 101.0) as i32;
        if draw < 13 {
            h.send_message(
                caster,
                "Your staff negates the rune requirement of the spell.",
            )?;
            return Ok(true);
        }
    }
    let Some(required) = h.spell(spell).runes.clone() else {
        return Ok(true);
    };
    if let Some((id, _)) = runes::has_runes(h, caster, &required)? {
        if message {
            let name = h.item_name(id);
            h.packet_message(
                caster,
                &format!("You don't have enough {name}s to cast this spell."),
            )?;
        }
        return Ok(false);
    }
    if remove {
        let list =
            runes::items(h.attribute(caster, runes::SPELL_RUNES, Value::Items(Vec::new()))?)?;
        for (id, amount) in list {
            h.inventory_remove(caster, id, amount)?;
        }
        h.remove_attribute(caster, runes::SPELL_RUNES)?;
    }
    Ok(true)
}

fn attribute_int<H: Host>(h: &mut H, node: &H::Node, key: &str, default: i32) -> Result<i32> {
    match h.attribute(node, key, Value::Int(default))? {
        Value::Int(v) => Ok(v),
        Value::Items(_) => Err(Error::Cast),
    }
}

/// `addExperience(entity, hit)`, with `CurseSpells`' override (the
/// two-argument `addExperience` and nothing for the hit).
pub fn add_experience<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    hit: i32,
) -> Result<()> {
    let s = read(h, spell);
    if s.class == SpellClass::Curse {
        return h.add_experience_default(entity, skill::MAGIC, s.experience);
    }
    h.add_experience(entity, skill::MAGIC, s.experience, true)?;
    if !is_player(h, entity) || hit < 1 {
        return Ok(());
    }
    h.add_experience(entity, skill::HITPOINTS, hit as f64 * 1.33, true)?;
    if h.attack_style(entity)?.ok_or(Error::Null)? == super::STYLE_DEFENSIVE_CAST {
        let base = (h.experience_mod() * hit as f64) / 2.0;
        h.add_experience(entity, skill::DEFENCE, base, true)?;
        h.add_experience(entity, skill::MAGIC, base, true)?;
        return Ok(());
    }
    let modifier = h.experience_mod();
    h.add_experience(entity, skill::MAGIC, hit as f64 * modifier, true)
}

/// `CombatSpell.visualize`.
fn combat_visualize<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    target: Option<&H::Node>,
) -> Result<()> {
    let s = read(h, spell);
    h.graphics(entity, s.graphic)?;
    if let Some(projectile) = &s.projectile {
        let npc = is_npc(h, entity);
        let target = target.ok_or(Error::Null)?;
        if h.kind(target).is_entity() {
            h.send_projectile(
                projectile,
                entity,
                ProjectileTarget::Entity(Some(target)),
                npc,
                58,
                10,
            )?;
        } else {
            let location = h.location(target)?;
            h.send_projectile(
                projectile,
                entity,
                ProjectileTarget::Location(location),
                npc,
                58,
                10,
            )?;
        }
    }
    let auto = h.autocast(entity)?;
    if auto.as_ref().is_some_and(|a| h.same_spell(a, spell))
        && (is_player(h, entity) || s.animation.is_none())
    {
        if !is_player(h, entity) {
            return Err(Error::Cast);
        }
        let current = h.autocast(entity)?.ok_or(Error::Null)?;
        if h.spell(&current).spell_id == 31 {
            h.animate(entity, Some(Animation::new(1576)))?;
        } else {
            h.animate(entity, Some(AUTOCAST_ANIMATION))?;
        }
    } else if is_npc(h, entity) {
        if h.magic_animation(entity)?.is_some() {
            let animation = h.magic_animation(entity)?;
            h.animate(entity, animation)?;
        } else {
            h.animate(entity, s.animation)?;
        }
    } else {
        h.animate(entity, s.animation)?;
    }
    audio_at(h, entity, s.audio.id, 20)
}

/// The `visualize` of `IceSpells`, `BloodSpells`, `ShadowSpells`,
/// `SmokeSpells`, `MiasmicSpells` and `TeleblockSpell`: the projectile is
/// cast to an entity target and never transformed as an NPC's.
fn ancient_visualize<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    target: Option<&H::Node>,
) -> Result<()> {
    let s = read(h, spell);
    h.graphics(entity, s.graphic)?;
    if let Some(projectile) = &s.projectile {
        if target.is_some_and(|t| !h.kind(t).is_entity()) {
            return Err(Error::Cast);
        }
        h.send_projectile(
            projectile,
            entity,
            ProjectileTarget::Entity(target),
            false,
            58,
            10,
        )?;
    }
    h.animate(entity, s.animation)?;
    audio_at(h, entity, s.audio.id, 20)
}

/// `visualize(entity, target)`.
pub fn visualize<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    target: Option<&H::Node>,
) -> Result<()> {
    let class = h.spell(spell).class;
    match class {
        SpellClass::Ice
        | SpellClass::Blood
        | SpellClass::Shadow
        | SpellClass::Smoke
        | SpellClass::Miasmic
        | SpellClass::Teleblock => ancient_visualize(h, spell, entity, target),
        SpellClass::God => {
            combat_visualize(h, spell, entity, target)?;
            if is_npc(h, entity) {
                let forced = if h.npc_id(entity)? > 911 && h.npc_id(entity)? < 915 {
                    true
                } else {
                    h.npc_id(entity)? > 906 && h.npc_id(entity)? < 912
                };
                if forced {
                    let animation = h.attack_animation(entity)?;
                    h.force_animation(entity, animation)?;
                }
            }
            Ok(())
        }
        SpellClass::Default => {
            let (id, height) = h.spell(spell).default_projectile.unwrap_or((-1, 42));
            if id != -1 {
                if target.is_some_and(|t| !h.kind(t).is_entity()) {
                    return Err(Error::Cast);
                }
                let projectile = h.projectile_magic(entity, target, id, height, 36, 52, 15)?;
                h.spell(spell).projectile = Some(projectile);
            }
            combat_visualize(h, spell, entity, target)
        }
        _ => combat_visualize(h, spell, entity, target),
    }
}

/// `CombatSpell.visualizeImpact`.
fn combat_visualize_impact<H: Host>(
    h: &mut H,
    s: &Spell,
    target: &H::Node,
    state: StateView,
) -> Result<()> {
    if state.estimated_hit == -1 {
        audio_at(h, target, SPELL_FAIL_AUDIO, 20)?;
        h.graphics(target, Some(SPLASH_GRAPHIC))?;
        return Ok(());
    }
    h.graphics(target, s.end_graphic)?;
    audio_at(h, target, s.impact_audio, 20)
}

/// `visualizeImpact(entity, target, state)`, with the `IceSpells` and
/// `GodSpells` overrides. Only `GodSpells` changes state: the player's god
/// cast count.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: Option<&H::Node>,
    target: Option<&H::Node>,
    state: StateView,
) -> Result<()> {
    let s = read(h, spell);
    let target_node = target.ok_or(Error::Null);
    match s.class {
        SpellClass::Ice if state.frozen => {
            let target = target_node?;
            audio_at(h, target, s.impact_audio, 20)?;
            h.graphics(target, Some(super::graphics(1677, 96, 0)))
        }
        SpellClass::God => {
            if let Some(player) = entity.filter(|e| is_player(h, e)) {
                let index = god_index(&s)?;
                if god_cast(h, player, index)? < 100 {
                    let value = god_cast(h, player, index)?;
                    h.set_god_cast(player, index as usize, value.wrapping_add(1));
                    if god_cast(h, player, index)? >= 100 {
                        h.send_message(
                            player,
                            &format!(
                                "You can now cast {} outside the Arena.",
                                GOD_NAMES[index as usize]
                            ),
                        )?;
                    }
                }
                if state.estimated_hit == -1 {
                    let target = target_node?;
                    h.graphics(target, Some(SPLASH_GRAPHIC))?;
                    // All three projectile constants are null, so each
                    // comparison with the (null) projectile holds.
                    for fail in [1656, 1652, 1654] {
                        if s.projectile.is_none() {
                            audio_at(h, target, fail, 20)?;
                        }
                    }
                    return Ok(());
                }
            }
            let target = target_node?;
            h.graphics(target, s.end_graphic)?;
            audio_at(h, target, s.impact_audio, 0)
        }
        _ => combat_visualize_impact(h, &s, target_node?, state),
    }
}

/// One `BattleState(entity, victim)` per target: the pair it is built from.
pub type Targets<N> = Vec<(N, Option<N>)>;

/// `getTargets(entity, target)`: one state against the target, or for the
/// multi-target ancient spells in a multi-combat zone, one per multi-hit
/// target.
pub fn get_targets<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    target: Option<&H::Node>,
) -> Result<Targets<H::Node>> {
    let s = read(h, spell);
    let single = |target: Option<&H::Node>| vec![(entity.clone(), target.cloned())];
    let single_target = match s.class {
        SpellClass::Ice | SpellClass::Blood | SpellClass::Shadow | SpellClass::Smoke => {
            let id = s.animation.ok_or(Error::Null)?.id;
            id == 1978
        }
        SpellClass::Miasmic => {
            let id = s.animation.ok_or(Error::Null)?.id;
            id == 10513 || id == 10524
        }
        _ => return Ok(single(target)),
    };
    if single_target || !h.multi_zone(entity)? {
        return Ok(single(target));
    }
    let victim = target.ok_or(Error::Null)?;
    if !h.multi_zone(victim)? {
        return Ok(single(target));
    }
    let list = h.multi_targets(victim, entity)?;
    Ok(list
        .into_iter()
        .map(|e| (entity.clone(), Some(e)))
        .collect())
}

/// `getMaximumImpact(entity, victim, state)` of every class.
pub fn maximum_impact<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: Option<&H::Node>,
    victim: Option<&H::Node>,
) -> Result<i32> {
    let s = read(h, spell);
    let base = match s.class {
        SpellClass::Air | SpellClass::Smoke => 1,
        SpellClass::Water | SpellClass::Shadow => 2,
        SpellClass::Earth | SpellClass::Blood => 3,
        SpellClass::Fire | SpellClass::Ice => 4,
        SpellClass::SonicWave => 5,
        SpellClass::God
        | SpellClass::CrumbleUndead
        | SpellClass::IbanBlast
        | SpellClass::MagicDart => 0,
        SpellClass::Bind => return Ok(if s.kind == SpellType::Entangle { 5 } else { 3 }),
        SpellClass::Curse => return Ok(1),
        SpellClass::Teleblock => return Ok(0),
        SpellClass::Miasmic => {
            let id = s.animation.ok_or(Error::Null)?.id;
            let mut add = 9;
            if id == 10524 || id == 10516 {
                add = 6;
            }
            if s.animation.ok_or(Error::Null)?.id == 10513 {
                add = 4;
            }
            add
        }
        SpellClass::Default => {
            let entity = entity.ok_or(Error::Null)?;
            let level = h.level(entity, skill::MAGIC)?;
            let bonuses = h.bonuses(entity)?.ok_or(Error::Null)?;
            let bonus = *bonuses.get(13).ok_or(Error::Index)?;
            let sum = 14i32.wrapping_add(level).wrapping_add(bonus / 8);
            let value = sum as f64 + (level.wrapping_mul(bonus) as f64) * 0.016865;
            return Ok((value as i32) / 10 + 1);
        }
    };
    s.kind.impact_amount(h, entity, victim, base)
}

/// `CombatSpell.cast`.
fn combat_cast<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    target: Option<&H::Node>,
) -> Result<bool> {
    if !meets_requirements(h, spell, entity, true, false)? {
        return Ok(false);
    }
    if let Some(t) = target.filter(|t| h.kind(t).is_entity()) {
        h.face(entity, Some(t))?;
    }
    h.set_spell(entity, spell)?;
    if h.is_attacking(entity)? {
        h.set_victim(entity, target)?;
        h.update_style(entity)?;
        h.pulse_start(entity)?;
        return Ok(true);
    }
    h.pulse_attack(entity, target)?;
    Ok(true)
}

/// `ZogreBehavior.zogreIds` and `SkogreBehavior.skogreIds`.
const ZOGRES: [i32; 11] = [
    2044, 2045, 2046, 2047, 2048, 2049, 2051, 2052, 2053, 2054, 2055,
];
const SKOGRES: [i32; 3] = [2050, 2056, 2057];

/// `cast(entity, target)`, with the overrides of `CrumbleUndead`,
/// `IbanBlast`, `MagicDart`, `TeleblockSpell` and `MiasmicSpells`.
pub fn cast<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    target: Option<&H::Node>,
) -> Result<bool> {
    let class = h.spell(spell).class;
    match class {
        SpellClass::CrumbleUndead => {
            let npc = target.filter(|t| is_npc(h, t));
            // Each anyMatch reads the id once per element until a match; a
            // null NPC fails at the first element.
            let mut matched = false;
            for ids in [&ZOGRES[..], &SKOGRES[..]] {
                for id in ids {
                    if h.npc_id(npc.ok_or(Error::Null)?)? == *id {
                        matched = true;
                        break;
                    }
                }
                if matched {
                    break;
                }
            }
            if matched || h.npc_id(npc.ok_or(Error::Null)?)? == 2060 {
                return combat_cast(h, spell, entity, target);
            }
            let npc = npc.ok_or(Error::Null)?;
            let undead = match h.npc_task(npc)? {
                None => false,
                Some(_) => h.npc_task(npc)?.ok_or(Error::Null)?,
            };
            if !undead {
                if !is_player(h, entity) {
                    return Err(Error::Cast);
                }
                h.packet_message(entity, "This spell only affects the undead.")?;
                return Ok(false);
            }
            combat_cast(h, spell, entity, target)
        }
        SpellClass::IbanBlast => {
            if !is_player(h, entity) {
                return Err(Error::Cast);
            }
            if h.equipment_new(entity, slot::WEAPON)? != 1409 {
                h.packet_message(entity, "You need to wear Iban's staff to cast this spell.")?;
                return Ok(false);
            }
            combat_cast(h, spell, entity, target)
        }
        SpellClass::MagicDart => {
            if h.level(entity, skill::SLAYER)? < 55 {
                if !is_player(h, entity) {
                    return Err(Error::Cast);
                }
                h.packet_message(entity, "You need a Slayer level of 55 to cast this spell.")?;
                return Ok(false);
            }
            if !is_player(h, entity) {
                return Err(Error::Cast);
            }
            if h.equipment_new(entity, slot::WEAPON)? != 4170 {
                h.packet_message(
                    entity,
                    "You need to wear a Slayer's staff to cast this spell.",
                )?;
                return Ok(false);
            }
            combat_cast(h, spell, entity, target)
        }
        SpellClass::Teleblock => {
            let target_player = target.filter(|t| is_player(h, t));
            let Some(victim) = target_player else {
                if !is_player(h, entity) {
                    return Err(Error::Cast);
                }
                h.send_message(entity, "You can only cast this spell on another player.")?;
                return Ok(false);
            };
            if !h.in_zone(entity, "Wilderness")? || !h.in_zone(victim, "Wilderness")? {
                if !is_player(h, entity) {
                    return Err(Error::Cast);
                }
                h.send_message(
                    entity,
                    "You and your opponent must both be in the wilderness for you to use this spell.",
                )?;
                return Ok(false);
            }
            if h.timer_active(victim, "teleblock")? {
                if !is_player(h, entity) {
                    return Err(Error::Cast);
                }
                h.send_message(entity, "That player is already affected by this spell.")?;
                return Ok(false);
            }
            if !meets_requirements(h, spell, entity, true, false)? {
                return Ok(false);
            }
            combat_cast(h, spell, entity, target)
        }
        SpellClass::Miasmic => {
            if !is_player(h, entity) {
                return Err(Error::Cast);
            }
            let mut valid = false;
            for id in [13867, 13869, 13841, 13843] {
                if h.equipment_new(entity, slot::WEAPON)? == id {
                    valid = true;
                    break;
                }
            }
            if !valid && !h.dev_mode()? {
                h.packet_message(
                    entity,
                    "You need to be wielding Zuriel's staff in order to cast this spell.",
                )?;
                return Ok(false);
            }
            if !meets_requirements(h, spell, entity, true, false)? {
                return Ok(false);
            }
            combat_cast(h, spell, entity, target)
        }
        _ => combat_cast(h, spell, entity, target),
    }
}

/// `MagicSpell.castSpell(p, book, spellId, target)`: the cast entry point of
/// the spell packets. Every spell here is a combat spell, so the non-combat
/// experience and temporary facing branches are not taken.
pub fn cast_spell<H: Host>(
    h: &mut H,
    player: &H::Node,
    book: SpellBook,
    spell_id: i32,
    target: Option<&H::Node>,
) -> Result<bool> {
    if attribute_int(h, player, "magic-delay", 0)? > h.ticks()? {
        return Ok(false);
    }
    let Some(spell) = h.registered(book, spell_id) else {
        return Ok(false);
    };
    if h.spell(&spell).book != book || *h.spell_book(player) != book.interface_id() {
        return Ok(false);
    }
    let target_node = target.ok_or(Error::Null)?;
    if h.location(target_node)?.is_some() && !h.same_node(target_node, player) {
        let there = h.location(target_node)?.ok_or(Error::Null)?;
        let here = h.location(player)?.ok_or(Error::Null)?;
        if !within_distance(there, here, 15) {
            return Ok(false);
        }
        let face = h.location(target_node)?;
        h.face_location(player, face)?;
    }
    if cast(h, &spell, player, target)? {
        if attribute_int(h, player, "magic-delay", 0)? <= h.ticks()? {
            let value = h.ticks()?.wrapping_add(delay());
            h.set_attribute(player, "magic-delay", Value::Int(value))?;
        }
        h.dispatch_cast(player, book, spell_id, target)?;
        return Ok(true);
    }
    Ok(false)
}

/// `Location.withinDistance(other, dist)`.
fn within_distance(this: [i32; 3], other: [i32; 3], dist: i32) -> bool {
    if other[2] != this[2] {
        return false;
    }
    crate::world::combat_swing::distance(this, other) <= dist as f64
}

/// `new DefaultCombatSpell(npc)`: a bolt-type spell from the NPC's magic
/// animation and combat graphics. Reads the magic animation, then the combat
/// graphics array for slots 0 and 2, then up to three times for slot 1.
pub fn default_combat_spell<H: Host>(h: &mut H, npc: &H::Node) -> Result<Spell> {
    let animation = h.magic_animation(npc)?;
    let slot = |h: &mut H, index: usize| -> Result<Option<Graphics>> {
        let array = h.combat_graphics(npc)?.ok_or(Error::Null)?;
        array.get(index).copied().ok_or(Error::Index)
    };
    let graphic = slot(h, 0)?;
    let end_graphic = slot(h, 2)?;
    let default_projectile = if slot(h, 1)?.is_some() {
        let id = slot(h, 1)?.ok_or(Error::Null)?.id;
        let height = slot(h, 1)?.ok_or(Error::Null)?.height;
        (id, height)
    } else {
        (-1, 42)
    };
    Ok(Spell {
        class: SpellClass::Default,
        book: SpellBook::Modern,
        level: 0,
        experience: 0.0,
        animation,
        graphic,
        audio: Audio {
            id: -1,
            delay: 1,
            loops: 0,
            radius: DEFAULT_AUDIO_RADIUS,
        },
        runes: Some(Vec::new()),
        spell_id: 0,
        kind: SpellType::Bolt,
        projectile: None::<Projectile>,
        end_graphic,
        impact_audio: -1,
        default_projectile: Some(default_projectile),
    })
}
