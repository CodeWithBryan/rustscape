//! Owning-world assembly for player-to-location walking. Uses real native walk
//! requests, movement pulses, pulse managers/runner, smart search and queues.
//! Content, zone/region activity, scripts, settings packets and collision are
//! required services. Scenery targets/approaches are retained natively; legacy
//! arrival/content dispatch and complete Player startup are separate services.
use super::{
    doors::{Handle as SceneryHandle, Location},
    interaction::{Approach, ApproachKind},
    legacy_interaction,
    movement::{self, DestinationService, Kind, Movement, NodeView, Target},
    navigation::{self, Navigator},
    pathfinding::{self, dispatch},
    tick, walk_request,
    walking::{self, Queue},
};
use crate::{
    definitions::item::{Shared, WeakShared},
    pulses,
};
use std::collections::{BTreeMap, BTreeSet};
mod ground_targets;
use ground_targets::GroundTarget;
pub type Node = usize;
#[derive(Debug)]
pub enum Error {
    Null,
    Binding(String),
    Unsupported(&'static str),
    Movement(movement::Error),
    Walking(walking::Error),
    Pulse(pulses::Error),
    Request(walk_request::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy)]
pub enum Scripts {
    Reset,
    Weak,
    Normal,
    Pre,
    Post(bool),
}
pub enum Query<'a> {
    Locked(Node),
    CloseInterface(Node),
    CloseDialogue(Node),
    Timer(Node, &'a str),
    Marathon(Node),
    ZoneMove(Node, Location, Location),
}
pub enum Event<'a> {
    CloseChatbox(Node),
    Debug(Node, &'a str),
    Minimap(Node),
    Face(Node, Option<Node>),
    FaceLocationNone(Node),
    Scripts(Node, Scripts),
    Message(Node, &'a str),
    Energy(Node, f64),
    RunToggled(Node, bool),
    RegionMove(Node, Option<Location>, Option<Location>),
    TickEvent(Node, i32),
    Skills(Node),
    Timers(Node),
    Masks(Node),
    Exception(&'a str),
    Log(pulses::Level, &'a str),
    PulseLength(&'a str, i32),
    CountPulse(&'a str),
    BotTotal(i32),
    OtherTotal(i32),
}
/// Services can reenter via state.context(self). No actor/queue/pulse lock is held
/// across these callbacks. Same-workspace recursive smart search remains explicit
/// unsupported behavior rather than an accidental empty/fallback route.
pub trait Services {
    fn query(&mut self, state: &mut State, query: Query<'_>) -> Result<bool>;
    fn event(&mut self, state: &mut State, event: Event<'_>) -> Result<()>;
    fn interfaces(&self, state: &State, node: Node) -> Result<walk_request::Interfaces>;
    fn region(&mut self, state: &mut State, location: &Location) -> Result<Option<bool>>;
    fn clipping(&mut self, state: &mut State, z: i32, x: i32, y: i32) -> Result<i32>;
    fn water_clipping(&mut self, _: &mut State, _: i32, _: i32, _: i32) -> Result<i32> {
        Err(Error::Unsupported("water clipping"))
    }
    fn now_millis(&mut self) -> i64;
    fn metrics_enabled(&self) -> bool;
    /// Other task domains use the same global runner. Keep their node-check
    /// namespace separate while all base update/catch/telemetry ordering remains
    /// in this bridge. A host must reject pulses it does not own.
    /// Foreign captures of canonical actor IDs must retain their NodeHandles for
    /// the task's observable lifetime; foreign numeric checks are never traced
    /// as this state's node IDs.
    fn external_pulse(&mut self, _: &mut State, _: &pulses::Handle) -> pulses::Result<bool> {
        Err(pulses::Error::Binding("unregistered external pulse".into()))
    }
    fn external_inactive(&mut self, _: &mut State, _: &pulses::Handle) -> pulses::Result<bool> {
        Err(pulses::Error::Binding(
            "unregistered external checks".into(),
        ))
    }
    fn external_stop(&mut self, _: &mut State, _: &pulses::Handle) -> pulses::Result<()> {
        Err(pulses::Error::Binding("unregistered external stop".into()))
    }
    fn npc_finder(&mut self, _: &mut State, _: Node) -> Result<Option<usize>> {
        Err(Error::Unsupported("NPC behavior pathfinder"))
    }
    fn npc_supplier(&mut self, _: &mut State, _: Node) -> Result<Option<dispatch::Supplier>> {
        Err(Error::Unsupported("NPC clipping supplier"))
    }
    fn npc_arrival(&mut self, _: &mut State, _: Node) -> Result<bool> {
        Err(Error::Unsupported("NPC return arrival"))
    }
    fn node_active(&mut self, state: &mut State, node: Node) -> Result<bool> {
        Ok(state.node_view(node)?.active)
    }
    fn interaction_arrival(&mut self, _: &mut State, _: &Approach) -> Result<bool> {
        Err(Error::Unsupported("interaction arrival"))
    }
    fn delayed_interaction(&mut self, _: &mut State, _: &legacy_interaction::Task) -> Result<bool> {
        Err(Error::Unsupported("delayed interaction arrival"))
    }
    fn destination(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
        _: DestinationService,
    ) -> Result<Option<Location>> {
        Err(Error::Unsupported("content destination"))
    }
    fn door_destination(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
    ) -> Result<Option<Location>> {
        Err(Error::Unsupported("door destinations"))
    }
    fn teleport_permitted(&mut self, _: &mut State, _: Option<Location>) -> Result<bool> {
        Err(Error::Unsupported("item destinations"))
    }
}
pub struct Actor {
    pub wire_index: i32,
    pub connected: bool,
    pub queue: Queue,
    pub view: NodeView,
    pub manager: pulses::Manager,
    current: Option<usize>,
    scripts: crate::scripts::Runtime,
    clocks: Shared<Vec<i32>>,
    locks: super::action_locks::ActionLocks,
    player_fields: Option<crate::player::owned::Player>,
}
struct Motion {
    movement: Movement,
    arrival: Arrival,
}
struct Task {
    base: pulses::Handle,
    body: TaskBody,
}
#[derive(Clone)]
enum TaskBody {
    Interaction(legacy_interaction::Task),
    Unhandled(Node),
}
impl TaskBody {
    fn nodes(&self) -> [Option<Node>; 2] {
        match self {
            Self::Interaction(t) => [t.player, t.target],
            Self::Unhandled(player) => [Some(*player), None],
        }
    }
}
#[derive(Clone)]
enum Arrival {
    Player(Node, bool),
    NpcReturn(Node),
    Interaction(Approach),
}
impl Arrival {
    fn nodes(&self) -> [Option<Node>; 2] {
        match self {
            Self::Player(n, _) | Self::NpcReturn(n) => [Some(*n), None],
            Self::Interaction(a) => [Some(a.player), a.target],
        }
    }
}
/// Stable actor-domain identity, independent of numeric node IDs or a shared
/// scheduler. Retaining it owns static value defaults, not actor/task tables.
#[derive(Clone)]
pub struct Identity(Shared<Domain>);
#[derive(Default)]
struct Domain {
    appearance: crate::player::appearance::Factory,
    varbits: crate::definitions::varbit::Registry,
    varbit_assets: Option<std::sync::Arc<rs09_cache::archive::Archive>>,
    processors: BTreeMap<
        (Node, usize),
        WeakShared<crate::scripts::Fields<super::interaction_listeners::Function, NodeHandle>>,
    >,
}
impl Identity {
    /// One Entity.scripts allocation per actual retained entity. The index is
    /// weak: keeping a world identity does not retain any entity or processor.
    pub(crate) fn processor(&self, owner: NodeHandle) -> crate::scripts::Runtime {
        let key = (owner.node, owner._lease.identity());
        let mut domain = self.0.write();
        if let Some(fields) = domain.processors.get(&key).and_then(WeakShared::upgrade) {
            return crate::scripts::Runtime { fields };
        }
        let processor = crate::scripts::Runtime::with_entity_in(owner, self.clone());
        domain.processors.insert(key, processor.fields.downgrade());
        processor
    }
    fn prune_processors(&self) {
        self.0
            .write()
            .processors
            .retain(|_, weak| weak.upgrade().is_some());
    }
    pub fn varbit_assets(&self) -> Option<std::sync::Arc<rs09_cache::archive::Archive>> {
        self.0.read().varbit_assets.clone()
    }
    pub fn varbits(&self) -> crate::definitions::varbit::Registry {
        self.0.read().varbits.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub(crate) fn appearance_factory(&self) -> crate::player::appearance::Factory {
        self.0.read().appearance.clone()
    }
}
pub struct State {
    identity: Identity,
    actors: BTreeMap<Node, Actor>,
    locations: BTreeMap<Node, Location>,
    scenery: BTreeMap<Node, SceneryHandle>,
    scenery_ids: BTreeMap<usize, Node>,
    grounds: BTreeMap<Node, GroundTarget>,
    ground_ids: BTreeMap<usize, Node>,
    motions: BTreeMap<usize, Motion>,
    bases: BTreeMap<usize, usize>,
    tasks: BTreeMap<usize, Task>,
    check_arrays: BTreeMap<usize, WeakShared<Vec<Option<Node>>>>,
    order: Vec<Node>,
    next_node: usize,
    next_motion: usize,
    navigator: Option<Navigator>,
    runner: pulses::Runner,
    leases: BTreeMap<Node, WeakShared<()>>,
    attribute_motions: BTreeMap<Node, usize>,
    owned_children: BTreeMap<Node, Shared<Vec<Option<Node>>>>,
    child_arrays: BTreeMap<usize, WeakShared<Vec<Option<Node>>>>,
    pub bfs: bool,
    pub max_distance: i32,
    pub ticks: i32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            identity: Identity(Shared::new(Domain::default())),
            actors: BTreeMap::new(),
            locations: BTreeMap::new(),
            scenery: BTreeMap::new(),
            scenery_ids: BTreeMap::new(),
            grounds: BTreeMap::new(),
            ground_ids: BTreeMap::new(),
            motions: BTreeMap::new(),
            bases: BTreeMap::new(),
            tasks: BTreeMap::new(),
            check_arrays: BTreeMap::new(),
            order: vec![],
            next_node: 0,
            next_motion: 0,
            navigator: Some(Navigator::default()),
            runner: pulses::Runner::default(),
            leases: BTreeMap::new(),
            attribute_motions: BTreeMap::new(),
            owned_children: BTreeMap::new(),
            child_arrays: BTreeMap::new(),
            bfs: false,
            max_distance: 25,
            ticks: 0,
        }
    }
}
impl State {
    pub fn identity(&self) -> Identity {
        self.identity.clone()
    }
    /// Attach domains at world construction; changing a live queue would lose
    /// its original fixed prefix and pending task ownership.
    pub fn with_runner(runner: pulses::Runner) -> Self {
        Self {
            runner,
            ..Self::default()
        }
    }
    pub fn runner(&self) -> pulses::Runner {
        self.runner.clone()
    }
    fn allocate_node(&mut self) -> Node {
        self.next_node = self
            .next_node
            .checked_add(1)
            .expect("node identity exhausted");
        self.next_node
    }
    pub fn add(&mut self, wire_index: i32, location: Location, enum_hashes: [i32; 4]) -> Node {
        self.add_actor(wire_index, Some(location), enum_hashes, Kind::Player)
    }
    pub fn add_npc(&mut self, wire_index: i32, location: Location, enum_hashes: [i32; 4]) -> Node {
        self.add_npc_at(wire_index, Some(location), enum_hashes)
    }
    pub fn add_npc_at(
        &mut self,
        wire_index: i32,
        location: Option<Location>,
        enum_hashes: [i32; 4],
    ) -> Node {
        self.add_actor(wire_index, location, enum_hashes, Kind::Npc)
    }
    fn add_actor(
        &mut self,
        wire_index: i32,
        location: Option<Location>,
        enum_hashes: [i32; 4],
        kind: Kind,
    ) -> Node {
        let node = self.allocate_node();
        let mut view = NodeView::new(kind, location.clone());
        view.direction = Some(rs09_game_core::direction::Direction::South);
        let queue = Queue::new(walking::Entity::new(kind == Kind::Player, location));
        queue.state.write().entity.direction = view.direction;
        let lease = Shared::new(());
        self.leases.insert(node, lease.downgrade());
        let scripts = self.identity.processor(NodeHandle {
            node,
            _lease: lease,
        });
        self.actors.insert(
            node,
            Actor {
                wire_index,
                connected: true,
                queue,
                view,
                manager: pulses::Manager::new(enum_hashes),
                current: None,
                scripts,
                clocks: Shared::new(vec![0; 10]),
                locks: super::action_locks::ActionLocks::new_in(self.identity.clone()),
                player_fields: None,
            },
        );
        self.order.push(node);
        node
    }
    pub fn actor(&self, node: Node) -> Result<&Actor> {
        self.actors.get(&node).ok_or(Error::Null)
    }
    pub fn actor_mut(&mut self, node: Node) -> Result<&mut Actor> {
        self.actors.get_mut(&node).ok_or(Error::Null)
    }
    pub fn scripts(&self, owner: &NodeHandle) -> Result<crate::scripts::Runtime> {
        if !self.local_handle(owner) {
            return Err(Error::Binding(
                "script entity belongs to another world".into(),
            ));
        }
        Ok(self.actor(owner.node)?.scripts.clone())
    }
    pub fn clocks(&self, owner: &NodeHandle) -> Result<Shared<Vec<i32>>> {
        if !self.local_handle(owner) {
            return Err(Error::Binding(
                "clock entity belongs to another world".into(),
            ));
        }
        Ok(self.actor(owner.node)?.clocks.clone())
    }
    pub fn locks(&self, owner: &NodeHandle) -> Result<super::action_locks::ActionLocks> {
        if !self.local_handle(owner) {
            return Err(Error::Binding(
                "lock entity belongs to another world".into(),
            ));
        }
        Ok(self.actor(owner.node)?.locks.clone())
    }
    pub fn bind_player_fields(&mut self, player: crate::player::owned::Player) -> Result<()> {
        let owner = player.owner();
        if !player.in_world(&self.identity) || !self.local_handle(&owner) {
            return Err(Error::Binding(
                "player fields belong to another canonical world".into(),
            ));
        }
        if !self.scripts(&owner)?.same(&player.scripts()) {
            return Err(Error::Binding(
                "player has a different Entity.scripts allocation".into(),
            ));
        }
        if self.actors.iter().any(|(&node, actor)| {
            node != owner.node
                && actor
                    .player_fields
                    .as_ref()
                    .is_some_and(|other| player.same_masks(other))
        }) {
            return Err(Error::Binding(
                "prepared masks belong to another player".into(),
            ));
        }
        let slot = &mut self.actor_mut(owner.node)?.player_fields;
        if slot.as_ref().is_some_and(|old| !old.same(&player)) {
            return Err(Error::Binding(
                "canonical player fields are already bound".into(),
            ));
        }
        *slot = Some(player);
        Ok(())
    }
    /// Cache archive22 is fixed before canonical player admission. Reusing the
    /// same archive handle is idempotent; replacement never changes old worlds.
    pub fn bind_varbit_assets(
        &mut self,
        archive: std::sync::Arc<rs09_cache::archive::Archive>,
    ) -> Result<()> {
        let mut domain = self.identity.0.write();
        if let Some(old) = &domain.varbit_assets {
            return if std::sync::Arc::ptr_eq(old, &archive) {
                Ok(())
            } else {
                Err(Error::Binding(
                    "variable cache archive is already bound".into(),
                ))
            };
        }
        if self
            .actors
            .values()
            .any(|actor| actor.player_fields.is_some())
        {
            return Err(Error::Binding(
                "variable cache archive must precede player admission".into(),
            ));
        }
        domain.varbit_assets = Some(archive);
        Ok(())
    }
    pub fn player_fields(&self, owner: &NodeHandle) -> Result<crate::player::owned::Player> {
        if !self.local_handle(owner) {
            return Err(Error::Binding(
                "player receiver belongs to another world".into(),
            ));
        }
        self.actor(owner.node)?
            .player_fields
            .clone()
            .ok_or(Error::Null)
    }
    pub fn connected(&self) -> Vec<Node> {
        self.order
            .iter()
            .copied()
            .filter(|n| self.actors.get(n).is_some_and(|a| a.connected))
            .collect()
    }
    pub fn context<'a, S: Services>(&'a mut self, services: &'a mut S) -> World<'a, S> {
        World {
            state: self,
            services,
        }
    }
    pub fn retained_counts(&self) -> [usize; 3] {
        [
            self.actors.len(),
            self.locations.len() + self.scenery.len() + self.grounds.len(),
            self.motions.len(),
        ]
    }
    pub fn retained_tasks(&self) -> usize {
        self.tasks.len()
    }
    /// Node.getLocation does not consult scenery definitions or path metadata.
    pub fn node_location(&self, node: Node) -> Result<Option<Location>> {
        if let Some(a) = self.actors.get(&node) {
            return Ok(a.queue.entity().location);
        }
        if let Some(l) = self.locations.get(&node) {
            return Ok(Some(l.clone()));
        }
        if let Some(target) = self.grounds.get(&node) {
            return Ok(target.view().read().location.clone());
        }
        Ok(Some(self.scenery(node)?.read().location.clone()))
    }
    pub fn node_size(&self, node: Node) -> Result<i32> {
        if let Some(a) = self.actors.get(&node) {
            return Ok(a.view.size);
        }
        if self.locations.contains_key(&node) {
            return Ok(1);
        }
        if let Some(target) = self.grounds.get(&node) {
            return Ok(target.view().read().node_size);
        }
        Ok(self.scenery(node)?.read().node_size)
    }
    /// Reuse a live canonical identity for this exact scenery reference. An
    /// exposed NodeHandle or captured movement keeps that binding alive.
    pub fn scenery_target(&mut self, object: SceneryHandle) -> Result<NodeHandle> {
        if let Some(node) = self.scenery_ids.get(&object.identity()).copied() {
            return self.retain_node(node);
        }
        let node = self.allocate_node();
        self.scenery_ids.insert(object.identity(), node);
        self.scenery.insert(node, object.unowned());
        self.retain_node(node)
    }
    pub fn scenery(&self, node: Node) -> Result<SceneryHandle> {
        let object = self.scenery.get(&node).ok_or(Error::Null)?;
        super::scenery_node::retained_view(object).map_err(|e| Error::Binding(format!("{e:?}")))
    }
    /// Register this exact GroundItem reference, preserving a GroundSpawn owner
    /// through the supplied owning-world subtype registry. Removal from a chunk
    /// or manager does not replace a captured approach target.
    pub fn ground_target(
        &mut self,
        item: super::ground_item::Handle,
        registry: &super::ground_spawn::Registry,
    ) -> Result<NodeHandle> {
        if let Some(dropper) = &item.read().dropper {
            if !self.local_handle(dropper) {
                return Err(Error::Unsupported("ground dropper from another world"));
            }
        }
        let identity = item.identity();
        let target = GroundTarget::new(item, registry)?;
        if let Some(node) = self.ground_ids.get(&identity).copied() {
            return self.retain_node(node);
        }
        let node = self.allocate_node();
        self.grounds.insert(node, target);
        self.ground_ids.insert(identity, node);
        self.retain_node(node)
    }
    pub fn ground(&self, node: Node) -> Result<super::ground_item::Handle> {
        self.grounds
            .get(&node)
            .map(GroundTarget::view)
            .ok_or(Error::Null)
    }
    fn local_handle(&self, handle: &NodeHandle) -> bool {
        self.leases
            .get(&handle.node)
            .and_then(WeakShared::upgrade)
            .is_some_and(|lease| lease.same(&handle._lease))
    }
    /// Validate the actual retained allocation, never just a wire slot or node ID.
    pub fn contains_handle(&self, handle: &NodeHandle) -> bool {
        self.local_handle(handle) && self.node_view(handle.node).is_ok()
    }
    pub fn node_view(&self, node: Node) -> Result<NodeView> {
        if let Some(a) = self.actors.get(&node) {
            return Ok(navigation::view(a.view.clone(), &a.queue));
        }
        if let Some(l) = self.locations.get(&node) {
            return Ok(NodeView::new(Kind::Location, Some(l.clone())));
        }
        if let Some(target) = self.grounds.get(&node) {
            let active = target.active(self.ticks);
            let item = target.view();
            let item = item.read();
            let mut view = NodeView::new(Kind::Other, item.location.clone());
            view.size = item.node_size;
            view.active = active;
            return Ok(view);
        }
        let object = self.scenery(node)?;
        let h = object.read();
        let mut view = NodeView::new(Kind::Scenery, Some(h.location.clone()));
        view.size = h.node_size;
        view.direction = Some(h.direction);
        view.active = h.active;
        view.scenery_kind = h.shape;
        view.rotation = h.rotation;
        let d = h.definition.clone().ok_or(Error::Null)?;
        drop(h);
        let d = d.read();
        view.dimensions = [d.size_x, d.size_y];
        view.walking_flag = d.walking_flag;
        Ok(view)
    }
    pub fn current(&self, node: Node) -> Result<Option<Movement>> {
        Ok(self
            .actor(node)?
            .current
            .and_then(|id| self.motions.get(&id).map(|m| m.movement.clone())))
    }
    pub fn pending(&self) -> Vec<pulses::Handle> {
        self.runner.current()
    }
    /// An NPC's return-pulse attribute is an internal graph edge, not an external
    /// root. It keeps an old movement alive while its owning node is reachable.
    pub fn attribute_motion(&mut self, node: Node, motion: Option<usize>) -> Result<()> {
        self.actor(node)?;
        if let Some(id) = motion {
            if !self.motions.contains_key(&id) {
                return Err(Error::Null);
            }
            self.attribute_motions.insert(node, id);
        } else {
            self.attribute_motions.remove(&node);
        }
        Ok(())
    }
    pub fn movement(&self, id: usize) -> Result<Movement> {
        self.motions
            .get(&id)
            .map(|m| m.movement.clone())
            .ok_or(Error::Null)
    }
    /// Child references are internal graph edges. Exposed old arrays remain
    /// roots for their elements after a parent replaces or drops its array.
    pub fn set_children(&mut self, node: Node, children: Shared<Vec<Option<Node>>>) -> Result<()> {
        self.actor(node)?;
        self.child_arrays
            .insert(children.identity(), children.downgrade());
        self.owned_children.insert(node, children);
        Ok(())
    }
    pub fn children(&self, node: Node) -> Result<Option<Shared<Vec<Option<Node>>>>> {
        self.actor(node)?;
        Ok(self.owned_children.get(&node).cloned())
    }
    pub fn retain_node(&mut self, node: Node) -> Result<NodeHandle> {
        if !self.actors.contains_key(&node)
            && !self.locations.contains_key(&node)
            && !self.scenery.contains_key(&node)
            && !self.grounds.contains_key(&node)
        {
            return Err(Error::Null);
        }
        let lease = self
            .leases
            .get(&node)
            .and_then(WeakShared::upgrade)
            .unwrap_or_else(|| Shared::new(()));
        self.leases.insert(node, lease.downgrade());
        let mut handle = NodeHandle {
            node,
            _lease: lease,
        };
        if let Some(object) = self.scenery.get(&node) {
            let view = super::scenery_node::retained_view(object)
                .map_err(|e| Error::Binding(format!("{e:?}")))?;
            handle._lease = handle._lease.keeping_from(&view);
        }
        Ok(handle)
    }
    fn motion(&self, base: &pulses::Handle) -> Result<Movement> {
        self.bases
            .get(&base.identity())
            .and_then(|id| self.motions.get(id))
            .map(|m| m.movement.clone())
            .ok_or(Error::Unsupported("unregistered pulse"))
    }
    /// Trace roots are connected players and the actual runner queue. Follow
    /// current/manager/check references so disconnects and replaced walk targets
    /// can be reclaimed without recycling a still-observable node identity.
    pub fn collect(&mut self) {
        let mut nodes: BTreeSet<_> = self.connected().into_iter().collect();
        let mut players = crate::player::owned::trace(
            self.actors
                .values()
                .filter_map(|actor| actor.player_fields.as_ref()),
            self.actors
                .iter()
                .map(|(&node, actor)| (node, &actor.scripts)),
            self.actors
                .iter()
                .map(|(&node, actor)| (node, &actor.locks)),
            |owner| self.local_handle(owner),
            &self.identity,
        );
        nodes.extend(&players.roots);
        let mut internal_arrays: BTreeMap<usize, usize> = BTreeMap::new();
        for a in self.owned_children.values() {
            *internal_arrays.entry(a.identity()).or_default() += 1;
        }
        for base in self
            .motions
            .values()
            .map(|m| m.movement.base())
            .chain(self.tasks.values().map(|t| t.base.clone()))
        {
            if let Some(array) = base.read().checks.clone() {
                *internal_arrays.entry(array.identity()).or_default() += 1;
                self.check_arrays
                    .insert(array.identity(), array.downgrade());
            }
        }
        self.child_arrays.retain(|_, a| a.upgrade().is_some());
        for (&id, weak) in &self.child_arrays {
            if let Some(array) = weak.upgrade() {
                if array.strong_count() > 1 + internal_arrays.get(&id).copied().unwrap_or(0) {
                    nodes.extend(array.read().iter().flatten().copied());
                }
            }
        }
        self.check_arrays.retain(|_, a| a.upgrade().is_some());
        for (&id, weak) in &self.check_arrays {
            if let Some(array) = weak.upgrade() {
                if array.strong_count() > 1 + internal_arrays.get(&id).copied().unwrap_or(0) {
                    nodes.extend(array.read().iter().flatten().copied());
                }
            }
        }
        for (&node, a) in &self.actors {
            if a.queue.state.strong_count() > 1 || a.manager.strong_count() > 1 {
                nodes.insert(node);
            }
        }
        let mut internal_leases = std::mem::take(&mut players.internal_leases);
        for (&node, object) in &self.scenery {
            if object.strong_count() > 1 {
                nodes.insert(node);
            }
            if let Some(binding) = &object.read().interaction {
                let owner = binding.owner();
                if self.local_handle(&owner) {
                    *internal_leases.entry(owner.node).or_default() += 1;
                }
            }
        }
        for (&node, target) in &self.grounds {
            if target.external() {
                nodes.insert(node);
            }
            let item = target.view();
            let item = item.read();
            if let Some(dropper) = &item.dropper {
                if self.local_handle(dropper) {
                    *internal_leases.entry(dropper.node).or_default() += 1;
                }
            }
        }
        self.leases.retain(|_, lease| lease.upgrade().is_some());
        for (&node, weak) in &self.leases {
            if let Some(lease) = weak.upgrade() {
                if lease.strong_count() > 1 + internal_leases.get(&node).copied().unwrap_or(0) {
                    nodes.insert(node);
                }
            }
        }
        let mut internal: BTreeMap<usize, usize> = BTreeMap::new();
        for a in self.actors.values() {
            for (_, p) in a.manager.entries() {
                if let Some(p) = p {
                    *internal.entry(p.identity()).or_default() += 1;
                }
            }
        }
        for p in self.runner.current() {
            *internal.entry(p.identity()).or_default() += 1;
        }
        let mut motions: BTreeSet<_> = self
            .runner
            .current()
            .iter()
            .filter_map(|p| self.motion(p).ok().map(|p| p.id()))
            .collect();
        let mut tasks: BTreeSet<_> = self
            .runner
            .current()
            .iter()
            .filter_map(|p| {
                self.tasks
                    .contains_key(&p.identity())
                    .then_some(p.identity())
            })
            .collect();
        for (&id, task) in &self.tasks {
            if task.base.strong_count() > 1 + internal.get(&id).copied().unwrap_or(0) {
                tasks.insert(id);
            }
        }
        // Detached manager slots are internal edges, not external roots. This
        // avoids retaining the old node/current-movement cycle indefinitely.
        for (&id, m) in &self.motions {
            let fields = m.movement.state.read();
            let known = 1 + internal.get(&fields.base.identity()).copied().unwrap_or(0);
            if m.movement.state.strong_count() > 1 || fields.base.strong_count() > known {
                motions.insert(id);
            }
            if fields
                .base
                .read()
                .checks
                .as_ref()
                .is_some_and(|c| c.strong_count() > 1)
            {
                motions.insert(id);
            }
        }
        loop {
            let old = (nodes.len(), motions.len(), tasks.len());
            for node in nodes.clone() {
                if let Some(edges) = players.edges.get(&node) {
                    nodes.extend(edges);
                }
                if let Some(target) = self.grounds.get(&node) {
                    let item = target.view();
                    let item = item.read();
                    if let Some(dropper) = &item.dropper {
                        if self.local_handle(dropper) {
                            nodes.insert(dropper.node);
                        }
                    }
                }
                motions.extend(self.attribute_motions.get(&node).copied());
                if let Some(children) = self.owned_children.get(&node) {
                    nodes.extend(children.read().iter().flatten().copied());
                }
                if let Some(a) = self.actors.get(&node) {
                    motions.extend(a.current);
                    for (_, base) in a.manager.entries() {
                        if let Some(base) = base {
                            if let Ok(p) = self.motion(&base) {
                                motions.insert(p.id());
                            } else if self.tasks.contains_key(&base.identity()) {
                                tasks.insert(base.identity());
                            }
                        }
                    }
                }
            }
            for id in motions.clone() {
                if let Some(m) = self.motions.get(&id) {
                    nodes.extend(m.arrival.nodes().into_iter().flatten());
                    let p = &m.movement;
                    nodes.extend(p.mover());
                    nodes.extend(p.destination());
                    if let Some(checks) = p.base().read().checks.clone() {
                        nodes.extend(checks.read().iter().flatten().copied());
                    }
                }
            }
            for id in tasks.clone() {
                if let Some(t) = self.tasks.get(&id) {
                    nodes.extend(t.body.nodes().into_iter().flatten());
                    if let Some(checks) = t.base.read().checks.clone() {
                        nodes.extend(checks.read().iter().flatten().copied());
                    }
                }
            }
            if old == (nodes.len(), motions.len(), tasks.len()) {
                break;
            }
        }
        self.motions.retain(|id, _| motions.contains(id));
        self.bases.retain(|_, id| motions.contains(id));
        self.tasks.retain(|id, _| tasks.contains(id));
        self.locations.retain(|id, _| nodes.contains(id));
        self.scenery.retain(|id, _| nodes.contains(id));
        self.scenery_ids.retain(|_, node| nodes.contains(node));
        self.grounds.retain(|id, _| nodes.contains(id));
        self.ground_ids.retain(|_, node| nodes.contains(node));
        self.actors.retain(|id, _| nodes.contains(id));
        self.order.retain(|id| self.actors.contains_key(id));
        self.attribute_motions.retain(|id, _| nodes.contains(id));
        self.owned_children.retain(|id, _| nodes.contains(id));
        players.reap_objects(&nodes);
        // Admission only performs one indexed lookup. Reclaim expired weak
        // entries once per collection, not with an O(n) sweep on every login.
        self.identity.prune_processors();
    }
}
#[derive(Clone)]
pub struct NodeHandle {
    pub node: Node,
    _lease: Shared<()>,
}
impl NodeHandle {
    pub(crate) fn internal_ref(&self) -> Self {
        Self {
            node: self.node,
            _lease: self._lease.unowned(),
        }
    }
}
impl PartialEq for NodeHandle {
    fn eq(&self, other: &Self) -> bool {
        self._lease.same(&other._lease)
    }
}
impl Eq for NodeHandle {}
pub struct World<'a, S: Services> {
    pub state: &'a mut State,
    pub services: &'a mut S,
}
impl<S: Services> World<'_, S> {
    /// Construct the original default option's plain Pulse(1, player, node).
    /// Its immutable captures are independent of the mutable check array.
    pub fn delayed_interaction(
        &mut self,
        task: legacy_interaction::Task,
    ) -> Result<pulses::Handle> {
        if task.kind != legacy_interaction::Kind::Delayed {
            return Err(Error::Unsupported("non-delayed plain interaction task"));
        }
        let base = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(vec![task.player, task.target])),
            pulses::Class::Other,
            task.class_name().into(),
        ));
        self.register_task(base.clone(), TaskBody::Interaction(task));
        Ok(base)
    }
    fn register_task(&mut self, base: pulses::Handle, body: TaskBody) {
        if let Some(checks) = base.read().checks.clone() {
            self.state
                .check_arrays
                .insert(checks.identity(), checks.downgrade());
        }
        self.state
            .tasks
            .insert(base.identity(), Task { base, body });
    }
    pub fn run_task_captured(
        &mut self,
        node: Node,
        manager: pulses::Manager,
        base: &pulses::Handle,
    ) -> Result<()> {
        if !self
            .state
            .tasks
            .get(&base.identity())
            .is_some_and(|t| t.base.same(base))
        {
            return Err(Error::Unsupported("task from another world"));
        }
        let runner = self.state.runner.clone();
        manager
            .run(
                Some(base),
                Some(pulses::Kind::Standard),
                &runner,
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: Some(node),
                },
            )
            .map_err(Error::Pulse)
    }
    pub fn clear_standard(&mut self, node: Node, manager: pulses::Manager) -> Result<()> {
        manager
            .clear_kind(
                Some(pulses::Kind::Standard),
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: Some(node),
                },
            )
            .map(|_| ())
            .map_err(Error::Pulse)
    }
    pub fn unhandled_captured(
        &mut self,
        node: Node,
        manager: pulses::Manager,
        player: Node,
    ) -> Result<()> {
        let base = pulses::unhandled_pulse(player);
        self.register_task(base.clone(), TaskBody::Unhandled(player));
        self.run_task_captured(node, manager, &base)
    }
    /// Construct only; the original caller schedules the result separately.
    /// The handler used for destination selection is captured now, while the
    /// option object itself remains shared for the later arrival callback.
    pub fn interaction_motion(&mut self, approach: Approach) -> Result<Movement> {
        let handler = match approach.kind {
            ApproachKind::Invalid => None,
            ApproachKind::Option => approach.option.as_ref().and_then(|o| o.read().handler),
        };
        self.interaction_motion_captured(approach, handler)
    }
    /// Honor the handler argument evaluated by an interaction factory before its
    /// constructor callback. Re-reading the option here could select a new handler.
    pub fn interaction_motion_captured(
        &mut self,
        approach: Approach,
        handler: Option<usize>,
    ) -> Result<Movement> {
        let constructor = match approach.kind {
            ApproachKind::Invalid => movement::Constructor::Basic,
            ApproachKind::Option => movement::Constructor::Option(handler),
        };
        self.construct_target(
            approach.player,
            approach.target,
            false,
            None,
            constructor,
            Arrival::Interaction(approach),
        )
    }
    pub fn run_motion(&mut self, node: Node, motion: Movement) -> Result<()> {
        let manager = self.state.actor(node)?.manager.clone();
        self.run_motion_captured(node, manager, motion)
    }
    /// A factory captures the original manager before constructing a pulse.
    /// Keep that receiver even if a callback changes the actor's current manager.
    /// The binding retains its owner node lease while this call is pending.
    pub fn run_motion_captured(
        &mut self,
        node: Node,
        manager: pulses::Manager,
        motion: Movement,
    ) -> Result<()> {
        let registered = self.state.motion(&motion.base())?;
        if !registered.state.same(&motion.state) {
            return Err(Error::Unsupported("movement from another world"));
        }
        self.schedule_with_manager(node, manager, motion)
    }
    pub fn manager_running(&mut self, node: Node) -> Result<bool> {
        let manager = self.state.actor(node)?.manager.clone();
        manager
            .has_running(
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: Some(node),
                },
            )
            .map_err(Error::Pulse)
    }
    pub fn return_to_spawn(&mut self, node: Node, location: Option<Location>) -> Result<usize> {
        let p = self.construct_motion(
            node,
            location,
            false,
            Some(movement::SMART),
            Arrival::NpcReturn(node),
        )?;
        Ok(p.id())
    }
    pub fn submit(&mut self, motion: usize) -> Result<()> {
        self.state
            .runner
            .submit(Some(self.state.movement(motion)?.base()))
            .map_err(Error::Pulse)
    }
    /// Apply a private transient path as original Path.walk does. Shared mutable
    /// public Path/Deque iteration is a separate contract; this API owns its path.
    pub fn walk_path(&mut self, node: Node, path: pathfinding::Path) -> Result<()> {
        if self.query(Query::Locked(node))? {
            return Ok(());
        }
        movement::Context::walk_reset(self, None, node, None).map_err(Error::Movement)?;
        let q = self.queue(node)?;
        for [x, y] in path.points {
            navigation::add(&mut Walking { world: self, node }, &q, x, y)
                .map_err(Error::Movement)?;
        }
        Ok(())
    }
    pub fn find_path(
        &mut self,
        node: Node,
        destination: Location,
        finder: usize,
        near: bool,
    ) -> Result<pathfinding::Path> {
        self.find_target_path(node, Target::Location(destination), finder, near)
    }
    pub fn find_node_path(&mut self, node: Node, target: Node) -> Result<pathfinding::Path> {
        self.find_target_path(node, Target::Node(target), movement::SMART, true)
    }
    pub fn find_water_path(
        &mut self,
        node: Node,
        destination: Location,
    ) -> Result<pathfinding::Path> {
        self.search_path(|nav, routing| nav.find_water(routing, node, destination))
    }
    fn find_target_path(
        &mut self,
        node: Node,
        target: Target,
        finder: usize,
        near: bool,
    ) -> Result<pathfinding::Path> {
        self.search_path(|nav, routing| nav.find(routing, Some(node), target, Some(finder), near))
    }
    fn search_path(
        &mut self,
        find: impl FnOnce(
            &mut navigation::Navigator,
            &mut Routing<'_, '_, S>,
        ) -> pathfinding::Result<pathfinding::Path>,
    ) -> Result<pathfinding::Path> {
        let mut nav = self
            .state
            .navigator
            .take()
            .ok_or(Error::Unsupported("recursive navigation search"))?;
        nav.bfs = self.state.bfs;
        nav.max_distance = self.state.max_distance;
        let result = find(&mut nav, &mut Routing(self));
        self.state.navigator = Some(nav);
        result.map_err(|e| Error::Binding(e.to_string()))
    }
    pub fn process(
        &mut self,
        node: Node,
        action: &rs09_protocol::game::actions::ClientAction,
    ) -> Result<bool> {
        let request = walk_request::Request::from_action(node, action);
        let recognized = request.is_some();
        walk_request::process(self, request).map_err(Error::Request)?;
        Ok(recognized)
    }
    pub fn pulses(&mut self) -> Result<()> {
        let runner = self.state.runner.clone();
        runner
            .update_all(
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: None,
                },
            )
            .map_err(Error::Pulse)
    }
    pub fn entity(&mut self, node: Node) -> Result<()> {
        tick::entity(&mut EntityBridge { world: self, node })
    }
    pub fn entity_before_masks(&mut self, node: Node) -> Result<()> {
        tick::entity_before_masks(&mut EntityBridge { world: self, node })
    }
    pub fn prepare_masks(&mut self, node: Node) -> Result<()> {
        self.emit(Event::Masks(node))
    }
    /// `entity.getPulseManager().clear()`: the actor's current manager,
    /// without disconnect's flags.
    pub fn clear_pulses(&mut self, node: Node) -> Result<()> {
        let manager = self.state.actor(node)?.manager.clone();
        manager
            .clear(
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: Some(node),
                },
            )
            .map_err(Error::Pulse)
    }
    pub fn disconnect(&mut self, node: Node) -> Result<()> {
        {
            let a = self.state.actor_mut(node)?;
            a.connected = false;
            a.view.active = false;
        }
        let manager = self.state.actor(node)?.manager.clone();
        manager
            .clear(
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: Some(node),
                },
            )
            .map_err(Error::Pulse)
    }
    fn emit(&mut self, event: Event<'_>) -> Result<()> {
        self.services.event(self.state, event)
    }
    fn query(&mut self, query: Query<'_>) -> Result<bool> {
        self.services.query(self.state, query)
    }
    fn queue(&self, node: Node) -> Result<Queue> {
        Ok(self.state.actor(node)?.queue.clone())
    }
    fn construct(&mut self, node: Node, location: Location, running: bool) -> Result<Movement> {
        self.construct_motion(
            node,
            Some(location),
            running,
            None,
            Arrival::Player(node, running),
        )
    }
    fn construct_motion(
        &mut self,
        node: Node,
        location: Option<Location>,
        running: bool,
        finder: Option<usize>,
        arrival: Arrival,
    ) -> Result<Movement> {
        let target = location.map(|location| {
            let target = self.state.allocate_node();
            self.state.locations.insert(target, location);
            target
        });
        self.construct_target(
            node,
            target,
            running,
            finder,
            movement::Constructor::Basic,
            arrival,
        )
    }
    fn construct_target(
        &mut self,
        node: Node,
        target: Option<Node>,
        running: bool,
        finder: Option<usize>,
        constructor: movement::Constructor,
        arrival: Arrival,
    ) -> Result<Movement> {
        self.state.next_motion = self
            .state
            .next_motion
            .checked_add(1)
            .expect("movement identity exhausted");
        let id = self.state.next_motion;
        let p = Movement::new(self, id, Some(node), target, finder, running, constructor)
            .map_err(Error::Movement)?;
        p.base().write().class_name = match &arrival {
            Arrival::Player(..) => "core.net.packet.PacketProcessor$processWalkPacket$1",
            Arrival::NpcReturn(..) => "core.game.node.entity.npc.NPC$1",
            Arrival::Interaction(a) => match a.kind {
                ApproachKind::Invalid => "core.game.interaction.InteractPlugin$2",
                ApproachKind::Option => "core.game.interaction.InteractPlugin$3",
            },
        }
        .into();
        self.state.bases.insert(p.base().identity(), id);
        if let Some(checks) = p.base().read().checks.clone() {
            self.state
                .check_arrays
                .insert(checks.identity(), checks.downgrade());
        }
        self.state.motions.insert(
            id,
            Motion {
                movement: p.clone(),
                arrival,
            },
        );
        Ok(p)
    }
    fn schedule(&mut self, node: Node, p: Movement) -> Result<()> {
        let manager = self.state.actor(node)?.manager.clone();
        self.schedule_with_manager(node, manager, p)
    }
    fn schedule_with_manager(
        &mut self,
        node: Node,
        manager: pulses::Manager,
        p: Movement,
    ) -> Result<()> {
        let runner = self.state.runner.clone();
        manager
            .run(
                Some(&p.base()),
                Some(pulses::Kind::Standard),
                &runner,
                self.state,
                &mut PulseBridge {
                    services: self.services,
                    owner: Some(node),
                },
            )
            .map_err(Error::Pulse)
    }
}
fn merror(e: Error) -> movement::Error {
    match e {
        Error::Null => movement::Error::Null,
        Error::Movement(e) => e,
        _ => movement::Error::Binding(e.to_string()),
    }
}
fn werror(e: Error) -> walking::Error {
    match e {
        Error::Null => walking::Error::Null,
        Error::Walking(e) => e,
        _ => walking::Error::Binding(e.to_string()),
    }
}
fn perror(e: Error) -> pulses::Error {
    match e {
        Error::Null => pulses::Error::Null,
        Error::Pulse(e) => e,
        _ => pulses::Error::Binding(e.to_string()),
    }
}
fn rerror(e: Error) -> walk_request::Error {
    match e {
        Error::Null => walk_request::Error::Null,
        Error::Request(e) => e,
        _ => walk_request::Error::Binding(e.to_string()),
    }
}
fn ferror(e: Error) -> pathfinding::Error {
    match e {
        Error::Null => pathfinding::Error::Null,
        _ => pathfinding::Error::Binding(e.to_string()),
    }
}
impl<S: Services> walk_request::Context for World<'_, S> {
    type Pulse = Movement;
    fn location(&self, n: Node) -> walk_request::Result<Option<Location>> {
        Ok(self.state.actor(n).map_err(rerror)?.queue.entity().location)
    }
    fn movement_locked(&mut self, n: Node) -> walk_request::Result<bool> {
        self.query(Query::Locked(n)).map_err(rerror)
    }
    fn interfaces(&self, n: Node) -> walk_request::Result<walk_request::Interfaces> {
        self.services.interfaces(self.state, n).map_err(rerror)
    }
    fn close_interface(&mut self, n: Node) -> walk_request::Result<bool> {
        self.query(Query::CloseInterface(n)).map_err(rerror)
    }
    fn close_chatbox(&mut self, n: Node) -> walk_request::Result<()> {
        self.emit(Event::CloseChatbox(n)).map_err(rerror)
    }
    fn close_dialogue(&mut self, n: Node) -> walk_request::Result<bool> {
        self.query(Query::CloseDialogue(n)).map_err(rerror)
    }
    fn debug(&mut self, n: Node, s: &str) -> walk_request::Result<()> {
        self.emit(Event::Debug(n, s)).map_err(rerror)
    }
    fn clear_minimap(&mut self, n: Node) -> walk_request::Result<()> {
        self.emit(Event::Minimap(n)).map_err(rerror)
    }
    fn set_running(&mut self, n: Node, b: bool) -> walk_request::Result<()> {
        self.queue(n).map_err(rerror)?.state.write().running = b;
        Ok(())
    }
    fn face_none(&mut self, n: Node) -> walk_request::Result<()> {
        self.emit(Event::Face(n, None)).map_err(rerror)
    }
    fn face_location_none(&mut self, n: Node) -> walk_request::Result<()> {
        self.emit(Event::FaceLocationNone(n)).map_err(rerror)
    }
    fn reset_scripts(&mut self, n: Node) -> walk_request::Result<()> {
        self.emit(Event::Scripts(n, Scripts::Reset)).map_err(rerror)
    }
    fn construct(&mut self, n: Node, l: Location, r: bool) -> walk_request::Result<Movement> {
        World::construct(self, n, l, r).map_err(rerror)
    }
    fn run(&mut self, n: Node, p: Movement) -> walk_request::Result<()> {
        self.schedule(n, p).map_err(rerror)
    }
    fn max_distance(&self) -> i32 {
        self.state.max_distance
    }
}
impl<S: Services> movement::Context for World<'_, S> {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> movement::Result<i32> {
        self.services.clipping(self.state, z, x, y).map_err(merror)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> movement::Result<i32> {
        movement::Context::clipping(self, z, x, y)
    }
    fn node(&self, n: Node) -> movement::Result<NodeView> {
        self.state.node_view(n).map_err(merror)
    }
    fn region(&mut self, l: &Location) -> movement::Result<Option<bool>> {
        self.services.region(self.state, l).map_err(merror)
    }
    fn npc_finder(&mut self, node: Node) -> movement::Result<Option<usize>> {
        self.services.npc_finder(self.state, node).map_err(merror)
    }
    fn current_movement(&self, n: Node) -> movement::Result<Option<usize>> {
        Ok(self.state.actor(n).map_err(merror)?.current)
    }
    fn stop_previous(&mut self, id: usize) -> movement::Result<()> {
        let mut p = self
            .state
            .motions
            .get(&id)
            .ok_or(movement::Error::Null)?
            .movement
            .clone();
        p.stop(self)
    }
    fn assign_current(&mut self, n: Node, id: usize) -> movement::Result<()> {
        self.state.actor_mut(n).map_err(merror)?.current = Some(id);
        Ok(())
    }
    fn active(&mut self, _: &mut Movement, n: Node) -> movement::Result<bool> {
        self.services.node_active(self.state, n).map_err(merror)
    }
    fn scripts(&mut self, _: &mut Movement, n: Node, weak: bool) -> movement::Result<()> {
        self.emit(Event::Scripts(
            n,
            if weak { Scripts::Weak } else { Scripts::Normal },
        ))
        .map_err(merror)
    }
    fn face(&mut self, _: &mut Movement, n: Node, t: Option<Node>) -> movement::Result<()> {
        self.emit(Event::Face(n, t)).map_err(merror)
    }
    fn pulse(&mut self, p: &mut Movement) -> movement::Result<bool> {
        let record = self
            .state
            .motions
            .get(&p.id())
            .ok_or(movement::Error::Null)?;
        match record.arrival.clone() {
            Arrival::Player(player, running) => {
                walk_request::arrival(self, player, running).map_err(|e| merror(Error::Request(e)))
            }
            Arrival::NpcReturn(node) => self.services.npc_arrival(self.state, node).map_err(merror),
            Arrival::Interaction(approach) => self
                .services
                .interaction_arrival(self.state, &approach)
                .map_err(merror),
        }
    }
    fn destination(
        &mut self,
        mover: Option<Node>,
        target: Option<Node>,
        service: DestinationService,
    ) -> movement::Result<Option<Location>> {
        self.services
            .destination(self.state, mover, target, service)
            .map_err(merror)
    }
    fn find(
        &mut self,
        p: &mut Movement,
        t: Target,
        f: usize,
    ) -> movement::Result<Option<movement::RouteHandle>> {
        let mut nav = self
            .state
            .navigator
            .take()
            .ok_or_else(|| merror(Error::Unsupported("recursive smart search")))?;
        nav.bfs = self.state.bfs;
        nav.max_distance = self.state.max_distance;
        let result = nav.movement(&mut Routing(self), p, t, f);
        self.state.navigator = Some(nav);
        result
    }
    fn walk_reset(
        &mut self,
        _: Option<&mut Movement>,
        n: Node,
        r: Option<bool>,
    ) -> movement::Result<()> {
        let q = self.queue(n).map_err(merror)?;
        navigation::reset(
            &mut Walking {
                world: self,
                node: n,
            },
            &q,
            r,
        )
    }
    fn walk_add(&mut self, _: &mut Movement, n: Node, x: i32, y: i32) -> movement::Result<()> {
        let q = self.queue(n).map_err(merror)?;
        navigation::add(
            &mut Walking {
                world: self,
                node: n,
            },
            &q,
            x,
            y,
        )
    }
    fn message(&mut self, _: &mut Movement, n: Node, s: &str) -> movement::Result<()> {
        self.emit(Event::Message(n, s)).map_err(merror)
    }
    fn clear_minimap(&mut self, _: &mut Movement, n: Node) -> movement::Result<()> {
        self.emit(Event::Minimap(n)).map_err(merror)
    }
    fn exception(&mut self, _: &mut Movement, e: &movement::Error) -> movement::Result<()> {
        self.emit(Event::Exception(&e.to_string())).map_err(merror)
    }
    fn hint_clear(&mut self, _: &mut Movement, _: Node) -> movement::Result<()> {
        Err(merror(Error::Unsupported("intersection hints")))
    }
    fn hint_add(
        &mut self,
        _: &mut Movement,
        _: Node,
        _: Option<Location>,
        _: i32,
    ) -> movement::Result<()> {
        Err(merror(Error::Unsupported("intersection hints")))
    }
    fn teleport_permitted(&mut self, location: &Location) -> movement::Result<bool> {
        self.services
            .teleport_permitted(self.state, Some(location.clone()))
            .map_err(merror)
    }
    fn door_destination(
        &mut self,
        mover: Option<Node>,
        target: Option<Node>,
    ) -> movement::Result<Option<Location>> {
        self.services
            .door_destination(self.state, mover, target)
            .map_err(merror)
    }
    fn max_distance(&self) -> i32 {
        walk_request::Context::max_distance(self)
    }
}
struct Routing<'a, 'w, S: Services>(&'a mut World<'w, S>);
impl<S: Services> navigation::Context for Routing<'_, '_, S> {
    fn node(&self, n: Node) -> pathfinding::Result<dispatch::View> {
        let v = movement::Context::node(self.0, n).map_err(|e| ferror(Error::Movement(e)))?;
        Ok(dispatch::View {
            kind: match v.kind {
                Kind::Npc => dispatch::Kind::Npc,
                Kind::Player | Kind::Entity => dispatch::Kind::Entity,
                Kind::Scenery => dispatch::Kind::Scenery,
                _ if self.0.state.grounds.contains_key(&n) => dispatch::Kind::GroundItem,
                _ => dispatch::Kind::Other,
            },
            location: v.location,
            size: v.size,
            scenery: (v.kind == Kind::Scenery).then_some(dispatch::Scenery {
                kind: v.scenery_kind,
                rotation: v.rotation,
                definition: Some(dispatch::Definition {
                    size: v.dimensions,
                    walking_flag: v.walking_flag,
                }),
            }),
        })
    }
    fn npc_supplier(&mut self, node: Node) -> pathfinding::Result<Option<dispatch::Supplier>> {
        self.0
            .services
            .npc_supplier(self.0.state, node)
            .map_err(ferror)
    }
    fn teleport_permitted(&mut self, location: Option<Location>) -> pathfinding::Result<bool> {
        self.0
            .services
            .teleport_permitted(self.0.state, location)
            .map_err(ferror)
    }
    fn clipping(
        &mut self,
        s: dispatch::Supplier,
        z: i32,
        x: i32,
        y: i32,
    ) -> pathfinding::Result<i32> {
        let world = &mut self.0;
        match s {
            dispatch::Supplier::World => world.services.clipping(world.state, z, x, y),
            dispatch::Supplier::Water => world.services.water_clipping(world.state, z, x, y),
            dispatch::Supplier::Custom(_) => Err(Error::Unsupported("custom clipping")),
        }
        .map_err(ferror)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> pathfinding::Result<i32> {
        self.0
            .services
            .clipping(self.0.state, z, x, y)
            .map_err(ferror)
    }
}
struct Walking<'a, 'w, S: Services> {
    world: &'a mut World<'w, S>,
    node: Node,
}
impl<S: Services> walking::Context for Walking<'_, '_, S> {
    fn timer(&mut self, _: &mut Queue, s: &str) -> walking::Result<bool> {
        self.world.query(Query::Timer(self.node, s)).map_err(werror)
    }
    fn marathon(&mut self, _: &mut Queue) -> walking::Result<bool> {
        self.world.query(Query::Marathon(self.node)).map_err(werror)
    }
    fn energy(&mut self, _: &mut Queue, d: f64) -> walking::Result<()> {
        self.world.emit(Event::Energy(self.node, d)).map_err(werror)
    }
    fn run_toggled(&mut self, _: &mut Queue, b: bool) -> walking::Result<()> {
        self.world
            .emit(Event::RunToggled(self.node, b))
            .map_err(werror)
    }
    fn zone_move(&mut self, _: &mut Queue, a: Location, b: Location) -> walking::Result<bool> {
        self.world
            .query(Query::ZoneMove(self.node, a, b))
            .map_err(werror)
    }
    fn region_move(
        &mut self,
        _: &mut Queue,
        a: Option<Location>,
        b: Option<Location>,
    ) -> walking::Result<()> {
        self.world
            .emit(Event::RegionMove(self.node, a, b))
            .map_err(werror)
    }
    fn clear_pulses(&mut self, _: &mut Queue) -> walking::Result<()> {
        let manager = self
            .world
            .state
            .actor(self.node)
            .map_err(werror)?
            .manager
            .clone();
        manager
            .clear(
                self.world.state,
                &mut PulseBridge {
                    services: self.world.services,
                    owner: Some(self.node),
                },
            )
            .map_err(|e| werror(Error::Pulse(e)))
    }
    fn route_create(&mut self, _: &mut Queue, _: Location) -> walking::Result<walking::RouteItem> {
        Err(werror(Error::Unsupported("route debug items")))
    }
    fn route_add(&mut self, _: &mut Queue, _: walking::RouteItem) -> walking::Result<()> {
        Err(werror(Error::Unsupported("route debug items")))
    }
    fn route_remove(&mut self, _: &mut Queue, _: walking::RouteItem) -> walking::Result<()> {
        Err(werror(Error::Unsupported("route debug items")))
    }
    fn null_location(&mut self, _: &mut Queue) -> walking::Result<()> {
        self.world
            .emit(Event::Debug(
                self.node,
                "The entity location provided was null.",
            ))
            .map_err(werror)
    }
}
struct EntityBridge<'a, 'w, S: Services> {
    world: &'a mut World<'w, S>,
    node: Node,
}
impl<S: Services> tick::EntityContext for EntityBridge<'_, '_, S> {
    type Error = Error;
    fn pre_movement(&mut self) -> Result<()> {
        self.world.emit(Event::Scripts(self.node, Scripts::Pre))
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.world.state.ticks)
    }
    fn tick_event(&mut self, t: i32) -> Result<()> {
        self.world.emit(Event::TickEvent(self.node, t))
    }
    fn skills(&mut self) -> Result<()> {
        self.world.emit(Event::Skills(self.node))
    }
    fn location(&self) -> Option<Location> {
        self.world
            .state
            .actors
            .get(&self.node)
            .and_then(|a| a.queue.entity().location)
    }
    fn walking(&mut self) -> Result<()> {
        let mut q = self.world.queue(self.node)?;
        q.update(&mut Walking {
            world: self.world,
            node: self.node,
        })
        .map_err(Error::Walking)
    }
    fn post_movement(&mut self, b: bool) -> Result<()> {
        self.world.emit(Event::Scripts(self.node, Scripts::Post(b)))
    }
    fn timers(&mut self) -> Result<()> {
        self.world.emit(Event::Timers(self.node))
    }
    fn prepare_masks(&mut self) -> Result<()> {
        self.world.prepare_masks(self.node)
    }
}
struct PulseBridge<'a, S: Services> {
    services: &'a mut S,
    owner: Option<Node>,
}
impl<S: Services> pulses::Context<State> for PulseBridge<'_, S> {
    fn pulse(&mut self, pulse: &pulses::Handle, state: &mut State) -> pulses::Result<bool> {
        let Some(task) = state.tasks.get(&pulse.identity()) else {
            return self.services.external_pulse(state, pulse);
        };
        let body = task.body.clone();
        match body {
            TaskBody::Interaction(task) => self
                .services
                .delayed_interaction(state, &task)
                .map_err(perror),
            TaskBody::Unhandled(player) => {
                self.send_message(player, legacy_interaction::NOTHING, state)?;
                Ok(true)
            }
        }
    }
    fn node_active(&mut self, n: Node, state: &mut State) -> pulses::Result<bool> {
        self.services.node_active(state, n).map_err(perror)
    }
    fn has_inactive(&mut self, p: &pulses::Handle, state: &mut State) -> pulses::Result<bool> {
        if state.tasks.contains_key(&p.identity()) || state.bases.contains_key(&p.identity()) {
            pulses::has_inactive_base(p, state, self)
        } else {
            self.services.external_inactive(state, p)
        }
    }
    fn exception(&mut self, e: &pulses::Error, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::Exception(&format!("{e:?}")))
            .map_err(perror)
    }
    fn stop(&mut self, p: &pulses::Handle, state: &mut State) -> pulses::Result<()> {
        if state.tasks.contains_key(&p.identity()) {
            pulses::stop_base(p);
            return Ok(());
        }
        let Some(id) = state.bases.get(&p.identity()) else {
            return self.services.external_stop(state, p);
        };
        let mut m = state.movement(*id).map_err(perror)?;
        m.stop(&mut state.context(self.services))
            .map_err(|e| perror(Error::Movement(e)))
    }
    fn update(&mut self, p: &pulses::Handle, state: &mut State) -> pulses::Result<bool> {
        if !state.bases.contains_key(&p.identity()) {
            return pulses::update_base(p, state, self);
        }
        let mut m = state.motion(p).map_err(perror)?;
        m.update(&mut state.context(self.services))
            .map_err(|e| perror(Error::Movement(e)))
    }
    fn remove_weak_scripts(&mut self, state: &mut State) -> pulses::Result<()> {
        let n = self.owner.ok_or(pulses::Error::Null)?;
        self.services
            .event(state, Event::Scripts(n, Scripts::Weak))
            .map_err(perror)
    }
    fn remove_normal_scripts(&mut self, state: &mut State) -> pulses::Result<()> {
        let n = self.owner.ok_or(pulses::Error::Null)?;
        self.services
            .event(state, Event::Scripts(n, Scripts::Normal))
            .map_err(perror)
    }
    fn is_dead(&mut self, _: &mut State) -> pulses::Result<bool> {
        Err(perror(Error::Unsupported("death state")))
    }
    fn send_message(&mut self, n: Node, s: &str, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::Message(n, s))
            .map_err(perror)
    }
    fn now_millis(&mut self, _: &mut State) -> pulses::Result<i64> {
        Ok(self.services.now_millis())
    }
    fn log(&mut self, l: pulses::Level, s: String, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::Log(l, &s))
            .map_err(perror)
    }
    fn pulse_length(&mut self, s: &str, t: i32, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::PulseLength(s, t))
            .map_err(perror)
    }
    fn count_pulse(&mut self, s: &str, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::CountPulse(s))
            .map_err(perror)
    }
    fn metrics_enabled(&self, _: &State) -> bool {
        self.services.metrics_enabled()
    }
    fn set_bot_total(&mut self, t: i32, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::BotTotal(t))
            .map_err(perror)
    }
    fn set_other_total(&mut self, t: i32, state: &mut State) -> pulses::Result<()> {
        self.services
            .event(state, Event::OtherTotal(t))
            .map_err(perror)
    }
}
