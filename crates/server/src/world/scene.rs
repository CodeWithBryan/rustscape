//! Native region/scenery simulation. Networking, entity movement and content
//! services are supplied explicitly; this is not the server's world-entry path.
use super::{
    chunk, chunk_sync, clipping,
    doors::{self, Entity, Handle, Location},
    landscape, mapscape, region, region_queries,
    scenery::{self, ObjectUpdate, SceneryPulse},
    scenery_node,
};
use crate::{
    definitions::{
        item::{Shared, WeakShared},
        scenery as definitions,
    },
    pulses,
};
use rs09_cache::{CacheError, archive::Archive};
use rs09_protocol::{
    buffer::Reader,
    game::{self, Frame},
};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT_CHUNK_BINDING: AtomicUsize = AtomicUsize::new(1);
#[derive(Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
fn error(e: impl std::fmt::Debug) -> Error {
    Error(format!("{e:?}"))
}
pub trait Assets: Send + Sync {
    fn archive_id(&self, name: &str) -> Result<i32>;
    fn mapscape(&self, group: i32) -> Result<Option<Vec<u8>>>;
    fn landscape(&self, group: i32, key: [u32; 4]) -> Result<Option<Vec<u8>>>;
    fn keys(&self, region: i32) -> Result<[u32; 4]>;
}
pub struct CacheAssets {
    pub archive: Archive,
    pub keys: HashMap<i32, [u32; 4]>,
}
impl Assets for CacheAssets {
    fn archive_id(&self, name: &str) -> Result<i32> {
        Ok(self
            .archive
            .index
            .group_by_ascii_name(name)
            .map_or(-1, |n| n as i32))
    }
    fn mapscape(&self, group: i32) -> Result<Option<Vec<u8>>> {
        Ok(self
            .archive
            .file(group as u32, 0, [0; 4])
            .map_err(error)?
            .map(|d| d.as_ref().clone()))
    }
    fn landscape(&self, group: i32, key: [u32; 4]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .archive
            .file(group as u32, 0, key)
            .map_err(error)?
            .map(|d| d.as_ref().clone()))
    }
    fn keys(&self, region: i32) -> Result<[u32; 4]> {
        Ok(self.keys.get(&region).copied().unwrap_or([0; 4]))
    }
}
/// Services may reenter the scene through their mutable scene argument. No scene
/// or object lock is held while invoking these callbacks.
pub trait Services<A: Assets> {
    fn is_player(&self, entity: Entity) -> bool;
    fn location(&mut self, scene: &mut Scene<A>, entity: Entity) -> Result<Location>;
    fn lock(&mut self, scene: &mut Scene<A>, entity: Entity, ticks: i32) -> Result<()>;
    fn audio(
        &mut self,
        scene: &mut Scene<A>,
        entity: Entity,
        id: i32,
        delay: Option<i32>,
    ) -> Result<()>;
    fn message(&mut self, scene: &mut Scene<A>, entity: Entity, text: &str) -> Result<()>;
    fn logout_listener(
        &mut self,
        scene: &mut Scene<A>,
        entity: Entity,
        location: Option<Location>,
    ) -> Result<()>;
    fn reset_path(&mut self, scene: &mut Scene<A>, entity: Entity) -> Result<()>;
    fn add_path(&mut self, scene: &mut Scene<A>, entity: Entity, x: i32, y: i32) -> Result<()>;
    fn move_near(
        &mut self,
        scene: &mut Scene<A>,
        entity: Entity,
        location: Location,
    ) -> Result<bool>;
    fn diary(
        &mut self,
        scene: &mut Scene<A>,
        entity: Entity,
        diary: doors::Diary,
        level: i32,
        task: i32,
    ) -> Result<()>;
    fn ground_create(&mut self, scene: &mut Scene<A>, item: Option<usize>) -> Result<()>;
    fn pulse(&mut self, scene: &mut Scene<A>, pulse: &pulses::Handle) -> Result<bool>;
    fn stop(&mut self, scene: &mut Scene<A>, pulse: &pulses::Handle) -> Result<()>;
    fn now_millis(&mut self) -> i64;
}
enum WeakAction {
    Client(WeakShared<doors::Scenery>, WeakShared<doors::Scenery>),
    Restore(WeakShared<doors::Scenery>),
    Expire(WeakShared<doors::Scenery>, Option<scenery::GroundItems>),
    Respawn(WeakShared<doors::Scenery>),
}
impl WeakAction {
    fn new(a: &scenery::Action) -> Self {
        match a {
            scenery::Action::ClientRestore { remove, construct } => {
                Self::Client(remove.downgrade(), construct.downgrade())
            }
            scenery::Action::Restore { object } => Self::Restore(object.downgrade()),
            scenery::Action::Expire { object, items } => {
                Self::Expire(object.downgrade(), items.clone())
            }
            scenery::Action::Respawn { object } => Self::Respawn(object.downgrade()),
        }
    }
    fn get(&self) -> Result<scenery::Action> {
        let upgrade = |h: &WeakShared<doors::Scenery>| {
            h.upgrade()
                .ok_or_else(|| Error("expired scenery pulse capture".into()))
        };
        Ok(match self {
            Self::Client(a, b) => scenery::Action::ClientRestore {
                remove: upgrade(a)?,
                construct: upgrade(b)?,
            },
            Self::Restore(a) => scenery::Action::Restore {
                object: upgrade(a)?,
            },
            Self::Expire(a, items) => scenery::Action::Expire {
                object: upgrade(a)?,
                items: items.clone(),
            },
            Self::Respawn(a) => scenery::Action::Respawn {
                object: upgrade(a)?,
            },
        })
    }
}
enum Task {
    Scenery {
        base: WeakShared<pulses::Pulse>,
        action: WeakAction,
        check: Option<WeakShared<doors::Scenery>>,
    },
    Door {
        base: WeakShared<pulses::Pulse>,
        action: Shared<doors::Action>,
    },
}
impl Task {
    fn base(&self) -> Option<pulses::Handle> {
        match self {
            Self::Scenery { base, .. } | Self::Door { base, .. } => base.upgrade(),
        }
    }
}
pub struct Scene<A: Assets> {
    pub assets: A,
    pub definitions: definitions::Registry,
    pub doors: doors::Registry,
    pub regions: HashMap<i32, region::Handle>,
    pub flags: HashMap<(i32, bool), clipping::Flags>,
    pub updates: HashMap<i32, Vec<ObjectUpdate>>,
    pub diagnostics: Vec<String>,
    pub runner: pulses::Runner,
    tasks: HashMap<usize, Task>,
    checks: HashMap<usize, WeakShared<doors::Scenery>>,
    chunks: HashMap<i32, chunk::HandleChunk>,
    chunk_bindings: HashMap<usize, WeakShared<chunk::RegionChunk>>,
    chunk_ids: HashMap<usize, usize>,
    pub dirty_chunks: chunk_sync::DirtyTracker,
    pub pulse_counts: HashMap<String, u64>,
}
impl<A: Assets> Scene<A> {
    pub fn new(assets: A, definitions: definitions::Registry, doors: doors::Registry) -> Self {
        Self {
            assets,
            definitions,
            doors,
            regions: HashMap::new(),
            flags: HashMap::new(),
            updates: HashMap::new(),
            diagnostics: vec![],
            runner: pulses::Runner::default(),
            tasks: HashMap::new(),
            checks: HashMap::new(),
            chunks: HashMap::new(),
            chunk_bindings: HashMap::new(),
            chunk_ids: HashMap::new(),
            dirty_chunks: chunk_sync::DirtyTracker::default(),
            pulse_counts: HashMap::new(),
        }
    }
    pub fn region(&mut self, id: i32) -> region::Handle {
        self.regions
            .entry(id)
            .or_insert_with(|| Shared::new(region::Region::new((id >> 8) & 255, id & 255)))
            .clone()
    }
    pub fn chunk(&mut self, l: &Location) -> Result<chunk::HandleChunk> {
        let [x, y, z] = l.coordinates();
        let id = ((x >> 6) << 8) | (y >> 6);
        let r = self.region(id);
        let chunks = r
            .read()
            .chunks
            .clone()
            .ok_or_else(|| Error("null region chunks".into()))?;
        let c = chunks.read()[((x & 63) >> 3) as usize][((y & 63) >> 3) as usize][z as usize]
            .clone()
            .ok_or_else(|| Error("null chunk".into()))?;
        let key = c.read().flags.key();
        self.chunks.insert(key, c.clone());
        let identity = c.identity();
        let registered = self
            .chunk_ids
            .get(&identity)
            .and_then(|id| self.chunk_bindings.get(id))
            .and_then(WeakShared::upgrade)
            .is_some_and(|old| old.same(&c));
        if !registered {
            let token = NEXT_CHUNK_BINDING
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| Error("chunk binding identity exhausted".into()))?;
            self.chunk_ids.insert(identity, token);
            self.chunk_bindings.insert(token, c.downgrade());
        }
        Ok(c)
    }
    /// Opaque world binding; tokens never alias another scene or a later chunk.
    pub fn scenery_chunk(&mut self, l: &Location) -> Result<usize> {
        let c = self.chunk(l)?;
        Ok(self.chunk_ids[&c.identity()])
    }
    pub fn flag_token(&mut self, token: usize, update: ObjectUpdate) -> Result<()> {
        let c = self
            .chunk_bindings
            .get(&token)
            .and_then(WeakShared::upgrade)
            .ok_or_else(|| Error("expired scenery update chunk".into()))?;
        self.flag_object(&c, update)
    }
    fn prune_chunk_bindings(&mut self) {
        self.chunk_bindings.retain(|_, h| h.upgrade().is_some());
        self.chunk_ids
            .retain(|_, token| self.chunk_bindings.contains_key(token));
    }
    /// Queue on the exact registered chunk, including an externally retained
    /// old chunk after its coordinate slot has been replaced. Diagnostics keep
    /// the legacy coordinate-keyed observations; per-viewer queues use identity.
    pub fn flag_object(&mut self, chunk: &chunk::HandleChunk, update: ObjectUpdate) -> Result<()> {
        if !self
            .chunk_bindings
            .get(&self.chunk_ids.get(&chunk.identity()).copied().unwrap_or(0))
            .and_then(WeakShared::upgrade)
            .is_some_and(|c| c.same(chunk))
        {
            return Err(Error("unregistered scenery update chunk".into()));
        }
        let key = chunk.read().flags.key();
        chunk_sync::flag(
            chunk,
            Some(Shared::new(chunk_sync::Flag::Object(update.clone()))),
            &mut |c| {
                self.dirty_chunks.on_flagged(c);
                Ok(())
            },
        )
        .map_err(error)?;
        self.updates.entry(key).or_default().push(update);
        Ok(())
    }
    pub fn context<'a, S: Services<A>>(&'a mut self, services: &'a mut S) -> World<'a, A, S> {
        World {
            scene: self,
            services,
        }
    }
    /// Retain the pulse's captured objects when exposing a stored restore handle.
    pub fn restore_pulse(&self, h: &Handle) -> Result<Option<pulses::Handle>> {
        let Some(p) = h.read().restore_pulse.clone() else {
            return Ok(None);
        };
        if let Some(Task::Scenery { action, check, .. }) = self.tasks.get(&p.identity()) {
            let capture = Shared::new(SceneryPulse {
                pulse: p.clone(),
                action: action.get()?,
                check: check.as_ref().and_then(WeakShared::upgrade),
            });
            Ok(Some(p.keeping_from(&capture)))
        } else {
            Ok(Some(p))
        }
    }
    /// Snapshots the original chunk update list; queue clearing remains a separate
    /// world synchronization phase so several players can receive the same flags.
    pub fn update_frame(&self, key: i32, last_scene: &Location) -> Result<Option<Frame>> {
        let Some(updates) = self.updates.get(&key).filter(|v| !v.is_empty()) else {
            return Ok(None);
        };
        let c = self
            .chunks
            .get(&key)
            .ok_or_else(|| Error("unknown update chunk".into()))?;
        let [x, y, _] = c.read().current_base.coordinates();
        let [sx, sy, _] = last_scene.coordinates();
        let frames: Vec<_> = updates.clone().iter().map(ObjectUpdate::frame).collect();
        game::updates::zone_enclosed(
            x.wrapping_sub(((sx >> 3).wrapping_sub(6)).wrapping_shl(3)),
            y.wrapping_sub(((sy >> 3).wrapping_sub(6)).wrapping_shl(3)),
            &frames,
        )
        .map(Some)
        .map_err(error)
    }
    pub fn clear_updates(&mut self) {
        self.dirty_chunks.reset_all();
        self.updates.clear();
        self.prune_chunk_bindings();
    }
    pub fn retained_task_count(&self) -> usize {
        self.tasks.len()
    }
    pub fn owns_pulse(&self, p: &pulses::Handle) -> bool {
        self.tasks
            .get(&p.identity())
            .and_then(Task::base)
            .is_some_and(|base| base.same(p))
    }
    /// Reap expired task/check registrations after the single global pulse
    /// phase. This does not tick the queue a second time or clear viewer updates.
    pub fn collect_pulses(&mut self) {
        self.tasks.retain(|_, t| t.base().is_some());
        self.checks.retain(|_, h| h.upgrade().is_some());
        self.prune_chunk_bindings();
    }
}
pub struct World<'a, A: Assets, S: Services<A>> {
    pub scene: &'a mut Scene<A>,
    pub services: &'a mut S,
}
impl<A: Assets, S: Services<A>> super::pathfinding::Context for World<'_, A, S> {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> super::pathfinding::Result<i32> {
        World::clipping(self, Location::new(x, y, z), false)
            .map_err(|e| super::pathfinding::Error::Binding(e.0))
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> super::pathfinding::Result<i32> {
        <Self as super::pathfinding::Context>::clipping(self, z, x, y)
    }
}
impl<A: Assets, S: Services<A>> super::pathfinding::projectile::Context for World<'_, A, S> {
    fn flag(
        &mut self,
        z: i32,
        x: i32,
        y: i32,
        projectile: bool,
    ) -> super::pathfinding::Result<i32> {
        World::clipping(self, Location::new(x, y, z), projectile)
            .map_err(|e| super::pathfinding::Error::Binding(e.0))
    }
}
impl<A: Assets, S: Services<A>> World<'_, A, S> {
    pub fn load(&mut self, id: i32, build: Option<bool>) -> Result<region::Handle> {
        let r = self.scene.region(id);
        region::load(self, &r, build).map_err(error)?;
        Ok(r)
    }
    pub fn object(&mut self, l: Location, id: i32, shape: i32) -> Result<Option<Handle>> {
        let [x, y, _] = l.coordinates();
        let r = self.scene.region(((x >> 6) << 8) | (y >> 6));
        region::load(self, &r, None).map_err(error)?;
        let c = self.scene.chunk(&l)?;
        let i = chunk::get_index(&c, x & 7, y & 7, id, shape).map_err(error)?;
        if i < 0 {
            return Ok(None);
        }
        let h = c.read().objects.read()[(x & 7) as usize][(y & 7) as usize][i as usize].clone();
        Ok(h.filter(|h| h.read().renderable))
    }
    pub fn clipping(&mut self, l: Location, projectile: bool) -> Result<i32> {
        region_queries::clipping_flag(self, Some(&l), projectile).map_err(error)
    }
    pub fn is_landscape(&mut self, l: Option<&Location>) -> Result<bool> {
        region_queries::is_landscape(self, l).map_err(error)
    }
    pub fn teleport_permitted(&mut self, l: Option<&Location>) -> Result<bool> {
        region_queries::teleport_permitted(self, l).map_err(error)
    }
    pub fn is_clipped(&mut self, l: Option<&Location>) -> Result<bool> {
        region_queries::is_clipped(self, l).map_err(error)
    }
    pub fn water_clipping(&mut self, l: Option<&Location>) -> Result<i32> {
        region_queries::water_flag(self, l).map_err(error)
    }
    pub fn door(&mut self, entity: Entity, h: &Handle) -> Result<()> {
        doors::handle(self, entity, h).map_err(error)
    }
    pub fn tick(&mut self) -> Result<()> {
        let runner = self.scene.runner.clone();
        runner.update_all(&mut (), self).map_err(error)?;
        self.scene.collect_pulses();
        Ok(())
    }
    pub fn pulse_body(&mut self, p: &pulses::Handle) -> pulses::Result<bool> {
        if !self.scene.owns_pulse(p) {
            return Err(pulses::Error::Binding(
                "pulse is not owned by this scene".into(),
            ));
        }
        self.body(p).map_err(|e| pulses::Error::Binding(e.0))
    }
    pub fn pulse_inactive(&mut self, p: &pulses::Handle) -> pulses::Result<bool> {
        if !self.scene.owns_pulse(p) {
            return Err(pulses::Error::Binding(
                "checks are not owned by this scene".into(),
            ));
        }
        pulses::has_inactive_base(p, &mut (), self)
    }
    pub fn stop_pulse(&mut self, p: &pulses::Handle) -> pulses::Result<()> {
        if !self.scene.owns_pulse(p) {
            return Err(pulses::Error::Binding(
                "stop is not owned by this scene".into(),
            ));
        }
        pulses::stop_base(p);
        Ok(())
    }
    fn body(&mut self, p: &pulses::Handle) -> Result<bool> {
        enum Body {
            Scenery(SceneryPulse),
            Door(doors::DoorPulse),
            External,
        }
        let body = match self.scene.tasks.get(&p.identity()) {
            Some(Task::Scenery { action, check, .. }) => Body::Scenery(SceneryPulse {
                pulse: p.clone(),
                action: action.get()?,
                check: check.as_ref().and_then(WeakShared::upgrade),
            }),
            Some(Task::Door { action, .. }) => Body::Door(doors::DoorPulse {
                pulse: p.clone(),
                action: action.clone(),
            }),
            None => Body::External,
        };
        match body {
            Body::Scenery(p) => scenery::pulse_body(self, &p).map_err(error),
            Body::Door(p) => doors::pulse_body(self, &p).map_err(error),
            Body::External => self.services.pulse(self.scene, p),
        }
    }
}
impl<A: Assets, S: Services<A>> region_queries::Context for World<'_, A, S> {
    fn region(&mut self, location: &Location) -> region_queries::Result<region::Handle> {
        let [x, y, _] = location.coordinates();
        Ok(self.scene.region(((x >> 6) << 8) | (y >> 6)))
    }
    fn load(&mut self, region: &region::Handle) -> region_queries::Result<()> {
        region::load(self, region, None)
            .map_err(|e| region_queries::Error::Binding(format!("{e:?}")))
    }
    fn chunk(&mut self, location: &Location) -> region_queries::Result<chunk::HandleChunk> {
        self.scene
            .chunk(location)
            .map_err(|e| region_queries::Error::Binding(e.0))
    }
    fn flags(&mut self, key: i32, projectile: bool) -> region_queries::Result<clipping::Flags> {
        clipping::Context::flags(self, key, projectile)
            .map_err(|e| region_queries::Error::Binding(format!("{e:?}")))
    }
}
impl<A: Assets, S: Services<A>> clipping::Context for World<'_, A, S> {
    fn flags(&mut self, key: i32, p: bool) -> clipping::Result<clipping::Flags> {
        Ok(self
            .scene
            .flags
            .entry((key, p))
            .or_insert_with(|| Shared::new([if p { 0 } else { -1 }; 64]))
            .clone())
    }
    fn border(&mut self, b: clipping::Border) -> clipping::Result<()> {
        let l = Location::new(b.x, b.y, b.z);
        let [x, y, _] = l.coordinates();
        let r = self.scene.region(((x >> 6) << 8) | (y >> 6));
        region::load(self, &r, None).map_err(|e| clipping::Error::Binding(format!("{e:?}")))?;
        if !r.read().has_flags {
            return Ok(());
        }
        let c = self
            .scene
            .chunk(&l)
            .map_err(|e| clipping::Error::Binding(e.0))?;
        let flags = if b.projectile {
            c.read().projectile_flags.clone()
        } else {
            c.read().flags.clone()
        };
        flags.flag(self, x & 7, y & 7, b.mask, b.add)
    }
}
impl<A: Assets, S: Services<A>> scenery::Context for World<'_, A, S> {
    fn construct(&mut self, id: i32, l: Location, shape: i32, rot: i32) -> scenery::Result<Handle> {
        scenery_node::construct(&mut self.scene.definitions, id, l, shape, rot, false)
            .map_err(|e| scenery::Error::Binding(format!("{e:?}")))
    }
    fn remove_landscape(&mut self, h: &Handle) -> scenery::Result<Option<Handle>> {
        landscape::remove(self, h).map_err(|e| scenery::Error::Binding(format!("{e:?}")))
    }
    fn add_landscape(&mut self, h: &Handle) -> scenery::Result<()> {
        landscape::add(self, h, false).map_err(|e| scenery::Error::Binding(format!("{e:?}")))
    }
    fn chunk(&mut self, l: Location) -> scenery::Result<usize> {
        self.scene
            .scenery_chunk(&l)
            .map_err(|e| scenery::Error::Binding(e.0))
    }
    fn flag(&mut self, key: usize, update: ObjectUpdate) -> scenery::Result<()> {
        self.scene
            .flag_token(key, update)
            .map_err(|e| scenery::Error::Binding(e.0))
    }
    fn stop(&mut self, p: &pulses::Handle) -> scenery::Result<()> {
        if self.scene.tasks.contains_key(&p.identity()) {
            pulses::stop_base(p);
            Ok(())
        } else {
            self.services
                .stop(self.scene, p)
                .map_err(|e| scenery::Error::Binding(e.0))
        }
    }
    fn pulse_direct(&mut self, p: &pulses::Handle) -> scenery::Result<bool> {
        self.body(p).map_err(|e| scenery::Error::Binding(e.0))
    }
    fn submit(&mut self, p: SceneryPulse) -> scenery::Result<()> {
        if let Some(h) = &p.check {
            self.scene.checks.insert(h.identity(), h.downgrade());
        }
        self.scene.tasks.insert(
            p.pulse.identity(),
            Task::Scenery {
                base: p.pulse.downgrade(),
                action: WeakAction::new(&p.action),
                check: p.check.as_ref().map(Shared::downgrade),
            },
        );
        let base = p.pulse.clone();
        let capture = Shared::new(p);
        self.scene
            .runner
            .submit(Some(base.keeping_from(&capture)))
            .map_err(|e| scenery::Error::Binding(format!("{e:?}")))
    }
    fn ground_create(&mut self, item: Option<usize>) -> scenery::Result<()> {
        self.services
            .ground_create(self.scene, item)
            .map_err(|e| scenery::Error::Binding(e.0))
    }
}
impl<A: Assets, S: Services<A>> landscape::Context for World<'_, A, S> {
    fn definition(&mut self, h: &Handle) -> landscape::Result<definitions::Handle> {
        scenery_node::definition(h).map_err(|e| landscape::Error::Binding(format!("{e:?}")))
    }
    fn child_has_actions(&mut self, h: &definitions::Handle) -> landscape::Result<bool> {
        struct NullPlayer;
        impl definitions::PlayerContext for NullPlayer {
            fn varbit(&mut self, _: i32, _: usize) -> definitions::Result<Option<i32>> {
                unreachable!()
            }
            fn varp(&mut self, _: i32, _: usize) -> definitions::Result<i32> {
                unreachable!()
            }
        }
        let h = self
            .scene
            .definitions
            .child(&mut NullPlayer, h, None)
            .map_err(|e| landscape::Error::Binding(format!("{e:?}")))?;
        self.scene
            .definitions
            .has_actions(&h)
            .map_err(|e| landscape::Error::Binding(format!("{e:?}")))
    }
    fn load_region(&mut self, l: &Location) -> landscape::Result<()> {
        let [x, y, _] = l.coordinates();
        self.load(((x >> 6) << 8) | (y >> 6), None)
            .map(|_| ())
            .map_err(|e| landscape::Error::Binding(e.0))
    }
    fn region_chunk(&mut self, l: &Location) -> landscape::Result<chunk::HandleChunk> {
        self.scene
            .chunk(l)
            .map_err(|e| landscape::Error::Binding(e.0))
    }
}
struct MapWorld<'a, 'b, A: Assets, S: Services<A>> {
    world: &'a mut World<'b, A, S>,
    region: region::Handle,
}
impl<A: Assets, S: Services<A>> clipping::Context for MapWorld<'_, '_, A, S> {
    fn flags(&mut self, k: i32, p: bool) -> clipping::Result<clipping::Flags> {
        clipping::Context::flags(self.world, k, p)
    }
    fn border(&mut self, b: clipping::Border) -> clipping::Result<()> {
        clipping::Context::border(self.world, b)
    }
}
impl<A: Assets, S: Services<A>> mapscape::Context for MapWorld<'_, '_, A, S> {
    fn region_chunks(&mut self) -> mapscape::Result<Option<landscape::Chunks>> {
        Ok(self.region.read().chunks.clone())
    }
}
impl<A: Assets, S: Services<A>> region::Context for World<'_, A, S> {
    fn archive_id(&mut self, name: &str) -> region::Result<i32> {
        self.scene
            .assets
            .archive_id(name)
            .map_err(|e| region::Error::Binding(e.0))
    }
    fn mapscape_bytes(&mut self, id: i32) -> region::Result<Option<Vec<u8>>> {
        self.scene
            .assets
            .mapscape(id)
            .map_err(|e| region::Error::Binding(e.0))
    }
    fn xtea(&mut self, id: i32) -> region::Result<[u32; 4]> {
        self.scene
            .assets
            .keys(id)
            .map_err(|e| region::Error::Binding(e.0))
    }
    fn landscape_bytes(&mut self, id: i32, key: [u32; 4]) -> region::Result<Option<Vec<u8>>> {
        self.scene
            .assets
            .landscape(id, key)
            .map_err(|e| region::Error::Binding(e.0))
    }
    fn parse_mapscape(
        &mut self,
        r: &region::Handle,
        m: &landscape::Mapscape,
        bytes: &[u8],
    ) -> region::Result<()> {
        mapscape::parse(
            &mut MapWorld {
                world: self,
                region: r.clone(),
            },
            m,
            &mut Reader::new(bytes),
        )
        .map_err(|e| region::Error::Binding(format!("{e:?}")))
    }
    fn parse_landscape(
        &mut self,
        r: &region::Handle,
        chunks: Option<landscape::Chunks>,
        m: &landscape::Mapscape,
        bytes: &[u8],
        build: bool,
    ) -> region::Result<()> {
        let chunks = chunks.ok_or(region::Error::Null)?;
        let data = r.read().data.clone();
        landscape::parse(self, &data, &chunks, m, &mut Reader::new(bytes), build)
            .map_err(|e| region::Error::Binding(format!("{e:?}")))
    }
    fn clip_mapscape(
        &mut self,
        r: &region::Handle,
        chunks: Option<landscape::Chunks>,
        m: &landscape::Mapscape,
    ) -> region::Result<()> {
        mapscape::clip(
            &mut MapWorld {
                world: self,
                region: r.clone(),
            },
            &chunks.ok_or(region::Error::Null)?,
            m,
        )
        .map_err(|e| region::Error::Binding(format!("{e:?}")))
    }
    fn object(&mut self, l: Location, id: i32, shape: i32) -> region::Result<Option<Handle>> {
        self.object(l, id, shape)
            .map_err(|e| region::Error::Binding(e.0))
    }
    fn remove(&mut self, h: &Handle) -> region::Result<()> {
        scenery::remove(self, Some(h))
            .map(|_| ())
            .map_err(|e| region::Error::Binding(format!("{e:?}")))
    }
    fn add(&mut self, h: &Handle) -> region::Result<()> {
        scenery::add(self, Some(h), -1, None)
            .map(|_| ())
            .map_err(|e| region::Error::Binding(format!("{e:?}")))
    }
    fn exception(&mut self, e: &region::Error, id: Option<i32>) -> region::Result<()> {
        self.scene.diagnostics.push(format!("region {id:?}: {e:?}"));
        Ok(())
    }
}
impl<A: Assets, S: Services<A>> doors::Context for World<'_, A, S> {
    fn config(&mut self, id: i32) -> doors::Result<Option<doors::Door>> {
        Ok(self.scene.doors.get(id))
    }
    fn object(&mut self, l: Location, shape: i32) -> doors::Result<Option<Handle>> {
        self.object(l, -1, shape)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn construct(&mut self, id: i32, l: Location, s: i32, r: i32) -> doors::Result<Handle> {
        scenery_node::construct(&mut self.scene.definitions, id, l, s, r, false)
            .map_err(|e| doors::Error::Binding(format!("{e:?}")))
    }
    fn replace(
        &mut self,
        a: &Handle,
        b: Handle,
        ticks: Option<i32>,
        clip: bool,
    ) -> doors::Result<bool> {
        scenery::replace(
            self,
            Some(a),
            Some(&b),
            ticks.map_or(
                scenery::ReplaceMode::Swap {
                    clip,
                    permanent: false,
                },
                |ticks| scenery::ReplaceMode::Temporary { clip, ticks },
            ),
        )
        .map_err(|e| doors::Error::Binding(format!("{e:?}")))
    }
    fn is_player(&mut self, e: Entity) -> bool {
        self.services.is_player(e)
    }
    fn entity_location(&mut self, e: Entity) -> doors::Result<Location> {
        self.services
            .location(self.scene, e)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn lock(&mut self, e: Entity, t: i32) -> doors::Result<()> {
        self.services
            .lock(self.scene, e, t)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn audio(&mut self, e: Entity, id: i32, d: Option<i32>) -> doors::Result<()> {
        self.services
            .audio(self.scene, e, id, d)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn message(&mut self, e: Entity, t: &str) -> doors::Result<()> {
        self.services
            .message(self.scene, e, t)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn logout_listener(&mut self, e: Entity, l: Option<Location>) -> doors::Result<()> {
        self.services
            .logout_listener(self.scene, e, l)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn reset_path(&mut self, e: Entity) -> doors::Result<()> {
        self.services
            .reset_path(self.scene, e)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn add_path(&mut self, e: Entity, x: i32, y: i32) -> doors::Result<()> {
        self.services
            .add_path(self.scene, e, x, y)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn path_move_near(&mut self, e: Entity, l: Location) -> doors::Result<bool> {
        self.services
            .move_near(self.scene, e, l)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn diary(&mut self, e: Entity, d: doors::Diary, l: i32, t: i32) -> doors::Result<()> {
        self.services
            .diary(self.scene, e, d, l, t)
            .map_err(|e| doors::Error::Binding(e.0))
    }
    fn submit(&mut self, p: doors::DoorPulse) -> doors::Result<()> {
        self.scene.tasks.insert(
            p.pulse.identity(),
            Task::Door {
                base: p.pulse.downgrade(),
                action: p.action,
            },
        );
        self.scene
            .runner
            .submit(Some(p.pulse))
            .map_err(|e| doors::Error::Binding(format!("{e:?}")))
    }
}
impl<A: Assets, S: Services<A>> pulses::Context<()> for World<'_, A, S> {
    fn pulse(&mut self, p: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        self.body(p).map_err(|e| pulses::Error::Binding(e.0))
    }
    fn node_active(&mut self, n: usize, _: &mut ()) -> pulses::Result<bool> {
        Ok(self
            .scene
            .checks
            .get(&n)
            .and_then(WeakShared::upgrade)
            .is_some_and(|h| h.read().active))
    }
    fn exception(&mut self, e: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        self.scene.diagnostics.push(format!("pulse: {e:?}"));
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding(
            "entity script manager is not part of scenery scene".into(),
        ))
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding(
            "entity script manager is not part of scenery scene".into(),
        ))
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        Err(pulses::Error::Binding(
            "entity life state is not part of scenery scene".into(),
        ))
    }
    fn send_message(&mut self, e: usize, t: &str, _: &mut ()) -> pulses::Result<()> {
        self.services
            .message(self.scene, e, t)
            .map_err(|e| pulses::Error::Binding(e.0))
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        Ok(self.services.now_millis())
    }
    fn log(&mut self, _: pulses::Level, m: String, _: &mut ()) -> pulses::Result<()> {
        self.scene.diagnostics.push(m);
        Ok(())
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn count_pulse(&mut self, n: &str, _: &mut ()) -> pulses::Result<()> {
        *self.scene.pulse_counts.entry(n.into()).or_default() += 1;
        Ok(())
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
}
impl From<CacheError> for Error {
    fn from(e: CacheError) -> Self {
        error(e)
    }
}
