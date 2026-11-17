//! `NPCSpawner` in the native start-up: the verified spawn parser
//! (`world::npc_spawns`) and NPC factory/initialisation (`world::npc_init`)
//! over canonical actors (`world::player_walk::State`), the world's
//! `Repository` (`world::repository`), its regions and chunks, its timer
//! registry and generators, and `npc_behaviour::set_default_behaviour`.
//!
//! What `NPC.create` / `NPC.init` reach, and what answers it here:
//!
//! - `AbstractNPC.forId(id)`: the registry of NPC subclasses
//!   (`world::npc_classes::registry`) that the class scan filled with the
//!   classes of [`STARTUP_NPC_CLASSES`] (their prototypes are
//!   [`Prototype`]s); `construct` is the class's constructor over a
//!   canonical NPC tagged with its class, and `init` runs the class's own
//!   statements after `NPC.init`. The original registers 1,118 ids of
//!   content subclasses; every other class is content not ported, so its
//!   NPCs are base `NPC`s here (`fixtures/startup-draws` lists the classes
//!   the original used: 3,646 NPCs of its spawner phase, of 132 classes).
//! - `new NPC(id, location, direction)`: a canonical NPC actor, its
//!   `NPCDefinition.forId(id)` (the world's lazy definition table, as the
//!   constructor looks it up), size, direction and `NPCBehavior.forId(id)`
//!   (the world's behaviour registry: the default behaviour, content
//!   behaviours being class-scan content).
//! - `Entity.init`: `active = true` and `TimerRegistry.addAutoTimers` over the
//!   world's timer registry (`startup::timers`): the three native automatic
//!   timers are instantiated and registered on the NPC's own timer manager;
//!   `AntiMacro.onRegister` makes the NPC's one shared draw.
//! - `initConfig`: `npc_init::init_config` over the NPC's properties and
//!   skills held here.
//! - `Repository.npcs.add`: the world's `NodeList`; the slot it returns is the
//!   NPC's index.
//! - `RegionManager.move(npc, null, location)`: the chunk's NPC list
//!   (`entity_regions::add_npc`), the region entered in the world's region
//!   table. `zoneMonitor.updateLocation` is not run: the zones a region holds
//!   during the original's spawner phase are registered by class-scan content
//!   (each NPC's missing zone update is counted as a gap).
//! - `getLocation().getRegion().isActive()`: false. A region becomes active
//!   when a player's viewport reaches it; no player exists during start-up.
//! - `interactPlugin.setDefault()`: not bound for NPCs (NPC option handlers
//!   are class-scan content); counted as a gap per NPC.
//! - `configure()`: the bonus index, protect style, attack style and combat
//!   style. The magic autocast spell (`DefaultCombatSpell`, `SpellBook`) and
//!   `Tasks.forId` (slayer content) are not ported; counted as gaps.
//! - `setDefaultBehavior()`: `npc_behaviour::set_default_behaviour`.
//! - `behavior.onCreation`: the NPC's behaviour (the default one does nothing).
use super::generators::WorldGenerators;
use super::ground::Chunks;
use super::regions::Regions;
use super::timers::{Entity as TimerEntity, Natives};
use crate::definitions::{
    item::Shared,
    npc::{self as defs, Graphics},
    property::{Animation, CombatStyle as PropertyStyle, Property},
};
use crate::timers::{self as timer_lib, registry::Registry as TimerRegistry};
use crate::world::creatures::classes::WorldClass;
use crate::world::dragonfire::dragons::DragonClass;
use crate::world::npc_classes::{
    self as nc, Class,
    registry::{self as classes, Registry},
};
use crate::world::{
    combat_swing::{self as cs, CombatStyle, Kind},
    doors::Location,
    entity_regions, npc,
    npc_behaviour::{self as behaviour, Ai, Hitsplat, PlainNpc, Rights},
    npc_init::{self, Change, Skill, Slot},
    npc_spawns,
    player_walk::{Node, NodeHandle, State},
    repository::Repository,
};
use rs09_cache::archive::Archive;
use rs09_game_core::direction::Direction;
use std::{collections::BTreeMap, sync::Arc};

/// `ServerConstants.MAX_PLAYERS` and `MAX_NPCS` (constants in the pinned
/// server, not configuration).
pub const MAX_PLAYERS: i32 = 2000;
pub const MAX_NPCS: i32 = 32000;
/// The four enum identity hashes a canonical actor takes. The world-trace JVM
/// runs with `-XX:hashCode=2`, where every identity hash is 1.
pub const ENUM_HASHES: [i32; 4] = [1, 1, 1, 1];

/// The members of `Entity.properties` that `initConfig` and `configure` set.
#[derive(Default)]
pub struct Properties {
    pub spawn: Option<Location>,
    pub combat_level: i32,
    pub attack_speed: i32,
    pub bonuses: Option<Shared<Vec<i32>>>,
    /// attack, defence, death, magic, range.
    pub animations: [Option<Shared<Animation>>; 5],
    pub death_graphics: Option<Shared<Graphics>>,
    /// `protectStyle`, from the `protect_style` configuration.
    pub protect_style: Option<Property>,
    /// `attackStyle = AttackStyle(STYLE_CONTROLLED, index)`: the bonus index.
    pub attack_style_bonus: Option<i32>,
    /// `combatPulse.style`, from the `combat_style` configuration.
    pub combat_style: Option<Property>,
}

