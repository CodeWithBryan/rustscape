//! DarkZone from `core.game.world.map.zone.impl`: its configure, combat,
//! interaction, enter, leave and overlay rules, the DARKNESS_OVERLAY
//! component's open/close bodies with their insect pulse, checkDarkArea, the
//! tinderbox UseWith hook, and the LightSource table it reads
//! (`content.data.LightSource`).
//!
//! A library: nothing installs it. The InterfaceManager, the inventory and
//! equipment snapshots, the skillcape perk, event hooks, queued scripts,
//! runTask, item names, extensions and the impact handler are required
//! services called in the original order; none has a default and no lock is
//! held across a service or zone callback. The insect pulse is a native
//! `pulses::Pulse` that the owning world dispatches by identity to
//! `insect_pulse`; the queued glow script and the tinderbox task are typed
//! native objects run through `glow_script` and `tinderbox_task`.
use super::{
    player_walk::NodeHandle,
    zones::{self, Behavior, Borders, EntityKind, Error, Result, Zone},
};
use crate::{definitions::item::Shared, pulses};
use rs09_game_core::java_lower::lowercase_root;
use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

/// `Components.DARKNESS_DARK_96`, the id of DARKNESS_OVERLAY.
pub const DARKNESS_OVERLAY_ID: i32 = 96;
/// `Components.DARKNESS_LIGHT_97`.
pub const DARKNESS_LIGHT: i32 = 97;
/// `Components.DARKNESS_MEDIUM_98`.
pub const DARKNESS_MEDIUM: i32 = 98;
/// The class of the anonymous insect pulse inside DARKNESS_OVERLAY.open.
pub const INSECT_PULSE_CLASS: &str = "core.game.world.map.zone.impl.DarkZone$1$1";
pub const INSECTS_HEARD: &str = "You hear tiny insects skittering over the ground...";
pub const INSECTS_SWARM: &str = "Tiny biting insects swarm all over you!";

