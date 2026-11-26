//! The activity and cutscene framework (`core.game.activity`):
//! ActivityManager's registry and start ([`manager`]), ActivityPlugin and
//! CutscenePlugin as a MapZone subclass with their start/end pulses
//! ([`plugin`]) and the Kotlin Cutscene utility ([`cutscene`]).
//!
//! An activity plugin is a native [`zones::Zone`] whose behaviour is
//! [`plugin::Activity`]; its region is a native instance built by
//! `dynamic_region` (the host answers `DynamicRegion.create` through
//! `dynamic_region::create` on its scene) and its multiway toggles,
//! RegionZones and NPC lists are `dynamic_region`'s. The minimap packet is
//! `dynamic_scene::minimap_state` behind [`Host::minimap_state`]. Every other
//! system is a required [`Host`] service with no default; content activities
//! and cutscenes are [`plugin::Content`] and [`cutscene::Script`]
//! implementations. Pulses are native `pulses::Pulse` handles dispatched by
//! identity through [`Pulses`]; no lock is held across a host or content call.
pub mod cutscene;
pub mod manager;
pub mod plugin;

use super::{
    doors::{Handle as Scenery, Location},
    dynamic_region,
    player_walk::NodeHandle,
    region, zones,
};
use crate::definitions::item::{Shared, WeakShared};
use crate::player::attributes::AttributeValue;
use crate::pulses;
use std::sync::Arc;

/// The errors of the original: Java exceptions by kind, and failures of a
/// service (`Binding`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// java.lang.NullPointerException (also a Kotlin `!!`).
    Null,
    /// java.lang.ClassCastException (an attribute of another type).
    ClassCast,
    /// kotlin.UninitializedPropertyAccessException: a `lateinit` property
    /// (Cutscene's `region` or `base`) read before it was set.
    Uninitialized(&'static str),
    Index,
    ConcurrentModification,
    /// Another error of a native library the framework calls.
    Zone(zones::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

const LATEINIT: &str = "lateinit property has not been initialized: ";
impl From<zones::Error> for Error {
    fn from(error: zones::Error) -> Self {
        match error {
            zones::Error::Null => Self::Null,
            zones::Error::ClassCast => Self::ClassCast,
            zones::Error::ConcurrentModification => Self::ConcurrentModification,
            zones::Error::Binding(text) => Self::Binding(text),
            zones::Error::IllegalState(text) if text.starts_with(LATEINIT) => {
                match &text[LATEINIT.len()..] {
                    "base" => Self::Uninitialized("base"),
                    _ => Self::Uninitialized("region"),
                }
            }
            other => Self::Zone(other),
        }
    }
}
impl From<Error> for zones::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Null => Self::Null,
            Error::ClassCast => Self::ClassCast,
            Error::ConcurrentModification => Self::ConcurrentModification,
            Error::Uninitialized(name) => Self::IllegalState(format!("{LATEINIT}{name}")),
            Error::Index => Self::Binding("ArrayIndexOutOfBounds".into()),
            Error::Zone(inner) => inner,
            Error::Binding(text) => Self::Binding(text),
        }
    }
}
impl From<pulses::Error> for Error {
    fn from(error: pulses::Error) -> Self {
        match error {
            pulses::Error::Null => Self::Null,
            pulses::Error::Index => Self::Index,
            pulses::Error::ConcurrentModification => Self::ConcurrentModification,
            other => Self::Binding(format!("{other:?}")),
        }
    }
}
impl From<Error> for pulses::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Null => Self::Null,
            Error::Index => Self::Index,
            Error::ConcurrentModification => Self::ConcurrentModification,
            other => Self::Binding(format!("{other:?}")),
        }
    }
}

/// `core.tools.Log` levels the framework logs at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fine,
    Err,
}
impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fine => "FINE",
            Self::Err => "ERR",
        }
    }
}

/// `core.game.dialogue.FacialExpression`, by constant name; the dialogue
/// services take it untouched.
pub type Expression = &'static str;

