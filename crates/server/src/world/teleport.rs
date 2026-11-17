//! `TeleportManager`, every `TeleportType` with its anonymous pulse, and
//! `TeleportPulse` from `core.game.node.entity.player.link`.
//!
//! Each teleport pulse is a native [`pulses::Pulse`] (delay 1, no checks)
//! submitted to the world [`pulses::Runner`]; the owning world's pulse context
//! dispatches the handle, by identity, to [`TeleportPulse::pulse`],
//! [`TeleportPulse::stop`] and [`TeleportPulse::start`]. Animations and
//! graphics are native [`Animation`]/[`Graphics`] objects handed to the
//! entity's Animator, and the destination is a [`Location`] the walking queue
//! moves the entity to (`WalkingQueue.updateTeleport`). Everything the
//! original reaches outside these classes is a required [`Host`] service,
//! called in the original order with the original arguments. No service has a
//! default and no lock is held across a service call.
//!
//! A library: nothing installs it in a session or a live world.
use super::{
    animation::{Animation, Graphics, Priority},
    doors::Location,
};
use crate::{definitions::item::Shared, pulses};
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// java.lang.NullPointerException (a null type, destination or entity location).
    Null,
    /// java.lang.ClassCastException (a non-Boolean "tablet-spell" attribute).
    ClassCast,
    /// java.util.ConcurrentModificationException from the zone walk.
    ConcurrentModification,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// `TeleportManager.WILDY_TELEPORT`.
pub const WILDY_TELEPORT: i32 = 1 << 16 | 8;
/// `Audio.defaultAudioRadius`, the default radius of playAudio/playGlobalAudio.
pub const DEFAULT_AUDIO_RADIUS: i32 = 8;
/// The ConstLib `Sounds` ids TeleportManager names (each carries its id as its suffix).
pub mod sounds {
    pub const BLOCK_TELEPORT_197: i32 = 197;
    pub const TELE_OTHER_CAST_199: i32 = 199;
    pub const TELEPORT_ALL_200: i32 = 200;
    pub const TELEPORT_REVERSE_201: i32 = 201;
    pub const POH_TABLET_BREAK_979: i32 = 979;
    pub const FT_FAIRY_TELEPORT_1098: i32 = 1098;
}
/// The message of a refused tele-blocked teleport.
pub const TELE_BLOCKED: &str = "A magical force has stopped you from teleporting.";

/// `TeleportSettings`: start and end animation, start and end graphic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub start_anim: i32,
    pub end_anim: i32,
    pub start_gfx: i32,
    pub end_gfx: i32,
}
const fn settings(start_anim: i32, end_anim: i32, start_gfx: i32, end_gfx: i32) -> Settings {
    Settings {
        start_anim,
        end_anim,
        start_gfx,
        end_gfx,
    }
}