/// A component handed to the InterfaceManager. DARKNESS_OVERLAY is one
/// object; every other component here is a fresh `new Component(id)`, so
/// only its id is observable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    /// DarkZone.DARKNESS_OVERLAY. The manager's open/close of this object
    /// run `darkness_open` / `darkness_close`.
    Darkness,
    /// `new Component(id)`.
    Id(i32),
}
impl Component {
    pub fn id(self) -> i32 {
        match self {
            Self::Darkness => DARKNESS_OVERLAY_ID,
            Self::Id(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStrength {
    Weak,
    Normal,
    Strong,
    Soft,
}
/// ImpactHandler.HitsplatType as DarkZone names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hitsplat {
    Normal,
}

/// The InterfaceManager and node services WildernessZone and DarkZone share.
pub trait Overlays: zones::Host {
    /// `interfaceManager.openOverlay(component)`. The manager closes the old
    /// overlay and opens this one through the components' own open/close.
    fn open_overlay(&mut self, player: &NodeHandle, component: Component) -> Result<()>;
    /// `interfaceManager.closeOverlay()`.
    fn close_overlay(&mut self, player: &NodeHandle) -> Result<()>;
    /// `interfaceManager.close(component)`.
    fn close_component(&mut self, player: &NodeHandle, component: Component) -> Result<bool>;
    /// `interfaceManager.getOverlay()`; every call is a read.
    fn overlay(&mut self, player: &NodeHandle) -> Result<Option<Component>>;
    /// `node instanceof Item` (a lookup, not a call of the original).
    fn is_item(&mut self, target: &Self::Target) -> Result<bool>;
}

/// DarkZone's own services.
pub trait Host: Overlays {
    /// `player.getInventory().toArray()`: the item ids, null slots as None.
    fn inventory_items(&mut self, player: &NodeHandle) -> Result<Vec<Option<i32>>>;
    /// `player.getEquipment().toArray()`.
    fn equipment_items(&mut self, player: &NodeHandle) -> Result<Vec<Option<i32>>>;
    /// `SkillcapePerks.isActive(SkillcapePerks.CONSTANT_GLOW, player)`.
    fn constant_glow(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `option.getName()`; the service owns a null option.
    fn option_name(&mut self, option: &Self::Opaque) -> Result<Option<String>>;
    /// `entity.hook(Event.getUsedWith(), zone)`.
    fn hook_used_with(&mut self, entity: &NodeHandle, zone: &Zone<Self>) -> Result<()>;
    /// `entity.unhook(zone)`.
    fn unhook(&mut self, entity: &NodeHandle, zone: &Zone<Self>) -> Result<()>;
    /// `player.getExtension(DarkZone.class)`.
    fn dark_extension(&mut self, player: &NodeHandle) -> Result<Option<InsectPulse>>;
    /// `player.addExtension(DarkZone.class, pulse)`.
    fn add_dark_extension(&mut self, player: &NodeHandle, pulse: InsectPulse) -> Result<()>;
    /// `GameWorld.getPulser().submit(pulse)`.
    fn pulser_submit(&mut self, pulse: &pulses::Handle) -> Result<()>;
    /// Component.open, the base class body (`super.open(player)`).
    fn component_open(&mut self, player: &NodeHandle, component: Component) -> Result<()>;
    /// Component.close, the base class body (`super.close(player)`).
    fn component_close(&mut self, player: &NodeHandle, component: Component) -> Result<bool>;
    /// `entity.getImpactHandler().manualHit(source, hit, type)`.
    fn manual_hit(
        &mut self,
        entity: &NodeHandle,
        source: &NodeHandle,
        hit: i32,
        kind: Hitsplat,
    ) -> Result<()>;
    /// ContentAPI.queueScript(entity, delay, strength, persist, script).
    fn queue_script(
        &mut self,
        entity: &NodeHandle,
        delay: i32,
        strength: QueueStrength,
        persist: bool,
        script: GlowScript,
    ) -> Result<()>;
    /// ContentAPI.stopExecuting(entity).
    fn stop_executing(&mut self, entity: &NodeHandle) -> Result<bool>;
    /// ContentAPI.runTask(entity, delay, repeatTimes, task).
    fn run_task(
        &mut self,
        entity: &NodeHandle,
        delay: i32,
        repeat: i32,
        task: TinderboxTask,
    ) -> Result<()>;
    /// ContentAPI.getItemName(id); None is a null name.
    fn item_name_by_id(&mut self, id: i32) -> Result<Option<String>>;
}

/// `content.data.LightSource`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightSource {
    Candle,
    BlackCandle,
    Torch,
    CandleLantern,
    OilLamp,
    OilLantern,
    BullseyeLantern,
    SapphireLantern,
    EmeraldLantern,
    MiningHelmet,
}
impl LightSource {
    pub const ALL: [Self; 10] = [
        Self::Candle,
        Self::BlackCandle,
        Self::Torch,
        Self::CandleLantern,
        Self::OilLamp,
        Self::OilLantern,
        Self::BullseyeLantern,
        Self::SapphireLantern,
        Self::EmeraldLantern,
        Self::MiningHelmet,
    ];
    /// (level, raw item, product item, open, interface id).
    fn row(self) -> (i32, i32, i32, bool, i32) {
        match self {
            Self::Candle => (1, 36, 33, true, DARKNESS_MEDIUM),
            Self::BlackCandle => (1, 38, 32, true, DARKNESS_MEDIUM),
            Self::Torch => (1, 596, 594, true, DARKNESS_MEDIUM),
            Self::CandleLantern => (4, 4529, 4531, false, DARKNESS_MEDIUM),
            Self::OilLamp => (12, 4522, 4524, true, DARKNESS_LIGHT),
            Self::OilLantern => (26, 4537, 4539, false, DARKNESS_LIGHT),
            Self::BullseyeLantern => (49, 4548, 4550, false, -1),
            Self::SapphireLantern => (49, 4701, 4702, false, -1),
            Self::EmeraldLantern => (49, 9064, 9065, false, -1),
            Self::MiningHelmet => (65, 5014, 5013, false, DARKNESS_LIGHT),
        }
    }
    pub fn level(self) -> i32 {
        self.row().0
    }
    pub fn raw(self) -> i32 {
        self.row().1
    }
    pub fn product(self) -> i32 {
        self.row().2
    }
    pub fn is_open(self) -> bool {
        self.row().3
    }
    pub fn interface_id(self) -> i32 {
        self.row().4
    }
    /// `name().toLowerCase().replaceAll("_", " ")`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Candle => "candle",
            Self::BlackCandle => "black candle",
            Self::Torch => "torch",
            Self::CandleLantern => "candle lantern",
            Self::OilLamp => "oil lamp",
            Self::OilLantern => "oil lantern",
            Self::BullseyeLantern => "bullseye lantern",
            Self::SapphireLantern => "sapphire lantern",
            Self::EmeraldLantern => "emerald lantern",
            Self::MiningHelmet => "mining helmet",
        }
    }
    /// 1 = dim, 2 = medium, 3 = bright.
    pub fn strength(self) -> i32 {
        match self.interface_id() {
            DARKNESS_LIGHT => 1,
            DARKNESS_MEDIUM => 2,
            -1 => 3,
            _ => 0,
        }
    }
    /// The first source whose raw item has this id.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|light| light.raw() == id)
    }
    /// The first source whose product has this id.
    pub fn for_product_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|light| light.product() == id)
    }
    /// `getRawIds()`: its loop stops one short, so the last entry stays 0.
    pub fn raw_ids() -> Vec<i32> {
        let mut ids = vec![0; Self::ALL.len()];
        for (index, light) in Self::ALL[..Self::ALL.len() - 1].iter().enumerate() {
            ids[index] = light.raw();
        }
        ids
    }
}