/// One NPC of the start-up world.
pub struct Npc {
    pub id: i32,
    /// The AbstractNPC subclass it was constructed as (`None`: a base NPC).
    pub class: Option<WorldClass>,
    pub definition: defs::Handle,
    pub brain: npc::State,
    pub direction: Option<Direction>,
    pub active: bool,
    pub attributes: Vec<(&'static str, bool)>,
    pub properties: Properties,
    pub static_levels: [i32; 25],
    pub levels: [i32; 25],
    pub lifepoints: i32,
    pub timers: timer_lib::Manager,
}

/// What the original does for an NPC that this port leaves out, per kind,
/// counted over the start-up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unported {
    pub zone_updates: u32,
    pub interact_defaults: u32,
    pub slayer_tasks: u32,
    pub magic_autocasts: u32,
    /// `SkillRestore`, the automatic timer without a native port.
    pub skill_restore_timers: u32,
}

/// The NPCs of the start-up world and the registries their creation uses.
pub struct Npcs {
    pub repository: Repository<NodeHandle>,
    pub rows: BTreeMap<Node, Npc>,
    pub ai: BTreeMap<Node, Ai<Npcs>>,
    pub behaviours: behaviour::Registry<Npcs>,
    /// `AbstractNPC.mapping`: each id's class and prototype (an index of
    /// [`Npcs::prototypes`]).
    pub classes: Registry<WorldClass, usize>,
    /// The prototype objects the class scan constructed.
    pub prototypes: Vec<Prototype>,
    roots: BTreeMap<Node, NodeHandle>,
    pub unported: Unported,
}
impl Npcs {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            repository: Repository::new(MAX_PLAYERS, MAX_NPCS).map_err(|e| e.to_string())?,
            rows: BTreeMap::new(),
            ai: BTreeMap::new(),
            behaviours: behaviour::Registry::default(),
            classes: Registry::default(),
            prototypes: Vec::new(),
            roots: BTreeMap::new(),
            unported: Unported::default(),
        })
    }
    /// `Repository.npcs` as `[slot, index, id, x, y, z]`, as the boot record
    /// lists it (the index is the one the NPC reports).
    pub fn list(&self, actors: &State) -> Vec<[Option<i32>; 6]> {
        self.repository
            .npcs()
            .occupied()
            .into_iter()
            .map(|(slot, handle)| {
                let location = actors.node_location(handle.node).ok().flatten();
                let index = actors.actor(handle.node).map(|a| a.wire_index).ok();
                let id = self.rows.get(&handle.node).map(|r| r.id);
                let c = location.map(|l| l.coordinates());
                [
                    Some(slot),
                    index,
                    id,
                    c.map(|c| c[0]),
                    c.map(|c| c[1]),
                    c.map(|c| c[2]),
                ]
            })
            .collect()
    }
}

fn error(e: impl std::fmt::Display) -> npc_init::Error {
    npc_init::Error::Binding(e.to_string())
}
fn class_error(e: nc::Error) -> npc_init::Error {
    match e {
        nc::Error::Null => npc_init::Error::Null,
        nc::Error::Index => npc_init::Error::Index,
        nc::Error::Cast => npc_init::Error::Type,
        nc::Error::Binding(m) => npc_init::Error::Binding(m.unwrap_or_default()),
    }
}

/// The AbstractNPC classes the start-up world registers: those whose
/// construction and `init` it can answer. `BankerNPC.init` looks for a bank
/// booth with `getScenery` and `DarkWizardNPC.init` sets an autocast spell;
/// the start-up world holds neither region objects nor spells, so those two
/// stay unregistered here (their NPCs are base NPCs, as before) and are
/// named as gaps by the class scan step. The dragon classes of
/// `world::dragonfire::dragons` (one `WorldClass` with this slice) construct
/// plain dragons: their combat action and static dragonfire are made by a
/// combat world (docs/dragonfire.md).
pub const STARTUP_NPC_CLASSES: [WorldClass; 10] = [
    WorldClass::Npc(Class::Shop),
    WorldClass::Npc(Class::Cow),
    WorldClass::Npc(Class::Duck),
    WorldClass::Npc(Class::Rat),
    WorldClass::Npc(Class::Guard),
    WorldClass::Npc(Class::Monk),
    WorldClass::Npc(Class::HighwayMan),
    WorldClass::Npc(Class::IceGiant),
    WorldClass::Dragon(DragonClass::Chromatic),
    WorldClass::Dragon(DragonClass::Metal),
];

/// A prototype object the class scan constructed (`new ShopNPC()`): never
/// initialised, never in the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prototype {
    pub class: WorldClass,
    pub id: i32,
    pub walks: bool,
    pub aggressive: bool,
}