/// `TeleportManager.TeleportType`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportType {
    Normal,
    Ancient,
    Lunar,
    Teletabs,
    Obelisk,
    TeleOther,
    FairyRing,
    PuroPuro,
    Ectophial,
    Christmas,
    Cabbage,
    EntranaMagicDoor,
    Minigame,
    Instant,
    RcGuild,
}
impl TeleportType {
    pub const ALL: [Self; 15] = [
        Self::Normal,
        Self::Ancient,
        Self::Lunar,
        Self::Teletabs,
        Self::Obelisk,
        Self::TeleOther,
        Self::FairyRing,
        Self::PuroPuro,
        Self::Ectophial,
        Self::Christmas,
        Self::Cabbage,
        Self::EntranaMagicDoor,
        Self::Minigame,
        Self::Instant,
        Self::RcGuild,
    ];
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Ancient => "ANCIENT",
            Self::Lunar => "LUNAR",
            Self::Teletabs => "TELETABS",
            Self::Obelisk => "OBELISK",
            Self::TeleOther => "TELE_OTHER",
            Self::FairyRing => "FAIRY_RING",
            Self::PuroPuro => "PURO_PURO",
            Self::Ectophial => "ECTOPHIAL",
            Self::Christmas => "CHRISTMAS",
            Self::Cabbage => "CABBAGE",
            Self::EntranaMagicDoor => "ENTRANA_MAGIC_DOOR",
            Self::Minigame => "MINIGAME",
            Self::Instant => "INSTANT",
            Self::RcGuild => "RC_GUILD",
        }
    }
    /// `TeleportType.valueOf(name)`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
    /// `getSettings()`: the constructor arguments of each constant.
    pub fn settings(self) -> Settings {
        match self {
            Self::Normal => settings(8939, 8941, 1576, 1577),
            Self::Ancient => settings(1979, -1, 392, -1),
            Self::Lunar => settings(1816, -1, 747, -1),
            Self::Teletabs => settings(4731, -1, 678, -1),
            Self::Obelisk => settings(8939, 8941, 661, -1),
            Self::TeleOther => settings(1816, -1, 342, -1),
            Self::FairyRing => settings(-1, -1, -1, -1),
            Self::PuroPuro => settings(6601, 1118, -1, -1),
            Self::Ectophial => settings(8939, 8941, 1587, 1588),
            Self::Christmas => settings(7534, -1, 1292, -1),
            Self::Cabbage => settings(9984, 9986, 1731, 1732),
            Self::EntranaMagicDoor => settings(10100, 9013, 1745, 1747),
            Self::Minigame => settings(6601, 1118, -1, -1),
            Self::Instant => settings(-1, -1, -1, -1),
            Self::RcGuild => settings(10180, 10182, 1771, 1772),
        }
    }
    /// The JVM name of the constant's anonymous pulse class (the pulse's class
    /// name for telemetry; it selects nothing).
    pub fn class_name(self) -> String {
        format!(
            "core.game.node.entity.player.link.TeleportManager$TeleportType${}$1",
            self.ordinal() + 1
        )
    }
    /// `getPulse(entity, location)`. FAIRY_RING shows its graphic before the
    /// pulse exists.
    pub fn pulse<H: Host>(
        self,
        host: &mut H,
        entity: &H::Entity,
        location: Option<&Location>,
    ) -> Result<TeleportPulse<H::Entity>> {
        if self == Self::FairyRing {
            host.graphics(entity, Graphics::new(569, 0, 0))?;
        }
        Ok(TeleportPulse::new(self, entity.clone(), location.cloned()))
    }
}

/// `TeleportManager.getType(player)` from the player's spell book interface.
pub fn get_type(spell_book: i32) -> TeleportType {
    match spell_book {
        193 => TeleportType::Ancient,
        430 => TeleportType::Lunar,
        _ => TeleportType::Normal,
    }
}