/// Required services. None has a default; an owning world answers each from
/// the real system or fails. Reads that the original makes through plain
/// getters (`zones::Host::location`, `kind`, names, settings) never fail a
/// recorded call; the rest are the original's calls one for one.
#[allow(clippy::too_many_arguments)]
pub trait Host: zones::Host + dynamic_region::Host<Zones = Self> {
    // ------------------------------------------------------------ world
    /// `GameWorld.getSettings().isDevMode()`; `None` when the settings are
    /// null (the caller fails with Null).
    fn dev_mode(&mut self) -> Result<Option<bool>>;
    /// `ServerConstants.HOME_LOCATION` (null until the configuration sets it).
    fn home_location(&mut self) -> Result<Option<Location>>;
    /// `ServerConstants.LOG_CUTSCENE`.
    fn log_cutscene(&mut self) -> Result<bool>;
    /// `log(origin, level, message)`; `origin` is the class's binary name.
    fn log(&mut self, origin: &str, level: Level, message: &str) -> Result<()>;
    /// `e.printStackTrace()` of a caught failure.
    fn print_stack_trace(&mut self, error: &Error) -> Result<()>;
    /// `DynamicRegion.create(regionId)`: `dynamic_region::create` on the
    /// host's scene.
    fn create_instance(&mut self, region_id: i32) -> Result<region::Handle>;
    /// `DynamicRegion.create(ZoneBorders)`: `dynamic_region::create_area`.
    fn create_instance_area(&mut self, borders: &zones::Borders) -> Result<Vec<region::Handle>>;
    /// `RegionManager.getObject(location)`: the renderable dynamic object
    /// of any id and type at the tile (the region loaded first).
    fn scenery(&mut self, location: &Location) -> Result<Option<Scenery>>;
    /// `obj.getDefinition().getName()`.
    fn scenery_name(&mut self, object: &Scenery) -> Result<Option<String>>;