/// What the class scan's `newInstance()` of an AbstractNPC class reaches:
/// `NPC(id, null)` (its `NPCDefinition.forId(id)` lookups) and the
/// constructors' field writes, recorded on the prototype.
pub struct Prototypes<'a> {
    pub npcs: &'a mut Npcs,
    pub definitions: &'a mut defs::Registry,
    pub archive: Option<&'a Archive>,
}
impl nc::Factory for Prototypes<'_> {
    type Npc = usize;
    type Class = WorldClass;
    fn new_npc(
        &mut self,
        class: Option<WorldClass>,
        id: i32,
        _: Option<[i32; 3]>,
        _: Option<Direction>,
    ) -> nc::Result<usize> {
        let class =
            class.ok_or_else(|| nc::Error::Binding(Some("a prototype has a class".into())))?;
        let lookup = |e: defs::Error| nc::Error::Binding(Some(format!("{e:?}")));
        // super(NPCDefinition.forId(id).getName(), location); definition = forId(id).
        self.definitions
            .for_id(&mut Files(self.archive), id)
            .map_err(lookup)?;
        self.definitions
            .for_id(&mut Files(self.archive), id)
            .map_err(lookup)?;
        self.npcs.prototypes.push(Prototype {
            class,
            id,
            walks: false,
            aggressive: false,
        });
        Ok(self.npcs.prototypes.len() - 1)
    }
    fn set_walks(&mut self, npc: &usize, walks: bool) -> nc::Result<()> {
        self.npcs.prototypes[*npc].walks = walks;
        Ok(())
    }
    fn set_aggressive(&mut self, npc: &usize, aggressive: bool) -> nc::Result<()> {
        self.npcs.prototypes[*npc].aggressive = aggressive;
        Ok(())
    }
    fn log(&mut self, origin: &str, _: nc::LogLevel, message: &str) -> nc::Result<()> {
        // Two registered classes never share an id; the start-up log has no
        // ContentAPI.log line for it.
        Err(nc::Error::Binding(Some(format!(
            "{origin}: {message} (not answered in the start-up world)"
        ))))
    }
}

/// The subclass constructors over the start-up world's canonical NPCs.
impl nc::Factory for Init<'_> {
    type Npc = Node;
    type Class = WorldClass;
    fn new_npc(
        &mut self,
        class: Option<WorldClass>,
        id: i32,
        location: Option<[i32; 3]>,
        direction: Option<Direction>,
    ) -> nc::Result<Node> {
        let fail = |e: npc_init::Error| nc::Error::Binding(Some(e.to_string()));
        let location = location.map(|[x, y, z]| Location::new(x, y, z));
        let node = npc_init::Context::base(self, id, location, direction).map_err(fail)?;
        let row = self.row_mut(node).map_err(fail)?;
        row.class = class;
        if let Some(radius) = class.and_then(WorldClass::walk_radius) {
            row.brain.fields.write().walk_radius_override = Some(radius);
        }
        Ok(node)
    }
    fn set_walks(&mut self, npc: &Node, walks: bool) -> nc::Result<()> {
        let fail = |e: npc_init::Error| nc::Error::Binding(Some(e.to_string()));
        self.row(*npc).map_err(fail)?.brain.fields.write().walks = walks;
        Ok(())
    }
    fn set_aggressive(&mut self, npc: &Node, aggressive: bool) -> nc::Result<()> {
        let fail = |e: npc_init::Error| nc::Error::Binding(Some(e.to_string()));
        self.row(*npc)
            .map_err(fail)?
            .brain
            .fields
            .write()
            .aggressive = aggressive;
        Ok(())
    }
    fn log(&mut self, origin: &str, _: nc::LogLevel, message: &str) -> nc::Result<()> {
        Err(nc::Error::Binding(Some(format!(
            "{origin}: {message} (construction does not log)"
        ))))
    }
}

struct Files<'a>(Option<&'a Archive>);
impl defs::Files for Files<'_> {
    fn file(&mut self, _: &mut defs::Registry, id: i32) -> defs::Result<Option<Vec<u8>>> {
        let archive = self
            .0
            .ok_or_else(|| defs::Error::Binding("cache not initialised".into()))?;
        Ok(archive
            .file((id as u32) >> 7, (id & 127) as u32, [0; 4])?
            .map(|b| b.as_ref().clone()))
    }
}