/// Required services. None has a default; an owning world answers each from
/// the real system or fails. `is_player` and `teleporter` are lookups
/// (`instanceof Player`, the entity's final teleporter field).
pub trait Host {
    type Entity: Clone;
    fn is_player(&mut self, entity: &Self::Entity) -> Result<bool>;
    fn teleporter(&mut self, entity: &Self::Entity) -> Result<Manager<Self::Entity>>;
    /// `player.getSpellBookManager().getSpellBook()`.
    fn spell_book(&mut self, player: &Self::Entity) -> Result<i32>;
    /// `hasTimerActive(entity, name)`: `entity.timers.getTimer(name) != null`.
    fn has_timer(&mut self, entity: &Self::Entity, name: &str) -> Result<bool>;
    /// `entity.getZoneMonitor().teleport(type, null)`.
    fn zone_teleport(&mut self, entity: &Self::Entity, teleport_type: i32) -> Result<bool>;
    /// `entity.isTeleBlocked()`.
    fn is_tele_blocked(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `player.sendMessage(text)`.
    fn send_message(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `GameWorld.getTicks()`.
    fn world_ticks(&mut self) -> Result<i32>;
    /// `player.scripts.setDelay(value)`.
    fn set_script_delay(&mut self, player: &Self::Entity, value: i32) -> Result<()>;
    /// `player.getDialogueInterpreter().close()`; its result is ignored.
    fn close_dialogue(&mut self, player: &Self::Entity) -> Result<()>;
    /// `entity.getAttribute("tablet-spell", false)` unboxed: a stored
    /// non-Boolean is a ClassCast failure.
    fn tablet_spell(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `entity.getAttribute(key, null) != null`.
    fn has_attribute(&mut self, entity: &Self::Entity, key: &str) -> Result<bool>;
    /// `entity.setAttribute("tele-pulse", pulse)` (store, then the event).
    fn set_tele_pulse(
        &mut self,
        entity: &Self::Entity,
        pulse: &TeleportPulse<Self::Entity>,
    ) -> Result<()>;
    /// `entity.removeAttribute(key)` (store, then the event).
    fn remove_attribute(&mut self, entity: &Self::Entity, key: &str) -> Result<()>;
    /// `entity.getWalkingQueue().reset()`.
    fn reset_walking(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `entity.getPulseManager().clear()`.
    fn clear_pulses(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `entity.lock(ticks)`: ActionLocks.lock(ticks).
    fn lock(&mut self, entity: &Self::Entity, ticks: i32) -> Result<()>;
    /// `entity.lock()`: ActionLocks.lock().
    fn lock_forever(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `entity.unlock()`: ActionLocks.unlock().
    fn unlock(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `entity.getImpactHandler().setDisabledTicks(ticks)`.
    fn set_disabled_ticks(&mut self, entity: &Self::Entity, ticks: i32) -> Result<()>;
    /// `GameWorld.getPulser().submit(pulse)`.
    fn submit(&mut self, pulse: &TeleportPulse<Self::Entity>) -> Result<()>;
    /// `player.getInterfaceManager().close()`; its result is ignored.
    fn close_interfaces(&mut self, player: &Self::Entity) -> Result<()>;
    /// `entity.getLocation()`.
    fn location(&mut self, entity: &Self::Entity) -> Result<Option<Location>>;
    /// `entity.getProperties().getTeleportLocation()`.
    fn teleport_location(&mut self, entity: &Self::Entity) -> Result<Option<Location>>;
    /// `entity.getProperties().setTeleportLocation(location)`.
    fn set_teleport_location(
        &mut self,
        entity: &Self::Entity,
        location: Option<Location>,
    ) -> Result<()>;
    /// `playGlobalAudio(location, id, delay, loops, radius)`.
    fn play_global_audio(
        &mut self,
        location: &Location,
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> Result<()>;
    /// `playAudio(player, id, delay, loops, location, radius)`.
    fn play_audio(
        &mut self,
        player: &Self::Entity,
        id: i32,
        delay: i32,
        loops: i32,
        location: Option<&Location>,
        radius: i32,
    ) -> Result<()>;
    /// `entity.getAnimator().forceAnimation(animation)`.
    fn force_animation(&mut self, entity: &Self::Entity, animation: Animation) -> Result<()>;
    /// `entity.animate(animation)`: Animator.animate(animation, null).
    fn animate(&mut self, entity: &Self::Entity, animation: Animation) -> Result<bool>;
    /// `entity.graphics(graphics)`: Animator.animate(null, graphics).
    fn graphics(&mut self, entity: &Self::Entity, graphics: Graphics) -> Result<bool>;
    /// The world's shared `Animation.RESET` object.
    fn animation_reset(&mut self) -> Animation;
}

struct PulseFields<E> {
    handle: pulses::Handle,
    kind: TeleportType,
    entity: E,
    location: Option<Location>,
    /// The anonymous class's own `int delay` counter.
    counter: AtomicI32,
}
/// One anonymous TeleportPulse: the native pulse handle the runner holds and
/// the captured entity, destination and step counter of its body.
pub struct TeleportPulse<E>(Arc<PulseFields<E>>);
impl<E> Clone for TeleportPulse<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
fn copy(location: &Option<Location>) -> Result<Location> {
    // Location.create(location): a new object with the same packed coordinates.
    let [x, y, z] = location.as_ref().ok_or(Error::Null)?.coordinates();
    Ok(Location::new(x, y, z))
}
impl<E: Clone> TeleportPulse<E> {
    fn new(kind: TeleportType, entity: E, location: Option<Location>) -> Self {
        Self(Arc::new(PulseFields {
            handle: Shared::new(pulses::Pulse::new(
                1,
                None,
                pulses::Class::Other,
                kind.class_name(),
            )),
            kind,
            entity,
            location,
            counter: AtomicI32::new(0),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn handle(&self) -> &pulses::Handle {
        &self.0.handle
    }
    pub fn kind(&self) -> TeleportType {
        self.0.kind
    }
    pub fn entity(&self) -> &E {
        &self.0.entity
    }
    /// The captured destination object (FAIRY_RING keeps it as the teleport
    /// location; every other type copies it when it moves).
    pub fn location(&self) -> Option<&Location> {
        self.0.location.as_ref()
    }
    pub fn counter(&self) -> i32 {
        self.0.counter.load(Ordering::Relaxed)
    }
    fn advance(&self) -> i32 {
        self.0
            .counter
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
    }
    fn set_destination<H: Host<Entity = E>>(&self, host: &mut H, copied: bool) -> Result<()> {
        let location = if copied {
            Some(copy(&self.0.location)?)
        } else {
            self.0.location.clone()
        };
        host.set_teleport_location(&self.0.entity, location)
    }
    fn global_audio<H: Host<Entity = E>>(
        &self,
        host: &mut H,
        id: i32,
        delay: i32,
        loops: i32,
    ) -> Result<()> {
        // playGlobalAudio's location parameter is non-null Kotlin.
        let location = host.location(&self.0.entity)?.ok_or(Error::Null)?;
        host.play_global_audio(&location, id, delay, loops, DEFAULT_AUDIO_RADIUS)
    }
    fn player_audio<H: Host<Entity = E>>(&self, host: &mut H, id: i32) -> Result<()> {
        if host.is_player(&self.0.entity)? {
            host.play_audio(&self.0.entity, id, 0, 1, None, DEFAULT_AUDIO_RADIUS)?;
        }
        Ok(())
    }
    fn force<H: Host<Entity = E>>(
        &self,
        host: &mut H,
        id: i32,
        priority: Option<Priority>,
    ) -> Result<()> {
        let animation = match priority {
            Some(priority) => Animation::with(id, 0, Some(priority)),
            None => Animation::new(id),
        };
        host.force_animation(&self.0.entity, animation)
    }
    fn gfx<H: Host<Entity = E>>(&self, host: &mut H, id: i32, height: i32) -> Result<()> {
        host.graphics(&self.0.entity, Graphics::new(id, height, 0))?;
        Ok(())
    }
    /// The body (`pulse()`): one step per call, true when the type is done.
    pub fn pulse<H: Host<Entity = E>>(&self, host: &mut H) -> Result<bool> {
        let s = self.0.kind.settings();
        let entity = &self.0.entity;
        let delay = self.counter();
        match self.0.kind {
            TeleportType::Normal
            | TeleportType::Ectophial
            | TeleportType::Christmas
            | TeleportType::EntranaMagicDoor => {
                if delay == 0 {
                    self.global_audio(host, sounds::TELEPORT_ALL_200, 0, 1)?;
                    self.force(host, s.start_anim, None)?;
                    self.gfx(host, s.start_gfx, 0)?;
                } else if delay == 3 {
                    self.set_destination(host, true)?;
                    fire_random(host, entity)?;
                } else if delay == 4 {
                    self.global_audio(host, sounds::TELEPORT_REVERSE_201, 0, 1)?;
                    self.force(host, s.end_anim, Some(Priority::High))?;
                    self.gfx(host, s.end_gfx, 0)?;
                    return Ok(true);
                }
            }
            TeleportType::Ancient => {
                if delay == 0 {
                    self.global_audio(host, sounds::BLOCK_TELEPORT_197, 0, 7)?;
                    self.force(host, s.start_anim, None)?;
                    self.gfx(host, s.start_gfx, 0)?;
                } else if delay == 4 {
                    self.set_destination(host, true)?;
                    fire_random(host, entity)?;
                } else if delay == 5 {
                    self.force(host, s.end_anim, Some(Priority::High))?;
                    self.gfx(host, s.end_gfx, 0)?;
                    return Ok(true);
                }
            }
            TeleportType::Lunar => {
                if delay == 0 {
                    self.gfx(host, s.start_gfx, 120)?;
                    self.force(host, s.start_anim, None)?;
                } else if delay == 3 {
                    self.set_destination(host, true)?;
                    fire_random(host, entity)?;
                } else if delay == 4 {
                    self.force(host, s.end_anim, Some(Priority::High))?;
                    self.gfx(host, s.end_gfx, 0)?;
                    return Ok(true);
                }
            }
            TeleportType::Teletabs => {
                if delay == 0 {
                    self.global_audio(host, sounds::POH_TABLET_BREAK_979, 0, 1)?;
                    self.force(host, 4069, None)?;
                } else if delay == 2 {
                    self.force(host, s.start_anim, Some(Priority::High))?;
                    self.gfx(host, s.start_gfx, 0)?;
                } else if delay == 3 {
                    self.set_destination(host, true)?;
                } else if delay == 4 {
                    self.force(host, s.end_anim, None)?;
                    self.gfx(host, s.end_gfx, 0)?;
                    return Ok(true);
                }
            }
            TeleportType::Obelisk | TeleportType::Instant => {
                if delay == 0 {
                    host.lock_forever(entity)?;
                    if self.0.kind == TeleportType::Obelisk {
                        self.force(host, 1816, None)?;
                    }
                } else if delay == 3 {
                    self.set_destination(host, true)?;
                } else if delay == 4 {
                    let reset = host.animation_reset();
                    host.force_animation(entity, reset)?;
                    host.unlock(entity)?;
                    return Ok(true);
                }
            }
            TeleportType::TeleOther => {
                if delay == 0 {
                    self.global_audio(host, sounds::TELE_OTHER_CAST_199, 0, 1)?;
                    self.force(host, s.start_anim, None)?;
                    self.gfx(host, s.start_gfx, 0)?;
                } else if delay == 3 {
                    self.set_destination(host, true)?;
                } else if delay == 4 {
                    self.force(host, -1, None)?;
                    host.unlock(entity)?;
                    return Ok(true);
                }
            }
            TeleportType::FairyRing => {
                // switch (++delay)
                match self.advance() {
                    2 => {
                        host.animate(entity, Animation::new(3265))?;
                        self.player_audio(host, sounds::FT_FAIRY_TELEPORT_1098)?;
                    }
                    4 => {
                        host.animate(entity, Animation::new(3266))?;
                        self.set_destination(host, false)?;
                        host.unlock(entity)?;
                        host.lock(entity, 2)?;
                        return Ok(true);
                    }
                    _ => {}
                }
                return Ok(false);
            }
            TeleportType::PuroPuro | TeleportType::Minigame => {
                if delay == 0 {
                    self.force(host, s.start_anim, None)?;
                    self.gfx(host, 1118, 0)?;
                } else if delay == 9 {
                    self.set_destination(host, true)?;
                    self.force(host, -1, None)?;
                    host.unlock(entity)?;
                    return Ok(true);
                }
            }
            TeleportType::Cabbage => {
                if delay == 0 {
                    self.player_audio(host, 5036)?;
                    self.force(host, s.start_anim, None)?;
                    self.gfx(host, s.start_gfx, 0)?;
                } else if delay == 5 {
                    self.set_destination(host, true)?;
                    fire_random(host, entity)?;
                    self.player_audio(host, 5034)?;
                    self.force(host, s.end_anim, Some(Priority::High))?;
                    self.gfx(host, s.end_gfx, 0)?;
                    return Ok(true);
                }
            }
            TeleportType::RcGuild => match delay {
                0 => {
                    self.global_audio(host, sounds::TELEPORT_ALL_200, 0, 1)?;
                    self.force(host, s.start_anim, None)?;
                    self.gfx(host, s.start_gfx, 0)?;
                }
                4 => {
                    self.set_destination(host, true)?;
                    fire_random(host, entity)?;
                }
                5 => {
                    self.global_audio(host, sounds::TELEPORT_REVERSE_201, 0, 1)?;
                    self.force(host, s.end_anim, Some(Priority::High))?;
                    self.gfx(host, s.end_gfx, 0)?;
                    return Ok(true);
                }
                _ => {}
            },
        }
        self.advance();
        Ok(false)
    }
    /// `start()`: TeleportPulse.start, then Pulse.start (MINIGAME's override
    /// only calls super).
    pub fn start<H: Host<Entity = E>>(&self, host: &mut H) -> Result<()> {
        host.set_tele_pulse(&self.0.entity, self)?;
        pulses::start_base(&self.0.handle);
        Ok(())
    }
    /// `stop()`: TeleportPulse.stop (remove "tele-pulse", then Pulse.stop),
    /// then the type's own additions.
    pub fn stop<H: Host<Entity = E>>(&self, host: &mut H) -> Result<()> {
        let entity = &self.0.entity;
        host.remove_attribute(entity, "tele-pulse")?;
        pulses::stop_base(&self.0.handle);
        match self.0.kind {
            TeleportType::Normal
            | TeleportType::Christmas
            | TeleportType::Cabbage
            | TeleportType::EntranaMagicDoor => {
                host.unlock(entity)?;
                host.lock(entity, 4)?;
            }
            TeleportType::RcGuild => {
                host.unlock(entity)?;
                host.lock(entity, 5)?;
            }
            TeleportType::Ancient | TeleportType::Lunar => {
                self.force(host, -1, None)?;
                self.gfx(host, -1, 0)?;
                host.unlock(entity)?;
            }
            TeleportType::Teletabs => {
                self.force(host, -1, None)?;
                self.gfx(host, -1, 0)?;
                host.unlock(entity)?;
                host.lock(entity, 2)?;
            }
            TeleportType::Minigame => {
                self.force(host, -1, None)?;
                self.gfx(host, -1, 0)?;
            }
            TeleportType::Obelisk
            | TeleportType::TeleOther
            | TeleportType::FairyRing
            | TeleportType::PuroPuro
            | TeleportType::Ectophial
            | TeleportType::Instant => {}
        }
        Ok(())
    }
}

/// `TeleportManager.fireRandom(entity, location)`: its body reads the
/// teleporter's type and does nothing else.
pub fn fire_random<H: Host>(host: &mut H, entity: &H::Entity) -> Result<()> {
    if host.is_player(entity)? {
        let _ = host.teleporter(entity)?.teleport_type();
    }
    Ok(())
}

/// `Entity.isTeleporting()`.
pub fn is_teleporting<H: Host>(host: &mut H, entity: &H::Entity) -> Result<bool> {
    Ok(host.has_attribute(entity, "tele-pulse")? || host.teleport_location(entity)?.is_some())
}

struct ManagerFields<E> {
    entity: E,
    /// Declared by the original and never assigned.
    last_teleport: Option<TeleportPulse<E>>,
    current: Option<TeleportPulse<E>>,
    teleport_type: i32,
}
/// `TeleportManager`: one per entity, holding its entity as the original does.
pub struct Manager<E>(Shared<ManagerFields<E>>);
impl<E> Clone for Manager<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<E: Clone> Manager<E> {
    pub fn new(entity: E) -> Self {
        Self(Shared::new(ManagerFields {
            entity,
            last_teleport: None,
            current: None,
            teleport_type: 0,
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn entity(&self) -> E {
        self.0.read().entity.clone()
    }
    /// `getCurrentTeleport()`.
    pub fn current(&self) -> Option<TeleportPulse<E>> {
        self.0.read().current.clone()
    }
    pub fn last(&self) -> Option<TeleportPulse<E>> {
        self.0.read().last_teleport.clone()
    }
    pub fn teleport_type(&self) -> i32 {
        self.0.read().teleport_type
    }
    pub fn set_teleport_type(&self, teleport_type: i32) {
        self.0.write().teleport_type = teleport_type;
    }
    /// `send(location)`: a player's spell-book type, NORMAL otherwise, type 0.
    pub fn send_default<H: Host<Entity = E>>(
        &self,
        host: &mut H,
        location: Option<&Location>,
    ) -> Result<bool> {
        let entity = self.entity();
        let kind = if host.is_player(&entity)? {
            get_type(host.spell_book(&entity)?)
        } else {
            TeleportType::Normal
        };
        self.send(host, location, Some(kind), 0)
    }
    /// `send(location, type)`: type 0.
    pub fn send_type<H: Host<Entity = E>>(
        &self,
        host: &mut H,
        location: Option<&Location>,
        kind: Option<TeleportType>,
    ) -> Result<bool> {
        self.send(host, location, kind, 0)
    }
    /// `send(location, type, teleportType)`; teleportType is 0 spell, 1 item,
    /// 2 object, 3 NPC, -1 forced, or WILDY_TELEPORT.
    pub fn send<H: Host<Entity = E>>(
        &self,
        host: &mut H,
        location: Option<&Location>,
        kind: Option<TeleportType>,
        teleport_type: i32,
    ) -> Result<bool> {
        let entity = self.entity();
        if teleport_type == WILDY_TELEPORT || kind == Some(TeleportType::Obelisk) {
            if host.has_timer(&entity, "teleblock")? {
                return Ok(false);
            }
        } else {
            if !host.zone_teleport(&entity, teleport_type)? {
                return Ok(false);
            }
            if teleport_type != -1 && host.is_tele_blocked(&entity)? {
                if host.is_player(&entity)? {
                    host.send_message(&entity, TELE_BLOCKED)?;
                }
                return Ok(false);
            }
        }
        if teleport_type != -1 && host.is_player(&entity)? {
            let delay = host.world_ticks()?.wrapping_add(12);
            host.set_script_delay(&entity, delay)?;
            host.close_dialogue(&entity)?;
        }
        let mut kind = kind;
        if host.tablet_spell(&entity)? {
            kind = Some(TeleportType::Teletabs);
        }
        self.0.write().teleport_type = teleport_type;
        host.reset_walking(&entity)?;
        let pulse = kind.ok_or(Error::Null)?.pulse(host, &entity, location)?;
        self.0.write().current = Some(pulse);
        host.clear_pulses(&entity)?;
        host.lock(&entity, 12)?;
        host.set_disabled_ticks(&entity, if teleport_type == -1 { 5 } else { 12 })?;
        // The field is read again for the submission.
        let current = self.current().ok_or(Error::Null)?;
        host.submit(&current)?;
        if host.is_player(&entity)? {
            host.close_interfaces(&entity)?;
        }
        Ok(true)
    }
}
