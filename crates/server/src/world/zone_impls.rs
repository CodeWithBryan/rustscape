//! Four of the six zones of `core.game.world.map.zone.impl`: BankZone,
//! KaramjaZone, ModeratorZone and MultiwayCombatZone. WildernessZone (with
//! ZoneBuilder.startup) is in `wilderness_zone.rs`, DarkZone in `dark_zone.rs`.
//!
//! Inventory, interface, packet, teleport, rights, combat-property and local
//! entity requests are required `Host` services in the original order; none
//! is answered here and none has a default.
use super::{
    doors::Location,
    player_walk::NodeHandle,
    zones::{self, Behavior, Borders, EntityKind, Error, Result, Zone},
};
use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rights {
    RegularPlayer,
    PlayerModerator,
    Administrator,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportType {
    Normal,
}

pub trait Host: zones::Host {
    /// `player.getInventory().getAmount(new Item(id))`.
    fn inventory_amount(&mut self, player: &NodeHandle, item_id: i32) -> Result<i32>;
    /// `player.getInventory().remove(new Item(id, amount))`.
    fn inventory_remove(&mut self, player: &NodeHandle, item_id: i32, amount: i32) -> Result<bool>;
    fn interface_resizable(&mut self, player: &NodeHandle) -> Result<bool>;
    fn window_pane_id(&mut self, player: &NodeHandle) -> Result<i32>;
    /// `PacketRepository.send(Interface.class, new InterfaceContext(...))`.
    fn send_interface(
        &mut self,
        player: &NodeHandle,
        window: i32,
        component: i32,
        interface: i32,
        walkable: bool,
    ) -> Result<()>;
    /// `PacketRepository.send(CloseInterface.class, new InterfaceContext(...))`.
    fn send_close_interface(
        &mut self,
        player: &NodeHandle,
        window: i32,
        component: i32,
        interface: i32,
        walkable: bool,
    ) -> Result<()>;
    /// `getPacketDispatch().sendInterfaceConfig(interface, child, hide)`.
    fn interface_config(
        &mut self,
        player: &NodeHandle,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()>;
    fn set_multi_zone(&mut self, entity: &NodeHandle, multi: bool) -> Result<()>;
    fn npc_walkable(&mut self, entity: &NodeHandle) -> Result<bool>;
    /// RegionManager.getLocalNPCs(location, distance).
    fn local_npcs(&mut self, location: Option<&Location>, distance: i32)
    -> Result<Vec<NodeHandle>>;
    /// RegionManager.getLocalPlayers(location, distance).
    fn local_players(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<NodeHandle>>;
    fn invisible(&mut self, npc: &NodeHandle) -> Result<bool>;
    /// `npc.getDefinition().hasAttackOption()`.
    fn has_attack_option(&mut self, npc: &NodeHandle) -> Result<bool>;
    fn should_prevent_stacking(&mut self, npc: &NodeHandle, mover: &NodeHandle) -> Result<bool>;
    fn size(&mut self, entity: &NodeHandle) -> Result<i32>;
    /// `player.getDetails().getRights()`.
    fn rights(&mut self, player: &NodeHandle) -> Result<Rights>;
    /// ServerConstants.HOME_LOCATION, which starts as null.
    fn home_location(&mut self) -> Result<Option<Location>>;
    /// `player.getTeleporter().send(location, type)`.
    fn teleporter_send(
        &mut self,
        player: &NodeHandle,
        location: Option<&Location>,
        kind: TeleportType,
    ) -> Result<bool>;
    /// `player.getInterfaceManager().restoreTabs()`.
    fn restore_tabs(&mut self, player: &NodeHandle) -> Result<()>;
}

/// MapDistance.RENDERING.
pub const RENDERING_DISTANCE: i32 = 15;

/// BankZone: two Varrock rectangles and no behaviour of its own.
pub struct BankZone {
    pub varrock_west: Borders,
    pub varrock_east: Borders,
}
impl<H: Host> Behavior<H> for BankZone {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> Result<()> {
        zone.register(host, &self.varrock_west)?;
        zone.register(host, &self.varrock_east)
    }
}
pub fn bank_zone<H: Host>() -> Zone<H> {
    Zone::new(
        Some("bank"),
        true,
        &[],
        BankZone {
            varrock_west: Borders::new(3179, 3432, 3194, 3446),
            varrock_east: Borders::new(3250, 3416, 3257, 3423),
        },
    )
}

/// KaramjaZone.REGIONS, with its repeated 11057.
pub const KARAMJA_REGIONS: [i32; 24] = [
    11309, 11054, 11566, 11565, 11567, 11568, 11053, 11821, 11055, 11057, 11569, 11822, 11823,
    11825, 11310, 11311, 11312, 11313, 11314, 11056, 11057, 11058, 10802, 10801,
];
pub const KARAMJAN_RUM: i32 = 431;
pub struct KaramjaZone;
impl<H: Host> Behavior<H> for KaramjaZone {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> Result<()> {
        for id in KARAMJA_REGIONS {
            zone.register_region(host, id)?;
        }
        Ok(())
    }
    fn teleport(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        _kind: i32,
        _node: Option<&H::Target>,
    ) -> Result<bool> {
        if host.kind(entity)? == EntityKind::Player {
            let amount = host.inventory_amount(entity, KARAMJAN_RUM)?;
            if amount != 0 {
                host.inventory_remove(entity, KARAMJAN_RUM, amount)?;
                host.packet_message(
                    entity,
                    "During the trip you lose your rum to a sailor in a game of dice. Better luck next time!",
                )?;
            }
        }
        Ok(true)
    }
}
pub fn karamja_zone<H: Host>() -> Zone<H> {
    Zone::new(Some("karamja"), true, &[], KaramjaZone)
}

/// ModeratorZone. `open` is a static of the class; here it is the state of
/// the zone object, shared by its clones.
pub struct ModeratorZone {
    open: Arc<AtomicBool>,
}
pub const MODERATOR_CENTER: [i32; 3] = [2846, 5213, 0];
impl ModeratorZone {
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }
    pub fn toggle_message(&self) -> String {
        format!(
            "The moderator room is currently {} to player moderators.",
            if self.is_open() {
                "available"
            } else {
                "not available"
            }
        )
    }
    /// `home(player)`.
    pub fn home<H: Host>(host: &mut H, player: &NodeHandle) -> Result<()> {
        let home = host.home_location()?;
        host.teleporter_send(player, home.as_ref(), TeleportType::Normal)?;
        Ok(())
    }
    /// The static `teleport(player)`: to the room's centre.
    pub fn teleport_to<H: Host>(host: &mut H, player: &NodeHandle) -> Result<()> {
        let [x, y, z] = MODERATOR_CENTER;
        host.teleporter_send(player, Some(&Location::new(x, y, z)), TeleportType::Normal)?;
        Ok(())
    }
    /// `toggle(player, on)`: closing sends every non-administrator within
    /// rendering distance of the centre home.
    pub fn toggle<H: Host>(&self, host: &mut H, player: &NodeHandle, on: bool) -> Result<()> {
        self.open.store(on, Ordering::Relaxed);
        host.packet_message(player, &self.toggle_message())?;
        if !self.is_open() {
            let [x, y, z] = MODERATOR_CENTER;
            for p in host.local_players(Some(&Location::new(x, y, z)), RENDERING_DISTANCE)? {
                if host.rights(&p)? == Rights::Administrator {
                    continue;
                }
                Self::home(host, &p)?;
            }
        }
        Ok(())
    }
}
impl<H: Host> Behavior<H> for ModeratorZone {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> Result<()> {
        zone.register(host, &Borders::new(2840, 5204, 2853, 5224))
    }
    fn enter(&self, _: &Zone<H>, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        if host.kind(entity)? != EntityKind::Player {
            return Ok(true);
        }
        if !self.is_open() && host.rights(entity)? != Rights::Administrator {
            Self::home(host, entity)?;
            return Ok(false);
        }
        if host.rights(entity)? != Rights::PlayerModerator {
            host.packet_message(entity, &self.toggle_message())?;
        }
        Ok(true)
    }
    fn leave(&self, _: &Zone<H>, host: &mut H, entity: &NodeHandle, _: bool) -> Result<bool> {
        if host.kind(entity)? != EntityKind::Player {
            return Ok(true);
        }
        host.restore_tabs(entity)?;
        Ok(true)
    }
}
pub fn moderator_zone<H: Host>() -> Zone<H> {
    Zone::new(
        Some("Moderator Zone"),
        true,
        &[],
        ModeratorZone {
            open: Arc::new(AtomicBool::new(true)),
        },
    )
}

/// One line of MultiwayCombatZone.configure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Borders([i32; 4]),
    Plane([i32; 5]),
    Region(i32),
    RegionBorders(i32, [i32; 4]),
}
/// MultiwayCombatZone.configure, in source order.
pub const MULTIWAY_AREAS: [Area; 55] = [
    Area::Borders([3210, 9333, 3339, 9424]),
    Area::Borders([2607, 3296, 2644, 3332]),
    Area::Borders([2949, 3370, 3001, 3392]),
    Area::Borders([3250, 9800, 3342, 9870]),
    Area::Borders([3190, 3648, 3327, 3839]),
    Area::Borders([3200, 3840, 3390, 3967]),
    Area::Borders([2992, 3912, 3007, 3967]),
    Area::Borders([2946, 3816, 2959, 3831]),
    Area::Borders([3008, 3856, 3199, 3903]),
    Area::Borders([3008, 3600, 3071, 3711]),
    Area::Borders([3072, 3608, 3327, 3647]),
    Area::Borders([2624, 2550, 2690, 2619]),
    Area::Borders([2371, 5062, 2422, 5117]),
    Area::Borders([2896, 3595, 2927, 3630]),
    Area::Borders([2820, 5250, 2955, 5370]),
    Area::Borders([2892, 4435, 2932, 4464]),
    Area::Borders([2724, 5071, 2747, 5109]),
    Area::Borders([2256, 4680, 2287, 4711]),
    Area::Borders([3107, 3234, 3134, 3259]),
    Area::Borders([2931, 3514, 2940, 3518]),
    Area::Borders([2869, 3687, 2940, 3839]),
    Area::Borders([1728, 5120, 1791, 5247]),
    Area::Borders([3136, 3523, 3328, 3710]),
    Area::RegionBorders(13105, [3282, 3159, 3303, 3177]),
    Area::Region(12341),
    Area::Region(13972),
    Area::Region(12107),
    Area::Region(7505),
    Area::Region(10554),
    Area::Region(12337),
    Area::Region(13131),
    Area::Region(14682),
    Area::Plane([3154, 3483, 3176, 3500, 2]),
    Area::Borders([2424, 5105, 2536, 5183]),
    Area::Borders([2487, 10113, 2563, 10174]),
    Area::Region(7236),
    Area::Region(7492),
    Area::Region(12615),
    Area::Region(12616),
    Area::Region(7748),
    Area::Region(12610),
    Area::Borders([3097, 4224, 3225, 4317]),
    Area::Borders([3116, 5412, 3362, 5584]),
    Area::Plane([3078, 5520, 3123, 5552, 0]),
    Area::Borders([2855, 9928, 2880, 9968]),
    Area::Region(11318),
    Area::Region(11844),
    Area::Region(10329),
    Area::Region(13370),
    Area::Region(12603),
    Area::Region(14939),
    Area::Region(9532),
    Area::Region(10810),
    Area::Region(12590),
    Area::Region(10140),
];
/// Pathfinder.isStandingIn.
#[allow(clippy::too_many_arguments)]
pub fn is_standing_in(
    x: i32,
    y: i32,
    mover_size_x: i32,
    mover_size_y: i32,
    dest_x: i32,
    dest_y: i32,
    size_x: i32,
    size_y: i32,
) -> bool {
    if x >= size_x.wrapping_add(dest_x) || mover_size_x.wrapping_add(x) <= dest_x {
        return false;
    }
    if dest_y.wrapping_add(size_y) <= y || y.wrapping_add(mover_size_y) <= dest_y {
        return false;
    }
    true
}
pub struct MultiwayCombatZone;
impl MultiwayCombatZone {
    fn component(resizable: bool) -> i32 {
        if resizable { 17 } else { 7 }
    }
}
impl<H: Host> Behavior<H> for MultiwayCombatZone {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> Result<()> {
        for area in MULTIWAY_AREAS {
            match area {
                Area::Borders([x1, y1, x2, y2]) => {
                    zone.register(host, &Borders::new(x1, y1, x2, y2))?
                }
                Area::Plane([x1, y1, x2, y2, plane]) => {
                    zone.register(host, &Borders::with_plane(x1, y1, x2, y2, plane))?
                }
                Area::Region(id) => zone.register_region(host, id)?,
                Area::RegionBorders(id, [x1, y1, x2, y2]) => {
                    zone.register_region_borders(host, id, &Borders::new(x1, y1, x2, y2))?
                }
            }
        }
        Ok(())
    }
    fn enter(&self, zone: &Zone<H>, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        if host.kind(entity)? == EntityKind::Player {
            let resizable = host.interface_resizable(entity)?;
            let window = host.window_pane_id(entity)?;
            host.send_interface(entity, window, Self::component(resizable), 745, true)?;
            host.interface_config(entity, 745, 1, false)?;
        }
        host.set_multi_zone(entity, true)?;
        zones::base_enter(zone, host, entity)
    }
    fn leave(&self, _: &Zone<H>, host: &mut H, entity: &NodeHandle, logout: bool) -> Result<bool> {
        if !logout && host.kind(entity)? == EntityKind::Player {
            let resizable = host.interface_resizable(entity)?;
            let window = host.window_pane_id(entity)?;
            host.send_close_interface(entity, window, Self::component(resizable), 745, true)?;
        }
        host.set_multi_zone(entity, false)?;
        Ok(true)
    }
    /// The "NPC stacking" rule: the destination may not overlap an attackable
    /// visible NPC that asks for it, within seven tiles.
    fn move_entity(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        _from: Option<&Location>,
        destination: Option<&Location>,
    ) -> Result<bool> {
        if host.npc_walkable(entity)? {
            return Ok(true);
        }
        let location = host.location(entity)?;
        for npc in host.local_npcs(location.as_ref(), RENDERING_DISTANCE / 2)? {
            if host.invisible(&npc)? || !host.has_attack_option(&npc)? || &npc == entity {
                continue;
            }
            if host.should_prevent_stacking(&npc, entity)? {
                let mover = host.size(entity)?;
                let size = host.size(&npc)?;
                let [x, y, _] = destination.ok_or(Error::Null)?.coordinates();
                let [npc_x, npc_y, _] = host.location(&npc)?.ok_or(Error::Null)?.coordinates();
                if is_standing_in(x, y, mover, mover, npc_x, npc_y, size, size) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}
pub fn multiway_combat_zone<H: Host>() -> Zone<H> {
    Zone::new(Some("Multicombat"), true, &[], MultiwayCombatZone)
}