/// One use of `npc_init` / `npc_spawns` against the start-up world.
pub struct Init<'a> {
    pub actors: &'a mut State,
    pub npcs: &'a mut Npcs,
    pub definitions: &'a mut defs::Registry,
    pub archive: Option<&'a Archive>,
    pub timers: &'a TimerRegistry,
    pub generators: &'a mut WorldGenerators,
    pub chunks: &'a mut Chunks,
    pub regions: &'a mut Regions,
    pub holiday_event_randoms: bool,
    /// `GameWorld.getTicks()`.
    pub ticks: i32,
    /// `registerTimer` lines (none are made when NPCs register timers).
    pub timer_log: &'a mut Vec<(timer_lib::registry::Level, String)>,
    pub spawned: Option<i32>,
}
impl Init<'_> {
    fn row(&self, n: Node) -> npc_init::Result<&Npc> {
        self.npcs.rows.get(&n).ok_or(npc_init::Error::Null)
    }
    fn row_mut(&mut self, n: Node) -> npc_init::Result<&mut Npc> {
        self.npcs.rows.get_mut(&n).ok_or(npc_init::Error::Null)
    }
    fn handle(&self, n: Node) -> npc_init::Result<NodeHandle> {
        self.npcs
            .roots
            .get(&n)
            .cloned()
            .ok_or(npc_init::Error::Null)
    }
    fn config(&self, n: Node, key: &str) -> npc_init::Result<Option<Property>> {
        let d = self.row(n)?.definition.clone();
        let handlers = d.read().handlers.clone();
        Ok(handlers.read().get(key).cloned())
    }
}
fn slot(s: Slot) -> usize {
    match s {
        Slot::Attack => 0,
        Slot::Defence => 1,
        Slot::Death => 2,
        Slot::Magic => 3,
        Slot::Range => 4,
    }
}