/// `LightSource.getActiveLightSource(player)`: the inventory snapshot first,
/// then the equipment snapshot; the first lit item wins.
pub fn active_light_source<H: Host>(
    host: &mut H,
    player: &NodeHandle,
) -> Result<Option<LightSource>> {
    for item in host.inventory_items(player)?.into_iter().flatten() {
        if let Some(source) = LightSource::for_product_id(item) {
            return Ok(Some(source));
        }
    }
    for item in host.equipment_items(player)?.into_iter().flatten() {
        if let Some(source) = LightSource::for_product_id(item) {
            return Ok(Some(source));
        }
    }
    Ok(None)
}

/// `LightSource.hasActiveLightSource(player)`.
pub fn has_active_light_source<H: Host>(host: &mut H, player: &NodeHandle) -> Result<bool> {
    Ok(active_light_source(host, player)?.is_some())
}

struct InsectFields {
    handle: pulses::Handle,
    player: NodeHandle,
    count: AtomicI32,
}
/// The anonymous `new Pulse(2, player)` of DARKNESS_OVERLAY.open with its
/// `count` field. The world dispatches its handle to `insect_pulse`.
#[derive(Clone)]
pub struct InsectPulse(Arc<InsectFields>);
impl InsectPulse {
    pub fn new(player: &NodeHandle) -> Self {
        let handle = Shared::new(pulses::Pulse::new(
            2,
            Some(Shared::new(vec![Some(player.node)])),
            pulses::Class::Other,
            INSECT_PULSE_CLASS.into(),
        ));
        Self(Arc::new(InsectFields {
            handle,
            player: player.clone(),
            count: AtomicI32::new(0),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn handle(&self) -> &pulses::Handle {
        &self.0.handle
    }
    pub fn player(&self) -> &NodeHandle {
        &self.0.player
    }
    pub fn count(&self) -> i32 {
        self.0.count.load(Ordering::Relaxed)
    }
}
/// The insect pulse body: a message at 0 and at 5, a 1 hit after 5, never
/// finishing.
pub fn insect_pulse<H: Host>(host: &mut H, pulse: &InsectPulse) -> Result<bool> {
    let player = pulse.player().clone();
    let count = pulse.count();
    if count == 0 {
        host.packet_message(&player, INSECTS_HEARD)?;
    } else if count == 5 {
        host.packet_message(&player, INSECTS_SWARM)?;
    } else if count > 5 {
        host.manual_hit(&player, &player, 1, Hitsplat::Normal)?;
    }
    pulse
        .0
        .count
        .store(count.wrapping_add(1), Ordering::Relaxed);
    Ok(false)
}

/// DARKNESS_OVERLAY.open(player): nothing while the stored pulse still runs;
/// otherwise a new insect pulse is submitted and stored before the
/// component itself opens.
pub fn darkness_open<H: Host>(host: &mut H, player: &NodeHandle) -> Result<()> {
    if let Some(pulse) = host.dark_extension(player)? {
        if pulses::is_running_base(pulse.handle()) {
            return Ok(());
        }
    }
    let pulse = InsectPulse::new(player);
    host.pulser_submit(pulse.handle())?;
    host.add_dark_extension(player, pulse)?;
    host.component_open(player, Component::Darkness)
}
/// DARKNESS_OVERLAY.close(player): the stored pulse stops only after the
/// component closed.
pub fn darkness_close<H: Host>(host: &mut H, player: &NodeHandle) -> Result<bool> {
    if !host.component_close(player, Component::Darkness)? {
        return Ok(false);
    }
    if let Some(pulse) = host.dark_extension(player)? {
        pulses::stop_base(pulse.handle());
    }
    Ok(true)
}

/// The script updateOverlay queues for a player with the constant-glow perk.
#[derive(Clone)]
pub struct GlowScript {
    pub player: NodeHandle,
}
/// Its body: close the overlay when the manager's current overlay (which
/// may not be null) has the darkness id, then stopExecuting.
pub fn glow_script<H: Host>(host: &mut H, script: &GlowScript, _stage: i32) -> Result<bool> {
    let player = &script.player;
    if host.overlay(player)?.ok_or(Error::Null)?.id() == DARKNESS_OVERLAY_ID {
        host.close_overlay(player)?;
    }
    host.stop_executing(player)
}

/// The task the tinderbox hook passes to runTask.
#[derive(Clone)]
pub struct TinderboxTask {
    pub entity: NodeHandle,
}
/// Its body: `checkDarkArea(entity.asPlayer())`.
pub fn tinderbox_task<H: Host>(host: &mut H, task: &TinderboxTask) -> Result<()> {
    if host.kind(&task.entity)? != EntityKind::Player {
        return Err(Error::ClassCast);
    }
    check_dark_area(host, &task.entity)?;
    Ok(())
}

/// `DarkZone.checkDarkArea(p)`: the first DarkZone in the monitor's live
/// list updates the overlay.
pub fn check_dark_area<H: Host>(host: &mut H, player: &NodeHandle) -> Result<bool> {
    let list = host.monitor(player)?.zones();
    Ok(zones::each(&list, |region_zone| {
        let zone = region_zone.zone().clone();
        match zone.behavior::<DarkZone>() {
            Some(dark) => {
                dark.update_overlay(host, player)?;
                Ok(Some(()))
            }
            None => Ok(None),
        }
    })?
    .is_some())
}

/// The UseWith hook: a tinderbox on either side runs checkDarkArea two
/// ticks later for a player.
pub fn process_use_with<H: Host>(
    host: &mut H,
    entity: &NodeHandle,
    used: i32,
    with: i32,
) -> Result<()> {
    let tinderbox = host.item_name_by_id(used)?.ok_or(Error::Null)? == "Tinderbox"
        || host.item_name_by_id(with)?.ok_or(Error::Null)? == "Tinderbox";
    if tinderbox && host.kind(entity)? == EntityKind::Player {
        host.run_task(
            entity,
            2,
            1,
            TinderboxTask {
                entity: entity.clone(),
            },
        )?;
    }
    Ok(())
}

/// DarkZone.
pub struct DarkZone;
impl DarkZone {
    /// `updateOverlay(player)`.
    pub fn update_overlay<H: Host>(&self, host: &mut H, player: &NodeHandle) -> Result<()> {
        let source = active_light_source(host, player)?;
        if host.constant_glow(player)? {
            host.queue_script(
                player,
                1,
                QueueStrength::Soft,
                false,
                GlowScript {
                    player: player.clone(),
                },
            )?;
        }
        let mut overlay = -1;
        if host.overlay(player)?.is_some() {
            overlay = host.overlay(player)?.ok_or(Error::Null)?.id();
        }
        let Some(source) = source else {
            if overlay != DARKNESS_OVERLAY_ID {
                host.open_overlay(player, Component::Darkness)?;
            }
            return Ok(());
        };
        if let Some(pulse) = host.dark_extension(player)? {
            pulses::stop_base(pulse.handle());
        }
        if source.interface_id() != overlay {
            if source.interface_id() == -1 {
                host.close_overlay(player)?;
                return Ok(());
            }
            host.open_overlay(player, Component::Id(source.interface_id()))?;
        }
        Ok(())
    }
}
impl<H: Host> Behavior<H> for DarkZone {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> Result<()> {
        zone.register(host, &Borders::new(1728, 5120, 1791, 5247))?;
        zone.register_region(host, 12693)?;
        zone.register_region(host, 12948)?;
        zone.register_region(host, 12949)?;
        zone.register(host, &Borders::new(3306, 9661, 3222, 9600))?;
        zone.register(host, &Borders::new(3717, 9473, 3841, 9346))
    }
    /// A player whose overlay is DARKNESS_OVERLAY itself cannot attack.
    fn continue_attack(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        _target: Option<&H::Target>,
        _style: Option<&H::Opaque>,
        _message: bool,
    ) -> Result<bool> {
        if host.kind(entity)? == EntityKind::Player {
            return Ok(host.overlay(entity)? != Some(Component::Darkness));
        }
        Ok(true)
    }
    /// Dropping or extinguishing a lit light source is refused.
    fn interact(
        &self,
        _: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        target: Option<&H::Target>,
        option: Option<&H::Opaque>,
    ) -> Result<bool> {
        let Some(target) = target else {
            return Ok(false);
        };
        if !host.is_item(target)? {
            return Ok(false);
        }
        let id = host.target_item_id(Some(target))?;
        if let Some(source) = LightSource::for_product_id(id) {
            let name = host.option_name(option.ok_or(Error::Null)?)?;
            let name = lowercase_root(&name.ok_or(Error::Null)?);
            for (verb, word) in [("drop", "Dropping"), ("extinguish", "Extinguishing")] {
                if name == verb {
                    if host.kind(entity)? != EntityKind::Player {
                        return Err(Error::ClassCast);
                    }
                    host.packet_message(
                        entity,
                        &format!(
                            "{word} the {} would leave you without a light source.",
                            source.name()
                        ),
                    )?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn enter(&self, zone: &Zone<H>, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        if host.kind(entity)? == EntityKind::Player {
            let source = active_light_source(host, entity)?;
            if host.constant_glow(entity)? {
                return Ok(true);
            }
            match source {
                None => host.open_overlay(entity, Component::Darkness)?,
                Some(source) if source.interface_id() > 0 => {
                    host.open_overlay(entity, Component::Id(source.interface_id()))?
                }
                Some(_) => {}
            }
        }
        host.hook_used_with(entity, zone)?;
        Ok(true)
    }
    fn leave(&self, zone: &Zone<H>, host: &mut H, entity: &NodeHandle, _: bool) -> Result<bool> {
        if host.kind(entity)? == EntityKind::Player {
            host.close_overlay(entity)?;
        }
        host.unhook(entity, zone)?;
        Ok(true)
    }
}
/// `new DarkZone()`.
pub fn dark_zone<H: Host>() -> Zone<H> {
    Zone::new(Some("Dark zone"), true, &[], DarkZone)
}
