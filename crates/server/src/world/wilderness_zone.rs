//! WildernessZone from `core.game.world.map.zone.impl`: its borders and
//! wilderness level, enter/leave/locationUpdate (overlay 381, the level line,
//! the player attack option, skull wilderness/level, familiars, NPC
//! aggression), the teleport rule (levels 20 and 30), the PvP and revenant
//! combat-level ranges, the revenant attack option, the revenant and Chaos
//! Elemental loot rolled at death, and the static isInZone, getWilderness,
//! show, leave and checkTeleport. ZoneBuilder.startup is here too: it is the
//! one place both remaining zones are configured.
//!
//! A library: nothing installs it. SkullManager, the familiar manager,
//! appearance, interaction options, world settings, NPC and item names,
//! combat levels, the combat pulse, ground items, news and the username are
//! required services called in the original order; none has a default and
//! no lock is held across a service or zone callback. The RANDOM draws go
//! through `zones::Host::random_next_int`.
use super::{
    dark_zone::{self, Component, Overlays},
    doors::Location,
    player_walk::NodeHandle,
    zone_impls::{self, Rights},
    zones::{self, Behavior, Borders, EntityKind, Error, Restriction, Result, Zone},
};
use rs09_game_core::java_lower::lowercase_root;
use std::any::Any;

/// The PvP gear rolled by rollWildernessExclusiveLoot, in its order.
pub const PVP_GEAR: [i32; 17] = [
    13887, 13893, 13899, 13905, 13870, 13873, 13876, 13879, 13883, 13884, 13890, 13896, 13902,
    13858, 13861, 13864, 13867,
];
pub const WILDERNESS_PROT_ATTR: &str = "/save:wilderness-protection-active";
pub const WILDERNESS_HIGHER_NEXTFEE: &str = "/save:wilderness-higher-next-fee";
/// The wilderness overlay and its level line.
pub const OVERLAY: i32 = 381;
pub const LEVEL_CHILD: i32 = 1;
/// `NPCs.CHAOS_ELEMENTAL_3200`.
pub const CHAOS_ELEMENTAL: i32 = 3200;
/// `Items.MORRIGANS_JAVELIN_13879` and `Items.MORRIGANS_THROWING_AXE_13883`.
pub const MORRIGANS_JAVELIN: i32 = 13879;
pub const MORRIGANS_THROWING_AXE: i32 = 13883;
/// SkullManager.DEEP_WILD_DROP_RISK_THRESHOLD.
pub const DEEP_WILD_DROP_RISK_THRESHOLD: i64 = 100_000;
pub const LEVEL_DIFFERENCE: &str =
    "The level difference between you and your opponent is too great.";
pub const TOO_DEEP: &str = "You can't teleport this deep in the wilderness!";
/// BrawlingGloves by indicator: (indicator, item id).
pub const BRAWLING_GLOVES: [(i32, i32); 13] = [
    (1, 13845),
    (2, 13846),
    (3, 13847),
    (4, 13848),
    (5, 13849),
    (6, 13850),
    (7, 13851),
    (8, 13852),
    (9, 13853),
    (10, 13854),
    (11, 13855),
    (12, 13856),
    (13, 13857),
];
/// `BrawlingGloves.forIndicator(indicator).getId()`.
pub fn brawling_gloves(indicator: i32) -> Option<i32> {
    BRAWLING_GLOVES
        .iter()
        .find(|(key, _)| *key == indicator)
        .map(|(_, id)| *id)
}