impl npc_init::Context for Init<'_> {
    type Arguments = ();
    fn prototype(&mut self, id: i32) -> npc_init::Result<Option<Node>> {
        // AbstractNPC.forId(id): the prototype's index.
        Ok(self.npcs.classes.for_id(id).map(|(_, p)| *p))
    }
    fn construct(
        &mut self,
        prototype: Node,
        id: i32,
        location: Option<Location>,
        _: &(),
    ) -> npc_init::Result<Option<Node>> {
        // ((AbstractNPC) n).construct(id, location, objects)
        let class = self
            .npcs
            .prototypes
            .get(prototype)
            .ok_or(npc_init::Error::Index)?
            .class;
        let at = location.map(|l| l.coordinates());
        match nc::Abstract::construct(&class, at).map_err(class_error)? {
            None => Ok(None),
            Some(constructor) => classes::construct_npc(self, class, id, at, constructor)
                .map(Some)
                .map_err(class_error),
        }
    }
    fn base(
        &mut self,
        id: i32,
        location: Option<Location>,
        direction: Option<Direction>,
    ) -> npc_init::Result<Node> {
        // super(NPCDefinition.forId(id).getName(), location); definition = forId(id).
        self.definitions.for_id(&mut Files(self.archive), id)?;
        let definition = self.definitions.for_id(&mut Files(self.archive), id)?;
        let size = definition.read().size;
        let node = self.actors.add_npc_at(0, location.clone(), ENUM_HASHES);
        {
            let a = self.actors.actor_mut(node).map_err(error)?;
            a.connected = false;
            a.view.active = false;
            a.view.size = size;
            a.queue.state.write().entity.direction = direction;
        }
        let handle = self.actors.retain_node(node).map_err(error)?;
        self.npcs.roots.insert(node, handle);
        let brain = npc::State::new(location.clone().unwrap_or_else(|| Location::new(0, 0, 0)));
        {
            let mut f = brain.fields.write();
            f.location = location;
            f.spawn = None;
        }
        // this.behavior = NPCBehavior.forId(id)
        let behaviour = self.npcs.behaviours.for_id(id);
        self.npcs
            .ai
            .insert(node, Ai::new(behaviour, Arc::new(PlainNpc)));
        self.npcs.rows.insert(
            node,
            Npc {
                id,
                class: None,
                definition,
                brain,
                direction,
                active: false,
                attributes: Vec::new(),
                properties: Properties::default(),
                static_levels: [1; 25],
                levels: [1; 25],
                lifepoints: 10,
                timers: timer_lib::Manager::default(),
            },
        );
        Ok(node)
    }
    fn definition(&self, n: Node) -> npc_init::Result<Option<defs::Handle>> {
        Ok(Some(self.row(n)?.definition.clone()))
    }
    fn brain(&self, n: Node) -> npc_init::Result<npc::State> {
        Ok(self.row(n)?.brain.clone())
    }
    fn location(&self, n: Node) -> npc_init::Result<Option<Location>> {
        self.actors.node_location(n).map_err(error)
    }
    fn index(&self, n: Node) -> npc_init::Result<i32> {
        Ok(self.actors.actor(n).map_err(error)?.wire_index)
    }
    fn size(&self, n: Node) -> npc_init::Result<i32> {
        Ok(self.actors.actor(n).map_err(error)?.view.size)
    }
    fn set_index(&mut self, n: Node, i: i32) -> npc_init::Result<()> {
        self.actors.actor_mut(n).map_err(error)?.wire_index = i;
        Ok(())
    }
    fn set_size(&mut self, n: Node, s: i32) -> npc_init::Result<()> {
        self.actors.actor_mut(n).map_err(error)?.view.size = s;
        Ok(())
    }
    fn children(&self, n: Node) -> npc_init::Result<Option<Shared<Vec<Option<Node>>>>> {
        self.actors.children(n).map_err(error)
    }
    fn set_children(&mut self, n: Node, c: Shared<Vec<Option<Node>>>) -> npc_init::Result<()> {
        self.actors.set_children(n, c).map_err(error)
    }
    fn entity_init(&mut self, n: Node) -> npc_init::Result<()> {
        // Entity.init: active = true; TimerRegistry.addAutoTimers(this).
        self.row_mut(n)?.active = true;
        self.actors.actor_mut(n).map_err(error)?.view.active = true;
        let mut manager = std::mem::take(&mut self.row_mut(n)?.timers);
        let mut entity = TimerEntity {
            player: false,
            artificial: false,
            ticks: self.ticks,
            holiday_event_randoms: self.holiday_event_randoms,
            generators: self.generators,
            log: self.timer_log,
        };
        let result = self
            .timers
            .add_automatic(&mut manager, &mut entity, &mut Natives);
        self.row_mut(n)?.timers = manager;
        result.map_err(|e| error(format!("{e:?}")))?;
        self.npcs.unported.skill_restore_timers += 1;
        Ok(())
    }
    fn properties(&mut self, n: Node) -> npc_init::Result<Option<usize>> {
        self.row(n)?;
        Ok(Some(n))
    }
    fn skills(&mut self, n: Node) -> npc_init::Result<Option<usize>> {
        self.row(n)?;
        Ok(Some(n))
    }
    fn set_property(&mut self, p: Option<usize>, change: Change) -> npc_init::Result<()> {
        let p = &mut self.row_mut(p.ok_or(npc_init::Error::Null)?)?.properties;
        match change {
            Change::CombatLevel(v) => p.combat_level = v,
            Change::Animation(s, a) => p.animations[slot(s)] = a,
            Change::Bonuses(b) => p.bonuses = Some(b),
            Change::AttackSpeed(v) => p.attack_speed = v,
            Change::DeathGraphic(g) => p.death_graphics = Some(g),
        }
        Ok(())
    }
    fn get_animation(
        &mut self,
        p: Option<usize>,
        s: Slot,
    ) -> npc_init::Result<Option<Shared<Animation>>> {
        Ok(self
            .row(p.ok_or(npc_init::Error::Null)?)?
            .properties
            .animations[slot(s)]
        .clone())
    }
    fn set_skill(
        &mut self,
        s: Option<usize>,
        kind: Skill,
        id: i32,
        level: i32,
    ) -> npc_init::Result<()> {
        let row = self.row_mut(s.ok_or(npc_init::Error::Null)?)?;
        let i = usize::try_from(id)
            .ok()
            .filter(|i| *i < 25)
            .ok_or(npc_init::Error::Index)?;
        match kind {
            // setStaticLevel: static and dynamic level (experience is not kept).
            Skill::Static => {
                row.static_levels[i] = level;
                row.levels[i] = level;
            }
            // setLevel: hitpoints also sets lifepoints.
            Skill::Current => {
                if i == 3 {
                    row.lifepoints = level;
                }
                row.levels[i] = level;
            }
        }
        Ok(())
    }
    fn maximum_life_points(&mut self, s: Option<usize>) -> npc_init::Result<i32> {
        // An NPC's lifepointsIncrease is 0.
        Ok(self.row(s.ok_or(npc_init::Error::Null)?)?.static_levels[3])
    }
    fn familiar(&self, _: Node) -> npc_init::Result<bool> {
        Ok(false)
    }
    fn attribute_hp(&mut self, _: Node, _: i32) -> npc_init::Result<i32> {
        Err(error("familiar NPCs are content"))
    }
    fn spawn_location(&mut self, p: Option<usize>, l: Option<Location>) -> npc_init::Result<()> {
        let row = self.row_mut(p.ok_or(npc_init::Error::Null)?)?;
        row.properties.spawn = l.clone();
        row.brain.fields.write().spawn = l;
        Ok(())
    }
    fn add_repository(&mut self, n: Node) -> npc_init::Result<()> {
        let handle = self.handle(n)?;
        // NodeList.add: the slot it takes is the node's index; a full list
        // adds nothing.
        if let Some(slot) = self.npcs.repository.npcs().add(handle) {
            self.set_index(n, slot)?;
        }
        Ok(())
    }
    fn region_move(
        &mut self,
        n: Node,
        from: Option<Location>,
        to: Option<Location>,
    ) -> npc_init::Result<()> {
        let to = to.ok_or(npc_init::Error::Null)?;
        let destination = self.chunks.get(self.regions, &to);
        let source = from.map(|l| self.chunks.get(self.regions, &l));
        if source.as_ref().is_none_or(|s| !s.same(&destination)) {
            if let Some(source) = source {
                entity_regions::remove_npc(&source, Some(self.handle(n)?));
            }
            entity_regions::add_npc(&destination, Some(self.handle(n)?));
        }
        self.npcs.unported.zone_updates += 1;
        Ok(())
    }
    fn region_active(&mut self, location: Location) -> npc_init::Result<bool> {
        let [x, y, _] = location.coordinates();
        self.regions.for_id(((x >> 6) << 8) | (y >> 6));
        Ok(false)
    }
    fn add_renderable(&mut self, n: Node) -> npc_init::Result<()> {
        // Repository.addRenderableNPC: `if (!contains) add`.
        let handle = self.handle(n)?;
        let list = self.npcs.repository.renderable_npcs();
        if !list.contains(&handle) {
            list.add(handle);
        }
        Ok(())
    }
    fn interact_default(&mut self, _: Node) -> npc_init::Result<()> {
        self.npcs.unported.interact_defaults += 1;
        Ok(())
    }
    fn configure(&mut self, n: Node) -> npc_init::Result<()> {
        // NPC.configure().
        let bonus = match self.config(n, "bonuses")? {
            None | Some(Property::Null) => Shared::new(vec![0; 3]),
            Some(Property::IntArray(v)) => v,
            _ => return Err(npc_init::Error::Type),
        };
        let (mut highest, mut index) = (0, 0);
        for i in 0..3 {
            let b = *bonus.read().get(i).ok_or(npc_init::Error::Index)?;
            if b > highest {
                highest = b;
                index = i as i32;
            }
        }
        let protect = self.config(n, "protect_style")?.filter(|p| !p.is_null());
        let style = self.config(n, "combat_style")?.filter(|p| !p.is_null());
        let magic = matches!(&style, Some(Property::CombatStyle(PropertyStyle::Magic)));
        let row = self.row_mut(n)?;
        if let Some(protect) = protect {
            row.properties.protect_style = Some(protect);
        }
        row.properties.attack_style_bonus = Some(index);
        if let Some(style) = style {
            row.properties.combat_style = Some(style);
        }
        if magic {
            self.npcs.unported.magic_autocasts += 1;
        }
        // task = Tasks.forId(getId()).
        self.npcs.unported.slayer_tasks += 1;
        Ok(())
    }
    fn default_behavior(&mut self, n: Node) -> npc_init::Result<()> {
        behaviour::aggression::set_default_behaviour(self.npcs, &n)
            .map_err(|e| error(format!("{e:?}")))
    }
    fn behavior_created(&mut self, n: Node) -> npc_init::Result<()> {
        let b = behaviour::behaviour(self.npcs, &n).map_err(|e| error(format!("{e:?}")))?;
        b.on_creation(self.npcs, &n)
            .map_err(|e| error(format!("{e:?}")))
    }
}