    // ------------------------------------------------------------ entities
    /// `entity.getUsername()` (players and NPCs alike).
    fn username(&mut self, entity: &NodeHandle) -> Result<String>;
    /// `Entity.setLocation(location)`.
    fn set_location(&mut self, entity: &NodeHandle, location: &Location) -> Result<()>;
    /// `entity.getAttribute(key, ...)`: the stored value, `None` when absent.
    fn get_attribute(&mut self, entity: &NodeHandle, key: &str) -> Result<Option<AttributeValue>>;
    fn set_attribute(
        &mut self,
        entity: &NodeHandle,
        key: &str,
        value: AttributeValue,
    ) -> Result<()>;
    fn remove_attribute(&mut self, entity: &NodeHandle, key: &str) -> Result<()>;
    /// `entity.lock()`.
    fn lock(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.unlock()`.
    fn unlock(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.getWalkingQueue().reset()`.
    fn walking_reset(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.getLocks().lockMovement(ticks)`.
    fn lock_movement(&mut self, entity: &NodeHandle, ticks: i32) -> Result<()>;
    /// `entity.getLocks().unlockMovement()`.
    fn unlock_movement(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `getProperties().setTeleportLocation(location)`.
    fn set_teleport_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> Result<()>;
    /// `getProperties().setSpawnLocation(location)`.
    fn set_spawn_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> Result<()>;
    /// `getProperties().setSafeZone(safe)`.
    fn set_safe_zone(&mut self, entity: &NodeHandle, safe: bool) -> Result<()>;
    /// The public `getProperties().safeRespawn` field (a plain write).
    fn set_safe_respawn(&mut self, entity: &NodeHandle, location: Option<&Location>) -> Result<()>;
    /// `entity.faceLocation(location)`.
    fn face_location(&mut self, entity: &NodeHandle, location: &Location) -> Result<()>;
    /// `entity.getPulseManager().run(MovementPulse(entity, destination,
    /// Pathfinder.SMART))` whose arrival body does nothing.
    fn run_movement(&mut self, entity: &NodeHandle, destination: &Location) -> Result<()>;
    /// `entity.hook(Event.SelfDeath, CUTSCENE_DEATH_HOOK)`.
    fn hook_self_death(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.unhook(CUTSCENE_DEATH_HOOK)`.
    fn unhook_self_death(&mut self, entity: &NodeHandle) -> Result<()>;

    // ------------------------------------------------------------ players
    /// `getInterfaceManager().removeTabs(tabs...)`.
    fn remove_tabs(&mut self, player: &NodeHandle, tabs: &[i32]) -> Result<()>;
    fn restore_tabs(&mut self, player: &NodeHandle) -> Result<()>;
    fn open_default_tabs(&mut self, player: &NodeHandle) -> Result<()>;
    /// `getInterfaceManager().close()`.
    fn close_interfaces(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `getInterfaceManager().openOverlay(new Component(id))`.
    fn open_overlay(&mut self, player: &NodeHandle, component: i32) -> Result<()>;
    fn close_overlay(&mut self, player: &NodeHandle) -> Result<()>;
    /// `PacketRepository.send(MinimapState.class, new
    /// MinimapStateContext(player, state))`: `dynamic_scene::minimap_state`;
    /// PacketRepository logs a failing write itself.
    fn minimap_state(&mut self, player: &NodeHandle, state: i32) -> Result<()>;
    /// `player.getSession().isActive()`.
    fn session_active(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `player.logoutListeners[key] = listener`; the listener runs
    /// [`cutscene::logout_listener`].
    fn put_logout_listener(
        &mut self,
        player: &NodeHandle,
        key: &str,
        cutscene: cutscene::Cutscene<Self>,
    ) -> Result<()>;
    fn remove_logout_listener(&mut self, player: &NodeHandle, key: &str) -> Result<()>;
    /// `player.dialogueInterpreter.addAction { _, _ -> ... }`; the host runs
    /// [`cutscene::run_continue`] with the value when the dialogue continues.
    fn add_dialogue_action(
        &mut self,
        player: &NodeHandle,
        action: cutscene::Continue<Self>,
    ) -> Result<()>;
    /// ContentAPI `sendNPCDialogue(player, npc, msg, expr, hide)`.
    fn send_npc_dialogue(
        &mut self,
        player: &NodeHandle,
        npc: i32,
        message: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()>;
    /// ContentAPI `sendNPCDialogueLines(player, npc, expr, hideContinue, msgs...)`.
    fn send_npc_dialogue_lines(
        &mut self,
        player: &NodeHandle,
        npc: i32,
        expression: Expression,
        hide: bool,
        lines: &[String],
    ) -> Result<()>;
    /// ContentAPI `closeDialogue(player)`.
    fn close_dialogue(&mut self, player: &NodeHandle) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, player: &NodeHandle, message: &str) -> Result<()>;
    /// ContentAPI `sendDialogueLines(player, messages...)`.
    fn send_dialogue_lines(&mut self, player: &NodeHandle, lines: &[String]) -> Result<()>;
    /// ContentAPI `sendPlayerDialogue(player, msg, expr, hide)`.
    fn send_player_dialogue(
        &mut self,
        player: &NodeHandle,
        message: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()>;
    /// `AntiMacro.pause(player)` (`timers::content::antimacro::pause`).
    fn anti_macro_pause(&mut self, player: &NodeHandle) -> Result<()>;
    fn anti_macro_unpause(&mut self, player: &NodeHandle) -> Result<()>;
    /// `PlayerCamera(player)` (core.api.utils, a ContentAPI port).
    fn camera_new(&mut self, player: &NodeHandle) -> Result<()>;
    fn camera_rotate_to(
        &mut self,
        player: &NodeHandle,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()>;
    fn camera_rotate_by(
        &mut self,
        player: &NodeHandle,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()>;
    fn camera_pan_to(
        &mut self,
        player: &NodeHandle,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()>;
    fn camera_shake(
        &mut self,
        player: &NodeHandle,
        kind: i32,
        jitter: i32,
        amplitude: i32,
        frequency: i32,
        speed: i32,
    ) -> Result<()>;
    fn camera_reset(&mut self, player: &NodeHandle) -> Result<()>;

    // ------------------------------------------------------------ NPCs
    /// `new NPC(id)`.
    fn npc_new(&mut self, id: i32) -> Result<NodeHandle>;
    fn npc_set_respawn(&mut self, npc: &NodeHandle, respawn: bool) -> Result<()>;
    fn npc_set_aggressive(&mut self, npc: &NodeHandle, aggressive: bool) -> Result<()>;
    fn npc_set_walks(&mut self, npc: &NodeHandle, walks: bool) -> Result<()>;
    /// `npc.init()`.
    fn npc_init(&mut self, npc: &NodeHandle) -> Result<()>;

    // ------------------------------------------------------------ framework state
    /// The world's ActivityManager.
    fn activities(&mut self) -> &mut manager::Manager<Self>;
    /// The world's activity pulses (dispatched by identity).
    fn activity_pulses(&mut self) -> &mut Pulses<Self>;
}

/// `entity instanceof Player`.
pub(crate) fn is_player<H: Host>(host: &mut H, entity: &NodeHandle) -> Result<bool> {
    Ok(host.kind(entity)? == zones::EntityKind::Player)
}
/// `entity.getLocation()`.
pub(crate) fn location_of<H: Host>(host: &mut H, entity: &NodeHandle) -> Result<Location> {
    zones::Host::location(host, entity)?.ok_or(Error::Null)
}
/// `Location.transform(dx, dy, dz)`: `(z + dz) % 4`, normalised by the
/// constructor.
pub(crate) fn transform(l: &Location, dx: i32, dy: i32, dz: i32) -> Location {
    let [x, y, z] = l.coordinates();
    Location::new(
        x.wrapping_add(dx),
        y.wrapping_add(dy),
        z.wrapping_add(dz) % 4,
    )
}
/// A copy of a location value (Java Locations are values to the framework).
pub(crate) fn copy(l: &Location) -> Location {
    let [x, y, z] = l.coordinates();
    Location::new(x, y, z)
}
// ---------------------------------------------------------------- pulses

/// What an activity pulse runs: the class the original's object belongs to
/// and the state its fields hold.
pub enum Body<H: Host> {
    /// CutscenePlugin.StartPulse (the counter lives on the plugin).
    Start(Arc<plugin::Core<H>>),
    /// CutscenePlugin.EndPulse: the plugin's own (`counter` on the plugin)
    /// or one made by `getEndPulse()` (its counter here).
    End(Arc<plugin::Core<H>>, Option<i32>),
    /// The anonymous pulse of Cutscene.timedUpdate(ticks, newStage).
    Timed(cutscene::Cutscene<H>, i32),
    /// The anonymous pulses of Cutscene.end(fade, endActions) and
    /// endWithoutFade(endActions), with their `tick` field.
    End2 {
        cutscene: cutscene::Cutscene<H>,
        fade: Option<bool>,
        tick: i32,
        actions: Option<cutscene::EndActions<H>>,
    },
}
impl<H: Host> Body<H> {
    /// The original object's class.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Start(_) => "core.game.activity.CutscenePlugin$StartPulse",
            Self::End(..) => "core.game.activity.CutscenePlugin$EndPulse",
            Self::Timed(..) => "core.game.activity.Cutscene$timedUpdate$1",
            Self::End2 { fade: None, .. } => "core.game.activity.Cutscene$endWithoutFade$1",
            Self::End2 { .. } => "core.game.activity.Cutscene$end$1",
        }
    }
}

/// The world's activity pulses: each entry is a pulse object and the body
/// its `pulse()` and `stop()` run. Entries hold the pulse weakly; a body
/// holds its owner strongly, as the Java object's outer reference does.
pub struct Pulses<H: Host> {
    entries: Vec<(WeakShared<pulses::Pulse>, Shared<Body<H>>)>,
}
impl<H: Host> Default for Pulses<H> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
impl<H: Host> Pulses<H> {
    pub fn new() -> Self {
        Self::default()
    }
    /// Constructs a pulse object of `body` (`new Pulse(delay, checks)`).
    pub(crate) fn create(
        &mut self,
        delay: i32,
        checks: Option<Vec<Option<pulses::Node>>>,
        body: Body<H>,
    ) -> pulses::Handle {
        let pulse = Shared::new(pulses::Pulse::new(
            delay,
            checks.map(Shared::new),
            pulses::Class::Other,
            body.class_name().into(),
        ));
        self.entries.push((pulse.downgrade(), Shared::new(body)));
        pulse
    }
    /// The body of an activity pulse, if `pulse` is one.
    pub fn body(&self, pulse: &pulses::Handle) -> Option<Shared<Body<H>>> {
        self.entries
            .iter()
            .find(|(p, _)| p.upgrade().is_some_and(|p| p.same(pulse)))
            .map(|(_, b)| b.clone())
    }
    /// The owner the body's outer reference points at (a plugin core or a
    /// cutscene), for diagnostics and the replay's labels only.
    pub fn owner(&self, pulse: &pulses::Handle) -> Option<Owner<H>> {
        let body = self.body(pulse)?;
        let body = body.read();
        Some(match &*body {
            Body::Start(core) | Body::End(core, _) => Owner::Plugin(core.clone()),
            Body::Timed(c, _) | Body::End2 { cutscene: c, .. } => Owner::Cutscene(c.clone()),
        })
    }
    /// Drops the entries Java would collect: a pulse nothing holds any more,
    /// and the plugin pulses of a plugin core held by nothing but its own
    /// pulse entries while none of those pulses is held elsewhere.
    pub fn sweep(&mut self) {
        self.entries.retain(|(p, _)| p.upgrade().is_some());
        let mut index = 0;
        while index < self.entries.len() {
            let core = match &*self.entries[index].1.read() {
                Body::Start(core) | Body::End(core, _) => Some(core.clone()),
                _ => None,
            };
            let Some(core) = core else {
                index += 1;
                continue;
            };
            let own: Vec<usize> = (0..self.entries.len())
                .filter(|&i| match &*self.entries[i].1.read() {
                    Body::Start(c) | Body::End(c, _) => Arc::ptr_eq(c, &core),
                    _ => false,
                })
                .collect();
            // The temporary `core` above, the bodies and the core's own Arc
            // from its zone (absent once the zone is gone).
            let unreachable = Arc::strong_count(&core) == own.len() + 1
                && own.iter().all(|&i| {
                    let fields = core.held_pulses();
                    self.entries[i].0.upgrade().is_some_and(|p| {
                        let held_by_core = usize::from(fields.iter().any(|f| f.same(&p)));
                        // `p` itself, the core's field when it is one.
                        p.strong_count() == 1 + held_by_core
                    })
                });
            if unreachable {
                let mut position = 0;
                self.entries.retain(|_| {
                    let keep = !own.contains(&position);
                    position += 1;
                    keep
                });
                index = 0;
            } else {
                index += 1;
            }
        }
    }
}

/// The outer object of an activity pulse.
pub enum Owner<H: Host> {
    Plugin(Arc<plugin::Core<H>>),
    Cutscene(cutscene::Cutscene<H>),
}

/// An activity pulse's `pulse()`, by identity: `None` when `pulse` is not
/// one of the framework's.
pub fn run_pulse<H: Host>(host: &mut H, pulse: &pulses::Handle) -> Option<Result<bool>> {
    let body = host.activity_pulses().body(pulse)?;
    Some(run_body(host, pulse, &body))
}
fn run_body<H: Host>(host: &mut H, pulse: &pulses::Handle, body: &Shared<Body<H>>) -> Result<bool> {
    enum Step<H: Host> {
        Start(Arc<plugin::Core<H>>),
        End(Arc<plugin::Core<H>>, bool),
        Timed(cutscene::Cutscene<H>, i32),
        End2(cutscene::Cutscene<H>, Option<bool>, i32),
    }
    let step = {
        let mut b = body.write();
        match &mut *b {
            Body::Start(core) => Step::Start(core.clone()),
            Body::End(core, None) => Step::End(core.clone(), true),
            Body::End(core, Some(_)) => Step::End(core.clone(), false),
            Body::Timed(c, stage) => Step::Timed(c.clone(), *stage),
            Body::End2 {
                cutscene,
                fade,
                tick,
                ..
            } => {
                let t = *tick;
                *tick = tick.wrapping_add(1);
                Step::End2(cutscene.clone(), *fade, t)
            }
        }
    };
    match step {
        Step::Start(core) => plugin::start_pulse(host, &core),
        Step::End(core, own) => {
            let counter = if own {
                core.next_end_counter()
            } else {
                let mut b = body.write();
                match &mut *b {
                    Body::End(_, Some(counter)) => {
                        let c = *counter;
                        *counter = counter.wrapping_add(1);
                        c
                    }
                    _ => return Err(Error::Binding("end pulse body changed".into())),
                }
            };
            plugin::end_pulse(host, &core, pulse, counter)
        }
        Step::Timed(c, stage) => cutscene::timed_pulse(host, &c, stage),
        Step::End2(c, fade, tick) => cutscene::end_pulse(host, &c, fade, tick),
    }
}
/// An activity pulse's `stop()`: Pulse.stop, then Cutscene's end pulses'
/// override. `None` when `pulse` is not one of the framework's.
pub fn stop_pulse<H: Host>(host: &mut H, pulse: &pulses::Handle) -> Option<Result<()>> {
    let body = host.activity_pulses().body(pulse)?;
    let (end, class) = {
        let b = body.read();
        let class = b.class_name();
        match &*b {
            Body::End2 { cutscene, .. } => (Some(cutscene.clone()), class),
            _ => (None, class),
        }
    };
    pulses::stop_base(pulse);
    Some(match end {
        Some(c) => {
            let actions = match &*body.read() {
                Body::End2 { actions, .. } => actions.clone(),
                _ => None,
            };
            cutscene::end_stop(host, &c, class, actions.as_ref())
        }
        None => Ok(()),
    })
}
