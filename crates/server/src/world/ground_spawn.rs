//! GroundSpawn's native subtype and captured respawn pulse. Configuration file
//! loading and owning-world player/plugin/session services remain separate.
use super::{
    doors::Location,
    ground_item::{self as ground, Handle as Ground},
    ground_manager::{self as manager, Result},
};
use crate::{
    definitions::item::{Shared, WeakShared},
    inventory::item,
    pulses,
};
use std::collections::HashMap;

pub struct Spawn {
    ground: Ground,
    pub respawn_rate: i32,
}
impl Spawn {
    /// Internal graph edge; exposed views must use as_ground to retain subtype.
    pub(crate) fn base(&self) -> &Ground {
        &self.ground
    }
}
pub type Handle = Shared<Spawn>;

/// The subtype owns an unleased base; exposed ground and Item views retain the
/// subtype, and therefore its respawn state, without an internal reference cycle.
pub fn as_ground(spawn: &Handle) -> Ground {
    spawn.read().ground.keeping_from(spawn)
}
pub fn java_string(spawn: &Handle) -> String {
    let ground = spawn.read().ground.clone();
    let item = ground.read().item.clone();
    let name = item.read().name.clone().unwrap_or_else(|| "null".into());
    let rate = spawn.read().respawn_rate;
    let location = ground.read().location.clone();
    let location = location.map_or_else(
        || "null".into(),
        |l| {
            let [x, y, z] = l.coordinates();
            format!("[{x}, {y}, {z}]")
        },
    );
    format!("GroundSpawn [name={name}, respawnRate={rate}, loc={location}]")
}
pub fn set_respawn_rate(spawn: &Handle, minimum: i32, maximum: i32) {
    // Kotlin infix functions associate left-to-right: `min or max shl 16`.
    spawn.write().respawn_rate = (minimum | maximum).wrapping_shl(16);
}
pub trait Context: manager::Context {
    fn max_players(&mut self) -> Result<i32>;
    fn player_count(&mut self) -> Result<i32>;
    fn pulser(&mut self) -> Result<pulses::Runner>;
}
/// Preserve the original two double divisions, including zero population,
/// over-capacity populations and the JVM saturating/NaN double-to-int cast.
pub fn respawn_duration(ctx: &mut impl Context, spawn: &Handle) -> Result<i32> {
    let rate = spawn.read().respawn_rate;
    let minimum = rate & 0xffff;
    let maximum = (rate >> 16) & 0xffff;
    let max_players = ctx.max_players()?;
    let players = ctx.player_count()?;
    let ratio = max_players as f64 / players as f64;
    Ok((minimum as f64 + (maximum - minimum) as f64 / ratio) as i32)
}
pub fn init(
    ctx: &mut impl manager::Context,
    manager: &manager::Manager,
    spawn: &Handle,
) -> Result<Ground> {
    {
        let mut spawn = spawn.write();
        if spawn.respawn_rate >> 16 == 0 {
            spawn.respawn_rate |= spawn.respawn_rate.wrapping_shl(16);
        }
    }
    manager::create(ctx, manager, Some(as_ground(spawn)))
}
struct Respawn {
    base: pulses::Handle,
    spawn: Handle,
}
/// Weak metadata does not extend Java-visible lifetimes. Queue/exposed pulse
/// handles retain their captured spawn; ground lists/flags retain its subtype.
#[derive(Clone)]
pub struct Registry {
    spawns: Shared<HashMap<usize, WeakShared<Spawn>>>,
    tasks: Shared<HashMap<usize, WeakShared<Respawn>>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            spawns: Shared::new(HashMap::new()),
            tasks: Shared::new(HashMap::new()),
        }
    }
}
impl Registry {
    pub fn construct(
        &self,
        ctx: &mut impl ground::Context,
        respawn_rate: i32,
        input: &item::Handle,
        location: Option<Location>,
    ) -> ground::Result<Handle> {
        let mut spawn = None;
        ground::construct_with(ctx, input, location, 200, None, |ground| {
            let identity = ground.identity();
            let s = Shared::new(Spawn {
                ground,
                respawn_rate: 0,
            });
            self.spawns.write().insert(identity, s.downgrade());
            let view = as_ground(&s);
            spawn = Some(s);
            view
        })?;
        let spawn = spawn.expect("ground constructor registered the subtype");
        spawn.write().respawn_rate = respawn_rate;
        Ok(spawn)
    }
    pub fn get(&self, ground: &Ground) -> Option<Handle> {
        let spawn = self.spawns.read().get(&ground.identity()).cloned()?;
        let spawn = spawn.upgrade()?;
        let same = spawn.read().ground.same(ground);
        same.then_some(spawn)
    }
    /// Capture GameWorld.Pulser before computing the duration. Reentrant player
    /// services may replace the world's runner while the old receiver is held.
    pub fn respawn(&self, ctx: &mut impl Context, spawn: &Handle) -> Result<pulses::Handle> {
        let runner = ctx.pulser()?;
        let delay = respawn_duration(ctx, spawn)?;
        let base = Shared::new(pulses::Pulse::new(
            delay,
            None,
            pulses::Class::Other,
            "core.game.system.config.GroundSpawnLoader$GroundSpawn$respawn$1".into(),
        ));
        let capture = Shared::new(Respawn {
            base,
            spawn: spawn.clone(),
        });
        let base = capture.read().base.keeping_from(&capture);
        self.tasks
            .write()
            .insert(base.identity(), capture.downgrade());
        runner
            .submit(Some(base.clone()))
            .map_err(|error| manager::Error::Binding(format!("{error:?}")))?;
        Ok(base)
    }
    fn capture(&self, base: &pulses::Handle) -> Option<Shared<Respawn>> {
        let capture = self
            .tasks
            .read()
            .get(&base.identity())
            .cloned()?
            .upgrade()?;
        let same = capture.read().base.same(base);
        same.then_some(capture)
    }
    pub fn owns_pulse(&self, base: &pulses::Handle) -> bool {
        self.capture(base).is_some()
    }
    /// Dispatch this body from the world's existing global Runner, never from a
    /// separate spawn clock. The same GroundItem reference is created again.
    pub fn pulse(
        &self,
        ctx: &mut impl manager::Context,
        manager: &manager::Manager,
        base: &pulses::Handle,
    ) -> Result<bool> {
        let capture = self.capture(base).ok_or_else(|| {
            manager::Error::Binding("respawn pulse is not owned by this world".into())
        })?;
        let spawn = capture.read().spawn.clone();
        manager::create(ctx, manager, Some(as_ground(&spawn)))?;
        Ok(true)
    }
    pub fn collect(&self) {
        self.spawns.write().retain(|_, s| s.upgrade().is_some());
        self.tasks.write().retain(|_, t| t.upgrade().is_some());
    }
    pub fn retained_task_count(&self) -> usize {
        self.tasks.read().len()
    }
}