impl npc_spawns::Context for Init<'_> {
    type Npc = Node;
    fn create(&mut self, id: i32, location: Location) -> npc_spawns::Result<Node> {
        npc_init::create(self, id, Some(location), Some(Direction::South), &())
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))
    }
    fn walks(&mut self, n: &Node, walks: bool) -> npc_spawns::Result<()> {
        let row = self
            .row(*n)
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?;
        row.brain.fields.write().walks = walks;
        Ok(())
    }
    fn direction(&mut self, n: &Node, d: Direction) -> npc_spawns::Result<()> {
        self.row_mut(*n)
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?
            .direction = Some(d);
        self.actors
            .actor(*n)
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?
            .queue
            .state
            .write()
            .entity
            .direction = Some(d);
        Ok(())
    }
    fn attribute(&mut self, n: &Node, key: &'static str, value: bool) -> npc_spawns::Result<()> {
        let row = self
            .row_mut(*n)
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?;
        match row.attributes.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => row.attributes.push((key, value)),
        }
        Ok(())
    }
    fn facing_booth(&mut self, n: &Node) -> npc_spawns::Result<bool> {
        match self
            .config(*n, "facing_booth")
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?
        {
            None | Some(Property::Null) => Ok(false),
            Some(Property::Bool(v)) => Ok(v),
            _ => Err(npc_spawns::Error::Type),
        }
    }
    fn init(&mut self, n: &Node) -> npc_spawns::Result<()> {
        // A subclass's init(): super.init(), then its own statements
        // (world::npc_classes::classes::init; only ShopNPC's of the
        // registered classes has any).
        let binding = |e: npc_init::Error| npc_spawns::Error::Binding(e.to_string());
        npc_init::init(self, *n).map_err(binding)?;
        match self
            .row(*n)
            .map_err(binding)?
            .class
            .and_then(WorldClass::npc)
        {
            Some(Class::Shop) => nc::Factory::set_walks(self, n, true)
                .map_err(|e| npc_spawns::Error::Binding(e.to_string())),
            Some(class @ (Class::Banker | Class::DarkWizard)) => Err(npc_spawns::Error::Binding(
                format!("{} is not registered in the start-up world", class.name()),
            )),
            _ => Ok(()),
        }
    }
    fn log(&mut self, count: i32) -> npc_spawns::Result<()> {
        self.spawned = Some(count);
        Ok(())
    }
}

fn unbound<T>(what: &str) -> behaviour::Result<T> {
    Err(cs::Error::Binding(format!(
        "{what} is not reached while an NPC is created and is not bound in the start-up world"
    )))
}

