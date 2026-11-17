//! NPCs made after the start-up: `new NPC(id, location)` / `NPC.create`,
//! `NPC.init()`, `NPC.transform(id)` and `NPC.clear()` in the live world
//! (graves now; familiars and random events need the same calls).
//!
//! Construction and the parts of `init` that only touch the NPC itself
//! (`Entity.init` with `addAutoTimers`, `initConfig`, `configure`,
//! `setDefaultBehavior`, the child NPCs, `behavior.onCreation`) are the
//! start-up's `npc_init::Context` (`startup::npcs::Init`) over the live
//! world's canonical actors and NPC store, the start-up's NPC definition
//! table (cache archive 18 and the configurations `NPCConfigParser` set) and
//! the start-up's timer registry. What reaches the world is the live world's:
//! `Repository.npcs.add` (the live repository and the start-up's NPC list,
//! which keep the same slots), `RegionManager.move` (`entity_regions` over
//! the scene's chunks, with the zone monitor's location update),
//! `Region.isActive` and `Repository.addRenderableNPC`.
//!
//! These bodies are ported from the pinned `NPC.java`/`Entity.java` without a
//! differential corpus of their own (the parts they call are the verified
//! libraries: npc_init, entity_regions, repository, update_masks).
use super::{P, host::Live};
use crate::{
    definitions::{item::Shared, npc as defs},
    startup::{ground::Chunks, npcs::Init, regions::Regions},
    timers::registry::{Level, Registry as TimerRegistry},
    world::{
        doors::Location,
        npc, npc_behaviour,
        npc_init::{self, Change, Skill, Slot},
        player_walk::{Node, NodeHandle},
        region_activity,
        update_masks::Flag,
    },
};
use rs09_cache::archive::Archive;
use rs09_game_core::direction::Direction;
use std::{collections::BTreeMap, sync::Arc};

/// What runtime NPC creation keeps from the start-up.
pub struct Spawning {
    /// `NPCDefinition`'s table (the start-up's, with the configurations
    /// `NPCConfigParser` applied); later ids load lazily from archive 18.
    pub definitions: defs::Registry,
    pub archive: Option<Arc<Archive>>,
    /// The start-up's `TimerRegistry` (its automatic timers for NPCs).
    pub timers: TimerRegistry,
    /// `registerTimer` lines of runtime NPCs (none are expected).
    pub timer_log: Vec<(Level, String)>,
    pub holiday_event_randoms: bool,
    /// `NPC.originalId` of every NPC that was transformed, kept from its
    /// first transform (an NPC's id equals its original id until then).
    pub original: BTreeMap<Node, i32>,
}

impl Spawning {
    pub fn new(
        definitions: defs::Registry,
        archive: Option<Arc<Archive>>,
        timers: TimerRegistry,
        holiday_event_randoms: bool,
    ) -> Self {
        Self {
            definitions,
            archive,
            timers,
            timer_log: Vec::new(),
            holiday_event_randoms,
            original: BTreeMap::new(),
        }
    }
}

fn binding(e: impl std::fmt::Debug) -> npc_init::Error {
    npc_init::Error::Binding(format!("{e:?}"))
}