/// WildernessZone's services.
pub trait Host: zone_impls::Host + Overlays {
    /// `skullManager.isWildernessDisabled()`.
    fn skull_wilderness_disabled(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `skullManager.setLevel(level)`.
    fn set_skull_level(&mut self, player: &NodeHandle, level: i32) -> Result<()>;
    /// `skullManager.setWilderness(wilderness)`.
    fn set_skull_wilderness(&mut self, player: &NodeHandle, wilderness: bool) -> Result<()>;
    /// `skullManager.isDeepWilderness()`.
    fn skull_deep_wilderness(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `player.getAttribute("deepwild-value-risk", 0L)`.
    fn deepwild_value_risk(&mut self, player: &NodeHandle) -> Result<i64>;
    /// `getPacketDispatch().sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &NodeHandle,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `GameWorld.getSettings().getWild_pvp_enabled()`.
    fn wild_pvp_enabled(&mut self) -> Result<bool>;
    /// `getInteraction().set(Option._P_ATTACK)`.
    fn set_attack_option(&mut self, player: &NodeHandle) -> Result<()>;
    /// `getInteraction().remove(Option._P_ATTACK)`.
    fn remove_attack_option(&mut self, player: &NodeHandle) -> Result<()>;
    fn has_familiar(&mut self, player: &NodeHandle) -> Result<bool>;
    fn has_pet(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `getFamiliarManager().getFamiliar()`.
    fn familiar(&mut self, player: &NodeHandle) -> Result<Option<NodeHandle>>;
    /// `Familiar.transform()`.
    fn familiar_transform(&mut self, familiar: &NodeHandle) -> Result<()>;
    fn familiar_is_combat(&mut self, familiar: &NodeHandle) -> Result<bool>;
    /// `NPC.reTransform()`.
    fn npc_retransform(&mut self, npc: &NodeHandle) -> Result<()>;
    /// `getAppearance().sync()`.
    fn appearance_sync(&mut self, player: &NodeHandle) -> Result<()>;
    fn npc_aggressive(&mut self, npc: &NodeHandle) -> Result<bool>;
    /// `setAggressiveHandler(new AggressiveHandler(npc, AggressiveBehavior.WILDERNESS))`.
    fn set_wilderness_aggression(&mut self, npc: &NodeHandle) -> Result<()>;
    /// `npc.getName()`; None is a null name.
    fn npc_name(&mut self, npc: &NodeHandle) -> Result<Option<String>>;
    /// `npc.getDefinition().getCombatLevel()`.
    fn npc_combat_level(&mut self, npc: &NodeHandle) -> Result<i32>;
    /// `getProperties().getCurrentCombatLevel()`.
    fn current_combat_level(&mut self, player: &NodeHandle) -> Result<i32>;
    /// `getProperties().getCombatPulse().attack(target)`.
    fn combat_pulse_attack(&mut self, player: &NodeHandle, target: &Self::Target) -> Result<()>;
    /// `Item.getName()` of an item with this id; None is a null name.
    fn item_name(&mut self, id: i32) -> Result<Option<String>>;
    /// `npc.getDropLocation()`.
    fn drop_location(&mut self, npc: &NodeHandle) -> Result<Option<Location>>;
    /// `GroundItemManager.create(new Item(id, amount), location, player)`.
    fn ground_item_create(
        &mut self,
        id: i32,
        amount: i32,
        location: Option<&Location>,
        player: &NodeHandle,
    ) -> Result<()>;
    /// `getUsername()`; None is null.
    fn username(&mut self, player: &NodeHandle) -> Result<Option<String>>;
    /// `Repository.sendNews(text)`.
    fn send_news(&mut self, text: &str) -> Result<()>;
}

/// `getWilderness(y)`: levels count in eights from 3520, or from 9920 above
/// 6400.
pub fn wilderness_level(y: i32) -> i32 {
    if 6400 < y {
        y.wrapping_sub(9920).wrapping_div(8).wrapping_add(1)
    } else {
        y.wrapping_sub(3520).wrapping_div(8).wrapping_add(1)
    }
}
/// `WildernessZone.getWilderness(entity)`: one location read.
pub fn get_wilderness<H: zones::Host>(host: &mut H, entity: &NodeHandle) -> Result<i32> {
    let location = host.location(entity)?.ok_or(Error::Null)?;
    Ok(wilderness_level(location.coordinates()[1]))
}
/// `getNewDropRate(combatLevel)`: `(int) (A + B * Math.log(x))`.
pub fn new_drop_rate(combat_level: i32) -> i32 {
    const A: f64 = 44044.5491;
    const B: f64 = -7360.19548;
    (A + (B * f64::from(combat_level).ln())) as i32
}
/// The glove rate without the deep-wilderness bonus.
pub fn normal_glove_rate(pvp_gear_rate: i32) -> i32 {
    ((1.0 / (1.0 - (1.0 - (1.0 / f64::from(pvp_gear_rate))).powf(16.0))) * 5.0 / 6.0) as i32
}
/// RandomFunction.random(max): no draw for a nonpositive bound.
pub fn random<H: zones::Host>(host: &mut H, max: i32) -> Result<i32> {
    if max <= 0 {
        return Ok(0);
    }
    host.random_next_int(max)
}
/// RandomFunction.random(a, b).
pub fn random_between<H: zones::Host>(host: &mut H, a: i32, b: i32) -> Result<i32> {
    let n = b.wrapping_sub(a).wrapping_abs();
    Ok(a.min(b)
        .wrapping_add(if n == 0 { 0 } else { random(host, n)? }))
}
/// RandomFunction.roll(chance).
pub fn roll<H: zones::Host>(host: &mut H, chance: i32) -> Result<bool> {
    if chance <= 1 {
        return Ok(true);
    }
    Ok(random(host, chance)? == 1)
}

fn contains(name: Option<String>, part: &str) -> Result<bool> {
    Ok(name.ok_or(Error::Null)?.contains(part))
}
fn java_string(value: Option<String>) -> String {
    value.unwrap_or_else(|| "null".into())
}
fn is_npc<H: zones::Host>(host: &mut H, entity: &NodeHandle) -> Result<bool> {
    Ok(host.kind(entity)?.is_npc())
}
fn require_player<H: zones::Host>(host: &mut H, entity: &NodeHandle) -> Result<()> {
    if host.kind(entity)? != EntityKind::Player {
        return Err(Error::ClassCast);
    }
    Ok(())
}

/// `WildernessZone.show(p)`: overlay 381, the level, its line, the attack
/// option when PvP is enabled, then the wilderness flag.
pub fn show<H: Host>(host: &mut H, player: &NodeHandle) -> Result<()> {
    if host.skull_wilderness_disabled(player)? {
        return Ok(());
    }
    host.open_overlay(player, Component::Id(OVERLAY))?;
    let level = get_wilderness(host, player)?;
    host.set_skull_level(player, level)?;
    let text = format!("Level: {}", host.skull_level(player)?);
    host.send_string(player, &text, OVERLAY, LEVEL_CHILD)?;
    if host.wild_pvp_enabled()? {
        host.set_attack_option(player)?;
    }
    host.set_skull_wilderness(player, true)
}
/// The final `leave(Player)`: a fresh component 381 is closed (the manager's
/// overlay field is left as it was), then the option, flag and level go.
pub fn leave_player<H: Host>(host: &mut H, player: &NodeHandle) -> Result<()> {
    let overlay = Component::Id(OVERLAY);
    if overlay.id() == OVERLAY {
        host.close_component(player, overlay)?;
    }
    host.remove_attack_option(player)?;
    host.set_skull_wilderness(player, false)?;
    host.set_skull_level(player, 0)
}
/// `checkTeleport(p, level)`.
pub fn check_teleport<H: Host>(host: &mut H, player: &NodeHandle, level: i32) -> Result<bool> {
    if host.skull_level(player)? > level && !host.skull_wilderness_disabled(player)? {
        zones::message(host, player, TOO_DEEP)?;
        return Ok(false);
    }
    Ok(true)
}

/// WildernessZone with its borders. The world keeps the INSTANCE (made by
/// `wilderness_zone`) for isInZone; `with_borders` is the public constructor.
pub struct WildernessZone {
    borders: Vec<Borders>,
}
impl WildernessZone {
    /// The INSTANCE borders, in order.
    pub fn instance_borders() -> Vec<Borders> {
        vec![
            Borders::new(2944, 3525, 3400, 3975),
            Borders::new(3070, 9924, 3135, 10002),
            Borders::for_region(12192),
            Borders::for_region(12193),
            Borders::for_region(11937),
        ]
    }
    /// `getBorders()`: the same border objects.
    pub fn borders(&self) -> &[Borders] {
        &self.borders
    }
    /// `isInZone(Entity)` over this instance's borders: an unused location
    /// read, then one read per border tested.
    pub fn is_in_zone<H: zones::Host>(&self, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        host.location(entity)?;
        for borders in &self.borders {
            if borders.inside_node(host, entity)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// `isInZone(Location)`.
    pub fn is_in_zone_location(&self, location: Option<&Location>) -> Result<bool> {
        for borders in &self.borders {
            if borders.inside_location(location)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn roll_exclusive_loot<H: Host>(
        &self,
        host: &mut H,
        npc: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<()> {
        let Some(killer) = killer else {
            return Ok(());
        };
        if host.kind(killer)? != EntityKind::Player || !is_npc(host, npc)? {
            return Ok(());
        }
        if !(host.id(npc)? == CHAOS_ELEMENTAL || contains(host.npc_name(npc)?, "Revenant")?) {
            return Ok(());
        }
        let level = host.npc_combat_level(npc)?;
        let mut pvp_gear_rate = new_drop_rate(level);
        let higher_rate = host.skull_deep_wilderness(killer)?
            && host.deepwild_value_risk(killer)? > DEEP_WILD_DROP_RISK_THRESHOLD;
        if higher_rate {
            pvp_gear_rate = pvp_gear_rate.wrapping_div(2);
        }
        let chaos_glove_rate = 50;
        let normal = if higher_rate {
            100
        } else {
            normal_glove_rate(pvp_gear_rate)
        };
        let glove_rate = if host.id(npc)? == CHAOS_ELEMENTAL {
            chaos_glove_rate
        } else {
            normal
        };
        if roll(host, glove_rate)? {
            let glove = random_between(host, 1, 14)? as i8;
            let id = brawling_gloves(i32::from(glove)).ok_or(Error::Null)?;
            let drop = host.drop_location(npc)?;
            host.ground_item_create(id, 1, drop.as_ref(), killer)?;
            let npc_string = self.npc_string(host, npc)?;
            let username = java_string(host.username(killer)?);
            let name = lowercase_root(&host.item_name(id)?.ok_or(Error::Null)?);
            host.send_news(&format!(
                "{username} has received {name} from {npc_string}!"
            ))?;
        }
        for j in PVP_GEAR {
            if roll(host, pvp_gear_rate)? {
                let amount = if j == MORRIGANS_JAVELIN || j == MORRIGANS_THROWING_AXE {
                    random_between(host, 15, 50)?
                } else {
                    1
                };
                let drop = host.drop_location(npc)?;
                host.ground_item_create(j, amount, drop.as_ref(), killer)?;
                let npc_string = self.npc_string(host, npc)?;
                let username = java_string(host.username(killer)?);
                let name = java_string(host.item_name(j)?);
                host.send_news(&format!(
                    "{username} has received a {name} from {npc_string}!"
                ))?;
            }
        }
        Ok(())
    }
    fn npc_string<H: Host>(&self, host: &mut H, npc: &NodeHandle) -> Result<String> {
        if host.id(npc)? == CHAOS_ELEMENTAL {
            return Ok("the Chaos Elemental".into());
        }
        let name = host.npc_name(npc)?.ok_or(Error::Null)?;
        Ok(format!("a {}", lowercase_root(&name)))
    }
    /// One combat-level range test: `combat - level > other || combat +
    /// level < other`, wrapping.
    fn out_of_range<H: Host>(
        host: &mut H,
        player: &NodeHandle,
        level: i32,
        combat: i32,
        other: i32,
        message: bool,
    ) -> Result<bool> {
        if combat.wrapping_sub(level) > other || combat.wrapping_add(level) < other {
            if message {
                host.packet_message(player, LEVEL_DIFFERENCE)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
}
impl<H: Host> Behavior<H> for WildernessZone {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> Result<()> {
        for borders in &self.borders {
            zone.register(host, borders)?;
        }
        Ok(())
    }
    /// Loot for a dying NPC; the default death handling always continues.
    fn death(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        if is_npc(host, entity)? {
            self.roll_exclusive_loot(host, entity, killer)?;
        }
        Ok(false)
    }
    /// A revenant is attacked straight through the combat pulse.
    fn interact(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        target: Option<&H::Target>,
        _option: Option<&H::Opaque>,
    ) -> Result<bool> {
        if let Some(target) = target {
            if let Some(npc) = host.target_entity(target)? {
                if is_npc(host, &npc)? && contains(host.npc_name(&npc)?, "Revenant")? {
                    require_player(host, entity)?;
                    host.combat_pulse_attack(entity, target)?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn enter(&self, _: &Zone<H>, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        let kind = host.kind(entity)?;
        if kind == EntityKind::Player {
            if !host.is_artificial(entity)? {
                show(host, entity)?;
            } else {
                host.set_skull_wilderness(entity, true)?;
                let level = get_wilderness(host, entity)?;
                host.set_skull_level(entity, level)?;
            }
            if host.has_familiar(entity)? && !host.has_pet(entity)? {
                let familiar = host.familiar(entity)?.ok_or(Error::Null)?;
                host.familiar_transform(&familiar)?;
            }
            host.appearance_sync(entity)?;
        } else if kind.is_npc() && host.has_attack_option(entity)? && host.npc_aggressive(entity)? {
            host.set_wilderness_aggression(entity)?;
        }
        Ok(true)
    }
    fn leave(&self, _: &Zone<H>, host: &mut H, entity: &NodeHandle, logout: bool) -> Result<bool> {
        if !logout && host.kind(entity)? == EntityKind::Player {
            leave_player(host, entity)?;
            if host.has_familiar(entity)? && !host.has_pet(entity)? {
                let familiar = host.familiar(entity)?.ok_or(Error::Null)?;
                if host.familiar_is_combat(&familiar)? {
                    host.npc_retransform(&familiar)?;
                }
            }
            host.appearance_sync(entity)?;
        }
        Ok(true)
    }
    /// Administrators always; otherwise not above level 20, or 30 with an
    /// item whose name holds "glory" or "slaying".
    fn teleport(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        _kind: i32,
        node: Option<&H::Target>,
    ) -> Result<bool> {
        if host.kind(entity)? == EntityKind::Player {
            if host.rights(entity)? == Rights::Administrator {
                return Ok(true);
            }
            let mut level = 20;
            if let Some(node) = node {
                if host.is_item(node)? {
                    let id = host.target_item_id(Some(node))?;
                    if contains(host.item_name(id)?, "glory")?
                        || contains(host.item_name(id)?, "slaying")?
                    {
                        level = 30;
                    }
                }
            }
            if !check_teleport(host, entity, level)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Player against player: the lower of both skull levels against the
    /// combat levels. Player against a revenant: the lower of the player's
    /// skull level and the NPC's wilderness level.
    fn continue_attack(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        target: Option<&H::Target>,
        _style: Option<&H::Opaque>,
        message: bool,
    ) -> Result<bool> {
        let other = match target {
            Some(target) => host.target_entity(target)?,
            None => None,
        };
        let player = host.kind(entity)? == EntityKind::Player;
        if let Some(other) = &other {
            if player && host.kind(other)? == EntityKind::Player {
                let mut level = host.skull_level(entity)?;
                if host.skull_level(other)? < level {
                    level = host.skull_level(other)?;
                }
                let combat = host.current_combat_level(entity)?;
                let target_combat = host.current_combat_level(other)?;
                if Self::out_of_range(host, entity, level, combat, target_combat, message)? {
                    return Ok(false);
                }
            }
            if player && is_npc(host, other)? && contains(host.npc_name(other)?, "Revenant")? {
                let mut level = host.skull_level(entity)?;
                let npc_level = get_wilderness(host, other)?;
                if npc_level < level {
                    level = npc_level;
                }
                let combat = host.current_combat_level(entity)?;
                let npc_combat = host.npc_combat_level(other)?;
                if Self::out_of_range(host, entity, level, combat, npc_combat, message)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    fn location_update(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        _last: Option<&Location>,
    ) -> Result<()> {
        if host.kind(entity)? == EntityKind::Player && !host.is_artificial(entity)? {
            let level = get_wilderness(host, entity)?;
            host.set_skull_level(entity, level)?;
        }
        Ok(())
    }
}
/// `new WildernessZone(borders...)`.
pub fn with_borders<H: Host>(borders: Vec<Borders>) -> Zone<H> {
    Zone::new(
        Some("Wilderness"),
        true,
        &[Restriction::RandomEvents],
        WildernessZone { borders },
    )
}
/// WildernessZone.INSTANCE. The owning world makes it once and keeps it:
/// isInZone reads its borders.
pub fn wilderness_zone<H: Host>() -> Zone<H> {
    with_borders(WildernessZone::instance_borders())
}

/// ZoneBuilder.startup: configure (uid, then the zone's configure) of the
/// wilderness INSTANCE, the multiway INSTANCE, a new ModeratorZone, a new
/// DarkZone, a new KaramjaZone and a new BankZone, in that order.
pub fn startup<H: Host + dark_zone::Host>(
    host: &mut H,
    wilderness: &Zone<H>,
    multiway: &Zone<H>,
) -> Result<()> {
    wilderness.configure(host)?;
    multiway.configure(host)?;
    zone_impls::moderator_zone::<H>().configure(host)?;
    dark_zone::dark_zone::<H>().configure(host)?;
    zone_impls::karamja_zone::<H>().configure(host)?;
    zone_impls::bank_zone::<H>().configure(host)
}