impl cs::Types for Npcs {
    type Entity = Node;
    type Item = ();
    type Spell = ();
}

/// `npc_behaviour`'s services for the start-up world's NPCs. Creating an NPC
/// reaches only the definition, configuration, `aggressive` and the AI fields;
/// every other member refuses.
impl behaviour::Host for Npcs {
    type Handler = ();
    type Region = i32;
    fn kind(&self, entity: &Node) -> Kind {
        if self.rows.contains_key(entity) {
            Kind::Npc
        } else {
            Kind::Other
        }
    }
    fn same(&self, a: &Node, b: &Node) -> bool {
        a == b
    }
    fn ai(&mut self, npc: &Node) -> Option<&mut Ai<Self>> {
        self.ai.get_mut(npc)
    }
    fn brain(&self, npc: &Node) -> Option<npc::State> {
        self.rows.get(npc).map(|r| r.brain.clone())
    }
    fn location(&self, entity: &Node) -> Option<Location> {
        self.rows.get(entity)?.brain.fields.read().location.clone()
    }
    fn set_location(&mut self, _: &Node, _: Option<Location>) {}
    fn style_handler(&self, _: CombatStyle) {}
    fn region_dynamic(&self, _: &i32) -> bool {
        false
    }
    fn max_players(&self) -> i32 {
        MAX_PLAYERS
    }
    fn ticks(&mut self) -> behaviour::Result<i32> {
        unbound("GameWorld.getTicks")
    }
    fn current_time_millis(&mut self) -> behaviour::Result<i64> {
        unbound("the clock")
    }
    fn next_int(&mut self, _: i32) -> behaviour::Result<i32> {
        unbound("RandomFunction.RANDOM")
    }
    fn math_random(&mut self) -> behaviour::Result<f64> {
        unbound("Math.random")
    }
    fn is_active(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("Node.isActive")
    }
    fn is_dead(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("DeathTask.isDead")
    }
    fn interaction_locked(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("locks")
    }
    fn attribute_bool(&mut self, _: &Node, _: &str, _: bool) -> behaviour::Result<bool> {
        unbound("attributes")
    }
    fn attribute_long(&mut self, _: &Node, _: &str, _: i64) -> behaviour::Result<i64> {
        unbound("attributes")
    }
    fn attribute_entity(&mut self, _: &Node, _: &str) -> behaviour::Result<Option<Node>> {
        unbound("attributes")
    }
    fn set_attribute_entity(&mut self, _: &Node, _: &str, _: &Node) -> behaviour::Result<()> {
        unbound("attributes")
    }
    fn remove_attribute(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("attributes")
    }
    fn config_bool(&mut self, npc: &Node, key: &str, default: bool) -> behaviour::Result<bool> {
        match self.config(npc, key)? {
            None | Some(Property::Null) => Ok(default),
            Some(Property::Bool(v)) => Ok(v),
            _ => Err(cs::Error::Binding(format!(
                "configuration {key} is not a boolean"
            ))),
        }
    }
    fn config_int(&mut self, npc: &Node, key: &str, default: i32) -> behaviour::Result<i32> {
        match self.config(npc, key)? {
            None | Some(Property::Null) => Ok(default),
            Some(Property::Int(v)) => Ok(v),
            _ => Err(cs::Error::Binding(format!(
                "configuration {key} is not an integer"
            ))),
        }
    }
    fn definition_combat_level(&mut self, npc: &Node) -> behaviour::Result<i32> {
        Ok(self.row_of(npc)?.definition.read().combat_level)
    }
    fn has_action(&mut self, _: &Node, _: &str) -> behaviour::Result<bool> {
        unbound("NPCDefinition.hasAction")
    }
    fn shown_npc(&mut self, _: &Node, _: &Node) -> behaviour::Result<Node> {
        unbound("getShownNPC")
    }
    fn is_attacking(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the combat pulse")
    }
    fn combat_victim(&mut self, _: &Node) -> behaviour::Result<Option<Node>> {
        unbound("the combat pulse")
    }
    fn combat_set_victim(&mut self, _: &Node, _: &Node) -> behaviour::Result<()> {
        unbound("the combat pulse")
    }
    fn combat_attack(&mut self, _: &Node, _: Option<&Node>) -> behaviour::Result<()> {
        unbound("the combat pulse")
    }
    fn combat_stop(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("the combat pulse")
    }
    fn next_attack(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("the combat pulse")
    }
    fn combat_style(&mut self, _: &Node) -> behaviour::Result<Option<CombatStyle>> {
        unbound("the combat pulse")
    }
    fn set_last_received_attack(&mut self, _: &Node, _: i32) -> behaviour::Result<()> {
        unbound("the combat state")
    }
    fn last_sent_attack(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("the combat state")
    }
    fn set_last_sent_attack(&mut self, _: &Node, _: i32) -> behaviour::Result<()> {
        unbound("the combat state")
    }
    fn face(&mut self, _: &Node, _: Option<&Node>) -> behaviour::Result<()> {
        unbound("Entity.face")
    }
    fn is_multi_zone(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("properties.isMultiZone")
    }
    fn current_combat_level(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("properties.getCurrentCombatLevel")
    }
    fn is_retaliating(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("properties.isRetaliating")
    }
    fn set_npc_walkable(&mut self, _: &Node, _: bool) -> behaviour::Result<()> {
        unbound("properties.setNPCWalkable")
    }
    fn set_combat_time_out(&mut self, _: &Node, _: i32) -> behaviour::Result<()> {
        unbound("properties.setCombatTimeOut")
    }
    fn local_players(&mut self, _: Location, _: i32) -> behaviour::Result<Vec<Node>> {
        unbound("RegionManager.getLocalPlayers")
    }
    fn wilderness_in_zone(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("WildernessZone")
    }
    fn region_for_id(&mut self, _: i32) -> behaviour::Result<i32> {
        unbound("RegionManager.forId")
    }
    fn is_tolerated(&mut self, _: &i32, _: &Node) -> behaviour::Result<bool> {
        unbound("Region.isTolerated")
    }
    fn rights(&mut self, _: &Node) -> behaviour::Result<Option<Rights>> {
        unbound("player rights")
    }
    fn player_invisible(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("Player.isInvisible")
    }
    fn skull_wilderness(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the skull manager")
    }
    fn wilderness_protection(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the skull manager")
    }
    fn settings(&mut self) -> behaviour::Result<bool> {
        unbound("GameWorld.settings")
    }
    fn wild_pvp_enabled(&mut self) -> behaviour::Result<bool> {
        unbound("GameWorld.settings")
    }
    fn zone_continue_attack(
        &mut self,
        _: &Node,
        _: &Node,
        _: Option<CombatStyle>,
        _: bool,
    ) -> behaviour::Result<bool> {
        unbound("zones")
    }
    fn zone_is_pvpable(
        &mut self,
        _: &Node,
        _: &Node,
        _: &Node,
        _: Option<CombatStyle>,
        _: bool,
    ) -> behaviour::Result<bool> {
        unbound("zones")
    }
    fn level(&mut self, _: &Node, _: i32) -> behaviour::Result<i32> {
        unbound("skills")
    }
    fn familiar_owner(&mut self, _: &Node) -> behaviour::Result<Option<Node>> {
        unbound("familiars")
    }
    fn send_message(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("messages")
    }
    fn path_find(&mut self, _: Option<&Node>, _: Option<Location>) -> behaviour::Result<()> {
        unbound("pathfinding")
    }
    fn path_walk(&mut self, _: Option<&Node>) -> behaviour::Result<()> {
        unbound("pathfinding")
    }
    fn move_step(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("movement")
    }
    fn walking_has_path(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the walking queue")
    }
    fn pulse_manager_is_moving(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the pulse manager")
    }
    fn ring_id(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("equipment")
    }
    fn send_chat(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("chat")
    }
    fn manual_hit(&mut self, _: &Node, _: &Node, _: i32, _: Hitsplat) -> behaviour::Result<()> {
        unbound("impacts")
    }
    fn handle_recoil_effect(
        &mut self,
        _: &Node,
        _: Option<&Node>,
        _: i32,
    ) -> behaviour::Result<()> {
        unbound("recoil")
    }
    fn check_armour_degrades(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("degradation")
    }
    fn walking_reset(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("the walking queue")
    }
    fn pulse_manager_clear(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("the pulse manager")
    }
    fn masks_reset(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("update masks")
    }
    fn chunk_remove(&mut self, _: &Node, _: Location) -> behaviour::Result<()> {
        unbound("chunk membership")
    }
    fn chunk_add(&mut self, _: &Node, _: Location) -> behaviour::Result<()> {
        unbound("chunk membership")
    }
    fn return_arrival(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the return pulse")
    }
    fn pulse_stop(&mut self, _: &Node, _: usize) -> behaviour::Result<()> {
        unbound("pulses")
    }
    fn remove_renderable(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("the renderable list")
    }
    fn npc_clear(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("NPC.clear")
    }
    fn skills_restore(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("skills")
    }
    fn remove_timer(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("timers")
    }
    fn clipping_flag(&mut self, _: i32, _: i32, _: i32) -> behaviour::Result<i32> {
        unbound("clipping")
    }
    fn other_clipping_flag(&mut self, _: u32, _: i32, _: i32, _: i32) -> behaviour::Result<i32> {
        unbound("clipping")
    }
}
impl Npcs {
    fn row_of(&self, npc: &Node) -> behaviour::Result<&Npc> {
        self.rows
            .get(npc)
            .ok_or_else(|| cs::Error::Binding("not an NPC of this world".into()))
    }
    fn config(&self, npc: &Node, key: &str) -> behaviour::Result<Option<Property>> {
        let d = self.row_of(npc)?.definition.clone();
        let handlers = d.read().handlers.clone();
        Ok(handlers.read().get(key).cloned())
    }
}