impl Live {
    /// Runs `f` with the start-up's `npc_init::Context` over the live
    /// world's actors, NPC store and generators. Its own chunk and region
    /// tables are empty: the world-reaching members are overridden by
    /// [`Runtime`].
    fn with_init<T>(&mut self, f: impl FnOnce(&mut Init<'_>) -> T) -> T {
        let mut chunks = Chunks::default();
        let mut regions = Regions::default();
        let spawning = &mut self.content.spawning;
        let mut init = Init {
            actors: &mut self.inner.state,
            npcs: &mut self.inner.npcs,
            definitions: &mut spawning.definitions,
            archive: spawning.archive.as_deref(),
            timers: &spawning.timers,
            generators: &mut self.inner.generators,
            chunks: &mut chunks,
            regions: &mut regions,
            holiday_event_randoms: spawning.holiday_event_randoms,
            ticks: self.inner.ticks,
            timer_log: &mut spawning.timer_log,
            spawned: None,
        };
        f(&mut init)
    }

    /// `new NPC(id, location, direction)` (or the registered subclass's
    /// `construct`, as `NPC.create` does) without `init`.
    pub fn npc_construct(
        &mut self,
        id: i32,
        location: Option<Location>,
        direction: Option<Direction>,
    ) -> npc_init::Result<NodeHandle> {
        let node = npc_init::create(&mut Runtime { live: self }, id, location, direction, &())?;
        self.inner.state.retain_node(node).map_err(binding)
    }

    /// `new NPC(id, location, direction)` itself (a subclass constructor
    /// such as `new Grave()` calls it directly, not through `NPC.create`).
    pub fn npc_new(
        &mut self,
        id: i32,
        location: Option<Location>,
        direction: Option<Direction>,
    ) -> npc_init::Result<NodeHandle> {
        let node = npc_init::Context::base(&mut Runtime { live: self }, id, location, direction)?;
        self.inner.state.retain_node(node).map_err(binding)
    }

    /// `NPC.init()` of an NPC made at runtime: a subclass's `init()`
    /// (`npc_classes::classes::init`, whose `super.init()` is
    /// [`Live::npc_init_base`]); the dragon classes do not override it.
    pub fn npc_init(&mut self, node: Node) -> npc_init::Result<()> {
        let class = self.inner.npcs.rows.get(&node).and_then(|row| row.class);
        match class.and_then(|c| c.npc()) {
            None => self.npc_init_base(node),
            Some(_) => {
                let npc = P(self.inner.state.retain_node(node).map_err(binding)?);
                crate::world::npc_classes::classes::init(
                    &mut super::subclasses::Classes::new(self),
                    &npc,
                )
                .map_err(binding)
            }
        }
    }

    /// `NPC.init()` itself.
    pub fn npc_init_base(&mut self, node: Node) -> npc_init::Result<()> {
        npc_init::init(&mut Runtime { live: self }, node)
    }

    /// `NPCDefinition.forId(id)`.
    pub fn npc_definition(&mut self, id: i32) -> Result<defs::Handle, String> {
        let spawning = &mut self.content.spawning;
        let mut files = ArchiveFiles(spawning.archive.as_deref());
        spawning
            .definitions
            .for_id(&mut files, id)
            .map_err(|e| format!("{e:?}"))
    }

    /// The `location` field of an NPC (Kotlin's `this.location = ...` in
    /// `Grave`): the walking entity's location and the NPC kernel's copy.
    pub fn npc_set_location_field(&mut self, node: Node, location: Option<Location>) {
        if let Ok(actor) = self.inner.state.actor(node) {
            actor.queue.state.write().entity.location = location.clone();
        }
        if let Some(row) = self.inner.npcs.rows.get(&node) {
            row.brain.fields.write().location = location;
        }
    }

    /// `NPC.transform(id)`: the id, definition, size, a new InteractPlugin
    /// (made when first asked for), `initConfig`, `configure`, then the
    /// TypeSwap mask (synced unless back to the original id, whose synced
    /// TypeSwap is unregistered first).
    pub fn npc_transform(&mut self, node: Node, id: i32) -> npc_init::Result<()> {
        let original = {
            let row = self
                .inner
                .npcs
                .rows
                .get(&node)
                .ok_or(npc_init::Error::Null)?;
            *self.content.spawning.original.entry(node).or_insert(row.id)
        };
        let definition = {
            let spawning = &mut self.content.spawning;
            let mut files = ArchiveFiles(spawning.archive.as_deref());
            spawning.definitions.for_id(&mut files, id)?
        };
        let size = definition.read().size;
        {
            let row = self
                .inner
                .npcs
                .rows
                .get_mut(&node)
                .ok_or(npc_init::Error::Null)?;
            row.id = id;
            row.definition = definition;
        }
        self.inner.state.actor_mut(node).map_err(binding)?.view.size = size;
        // The renderer's view shows the new id to new viewers.
        if let Some(view) = self.npc_views.get(&node) {
            view.write().id = id;
        }
        {
            let mut runtime = Runtime { live: self };
            npc_init::init_config(&mut runtime, node)?;
            npc_init::Context::configure(&mut runtime, node)?;
            npc_init::Context::interact_default(&mut runtime, node)?;
        }
        let masks = self.npc_masks(node);
        if id == original {
            masks.unregister_synced(5).map_err(binding)?;
        }
        masks
            .register(
                &mut super::combat::masks::Registry,
                Flag::TypeSwap,
                Some(super::combat::masks::Payload::TypeSwap(id)),
                id != original,
            )
            .map_err(binding)?;
        self.project_npc(node);
        Ok(())
    }

    /// `NPC.clear()`: `Entity.clear` (inactive, out of its chunk, the pulse
    /// manager cleared), `Repository.removeRenderableNPC`,
    /// `Repository.getNpcs().remove` and `behavior.onRemoval`.
    pub fn npc_clear(&mut self, handle: &NodeHandle) -> npc_init::Result<()> {
        let node = handle.node;
        if let Some(row) = self.inner.npcs.rows.get_mut(&node) {
            row.active = false;
        }
        if let Ok(actor) = self.inner.state.actor_mut(node) {
            actor.view.active = false;
        }
        self.region_clear(handle).map_err(binding)?;
        self.with_walk(|world| world.clear_pulses(node))
            .map_err(binding)?;
        let p = P(handle.clone());
        let repository = self.repository.clone();
        repository.remove_renderable_npc(self, &p);
        repository.npcs().remove(Some(&p));
        self.inner.npcs.repository.npcs().remove(Some(handle));
        let behaviour = npc_behaviour::behaviour(self, &p).map_err(binding)?;
        behaviour.on_removal(self, &p).map_err(binding)?;
        self.project_npc(node);
        Ok(())
    }

    /// The number of NPCs made at runtime that are active now.
    pub fn runtime_npcs_active(&self) -> usize {
        self.content
            .spawning
            .original
            .keys()
            .filter(|node| self.inner.node_active(**node))
            .count()
    }
}

/// `NPCDefinition.forId`'s cache reads (archive 18, file id >> 7, id & 127).
struct ArchiveFiles<'a>(Option<&'a Archive>);
impl defs::Files for ArchiveFiles<'_> {
    fn file(&mut self, _: &mut defs::Registry, id: i32) -> defs::Result<Option<Vec<u8>>> {
        let archive = self
            .0
            .ok_or_else(|| defs::Error::Binding("the NPC archive is not open".into()))?;
        Ok(archive
            .file((id as u32) >> 7, (id & 127) as u32, [0; 4])?
            .map(|b| b.as_ref().clone()))
    }
}

/// `npc_init::Context` for an NPC made at runtime: the start-up's context
/// for what stays inside the NPC, the live world for the repository, the
/// region move, region activity and the renderable list.
struct Runtime<'a> {
    live: &'a mut Live,
}

impl Runtime<'_> {
    fn handle(&mut self, node: Node) -> npc_init::Result<NodeHandle> {
        self.live.inner.state.retain_node(node).map_err(binding)
    }
    fn row(&self, node: Node) -> npc_init::Result<&crate::startup::npcs::Npc> {
        self.live
            .inner
            .npcs
            .rows
            .get(&node)
            .ok_or(npc_init::Error::Null)
    }
}

impl npc_init::Context for Runtime<'_> {
    type Arguments = ();
    fn prototype(&mut self, id: i32) -> npc_init::Result<Option<Node>> {
        self.live.with_init(|i| npc_init::Context::prototype(i, id))
    }
    fn construct(
        &mut self,
        prototype: Node,
        id: i32,
        location: Option<Location>,
        arguments: &(),
    ) -> npc_init::Result<Option<Node>> {
        self.live
            .with_init(|i| npc_init::Context::construct(i, prototype, id, location, arguments))
    }
    fn base(
        &mut self,
        id: i32,
        location: Option<Location>,
        direction: Option<Direction>,
    ) -> npc_init::Result<Node> {
        self.live
            .with_init(|i| npc_init::Context::base(i, id, location, direction))
    }
    fn definition(&self, node: Node) -> npc_init::Result<Option<defs::Handle>> {
        Ok(Some(self.row(node)?.definition.clone()))
    }
    fn brain(&self, node: Node) -> npc_init::Result<npc::State> {
        Ok(self.row(node)?.brain.clone())
    }
    fn location(&self, node: Node) -> npc_init::Result<Option<Location>> {
        self.live.inner.state.node_location(node).map_err(binding)
    }
    fn index(&self, node: Node) -> npc_init::Result<i32> {
        Ok(self
            .live
            .inner
            .state
            .actor(node)
            .map_err(binding)?
            .wire_index)
    }
    fn size(&self, node: Node) -> npc_init::Result<i32> {
        Ok(self
            .live
            .inner
            .state
            .actor(node)
            .map_err(binding)?
            .view
            .size)
    }
    fn set_index(&mut self, node: Node, index: i32) -> npc_init::Result<()> {
        self.live
            .inner
            .state
            .actor_mut(node)
            .map_err(binding)?
            .wire_index = index;
        Ok(())
    }
    fn set_size(&mut self, node: Node, size: i32) -> npc_init::Result<()> {
        self.live
            .inner
            .state
            .actor_mut(node)
            .map_err(binding)?
            .view
            .size = size;
        Ok(())
    }
    fn children(&self, node: Node) -> npc_init::Result<Option<Shared<Vec<Option<Node>>>>> {
        self.live.inner.state.children(node).map_err(binding)
    }
    fn set_children(
        &mut self,
        node: Node,
        children: Shared<Vec<Option<Node>>>,
    ) -> npc_init::Result<()> {
        self.live
            .inner
            .state
            .set_children(node, children)
            .map_err(binding)
    }
    fn entity_init(&mut self, node: Node) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::entity_init(i, node))
    }
    fn properties(&mut self, node: Node) -> npc_init::Result<Option<usize>> {
        self.row(node)?;
        Ok(Some(node))
    }
    fn skills(&mut self, node: Node) -> npc_init::Result<Option<usize>> {
        self.row(node)?;
        Ok(Some(node))
    }
    fn set_property(&mut self, properties: Option<usize>, change: Change) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::set_property(i, properties, change))
    }
    fn get_animation(
        &mut self,
        properties: Option<usize>,
        slot: Slot,
    ) -> npc_init::Result<Option<Shared<crate::definitions::property::Animation>>> {
        self.live
            .with_init(|i| npc_init::Context::get_animation(i, properties, slot))
    }
    fn set_skill(
        &mut self,
        skills: Option<usize>,
        skill: Skill,
        id: i32,
        level: i32,
    ) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::set_skill(i, skills, skill, id, level))
    }
    fn maximum_life_points(&mut self, skills: Option<usize>) -> npc_init::Result<i32> {
        self.live
            .with_init(|i| npc_init::Context::maximum_life_points(i, skills))
    }
    fn familiar(&self, _: Node) -> npc_init::Result<bool> {
        // `this instanceof Familiar`: familiars are not ported, so no
        // runtime NPC is one.
        Ok(false)
    }
    fn attribute_hp(&mut self, _: Node, _: i32) -> npc_init::Result<i32> {
        Err(npc_init::Error::Binding(
            "not ported in the live world: a familiar's hp attribute".into(),
        ))
    }
    fn spawn_location(
        &mut self,
        properties: Option<usize>,
        location: Option<Location>,
    ) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::spawn_location(i, properties, location))
    }
    fn add_repository(&mut self, node: Node) -> npc_init::Result<()> {
        // Repository.getNpcs().add(this): NodeList.add, the slot is the
        // index; the start-up's list keeps the same slots as the live one.
        let handle = self.handle(node)?;
        let slot = self.live.repository.npcs().add(P(handle.clone()));
        let mirror = self.live.inner.npcs.repository.npcs().add(handle);
        if slot != mirror {
            return Err(npc_init::Error::Binding(format!(
                "a runtime NPC took slot {slot:?} of the live repository and {mirror:?} of the start-up's"
            )));
        }
        if let Some(slot) = slot {
            npc_init::Context::set_index(self, node, slot)?;
        }
        Ok(())
    }
    fn region_move(
        &mut self,
        node: Node,
        from: Option<Location>,
        to: Option<Location>,
    ) -> npc_init::Result<()> {
        let to = to.ok_or(npc_init::Error::Null)?;
        let handle = self.handle(node)?;
        self.live.region_move(&handle, from, to).map_err(binding)
    }
    fn region_active(&mut self, location: Location) -> npc_init::Result<bool> {
        let region = self.live.region(super::walk::region_id(&location));
        Ok(region_activity::is_active(&region))
    }
    fn add_renderable(&mut self, node: Node) -> npc_init::Result<()> {
        let p = P(self.handle(node)?);
        let repository = self.live.repository.clone();
        repository.add_renderable_npc(self.live, &p);
        Ok(())
    }
    fn interact_default(&mut self, node: Node) -> npc_init::Result<()> {
        // interactPlugin.setDefault(): the live world makes an NPC's
        // InteractPlugin over its definition's options when first asked.
        self.row(node)?;
        Ok(())
    }
    fn configure(&mut self, node: Node) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::configure(i, node))
    }
    fn default_behavior(&mut self, node: Node) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::default_behavior(i, node))
    }
    fn behavior_created(&mut self, node: Node) -> npc_init::Result<()> {
        self.live
            .with_init(|i| npc_init::Context::behavior_created(i, node))
    }
}
