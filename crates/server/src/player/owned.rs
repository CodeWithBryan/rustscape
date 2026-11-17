//! Canonical fields currently bound by native player startup. Full Player,
//! profiles/content and the remaining concrete container subclasses are pending.
use crate::{
    definitions::item::{Registry, Shared, WeakShared},
    inventory::{
        bank::Bank,
        container::{Container, Listeners},
        container_listener::Handle as Listener,
        player_binding::Capture,
    },
    world::{
        interfaces::Manager,
        player_walk::{Identity, Node, NodeHandle},
        prepared_masks::PreparedMasks,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::Arc,
};
mod appearance;
mod attributes;
mod components;
mod inventory;
mod variables;
pub use attributes::Attributes;
pub use inventory::Services;
pub use variables::{VariableServices, Variables, WorldVariableAssets};

struct Fields {
    world: Identity,
    core: Capture,
    definitions: Arc<Registry>,
    manager: Manager,
    pouch: Shared<bool>,
    masks: PreparedMasks,
    appearance: super::appearance::Appearance,
    attributes: super::attributes::GameAttributes,
    ironman: super::ironman::Manager,
    bank_primary: Bank,
    bank_secondary: Bank,
    drop_log: Bank,
    use_secondary_bank: bool,
    variables: super::variables::Store,
    varp_manager: super::variables::Manager,
    bank_pin: super::bank_pin::Manager,
    scripts: crate::scripts::Runtime,
}
#[derive(Clone)]
pub struct Player(Shared<Fields>);
#[derive(Clone)]
pub struct WeakPlayer(WeakShared<Fields>);
impl WeakPlayer {
    pub fn upgrade(&self) -> Option<Player> {
        self.0.upgrade().map(Player)
    }
}
impl Player {
    pub fn apply<T>(
        &self,
        state: &mut crate::world::player_walk::State,
        frames: &mut BTreeMap<Node, Vec<rs09_protocol::game::Frame>>,
        clock: fn() -> i64,
        operation: impl FnOnce(
            &mut Container,
            &Registry,
            &mut crate::inventory::player_binding::Context<'_, '_, Services<'_>>,
        ) -> crate::inventory::Result<T>,
    ) -> crate::inventory::Result<T> {
        let owner = self.owner();
        if !self.in_world(&state.identity())
            || !state
                .player_fields(&owner)
                .is_ok_and(|fields| fields.same(self))
        {
            return Err(crate::inventory::Error::Binding(
                "player fields belong to another canonical world".into(),
            ));
        }
        let mut services = Services::new(state, frames, self.clone(), clock);
        self.core()
            .apply(&self.definitions(), &mut services, operation)
    }
    pub fn new(
        world: Identity,
        core: Capture,
        definitions: Arc<Registry>,
        manager: Manager,
        pouch: Shared<bool>,
        masks: PreparedMasks,
    ) -> super::appearance::Result<Self> {
        if manager.player().is_some_and(|old| old != core.owner()) {
            return Err(super::appearance::Error::Binding(
                "interface manager already belongs to another player".into(),
            ));
        }
        let appearance =
            super::appearance::Appearance::new(Some(core.owner()), world.appearance_factory())?;
        let ironman = super::ironman::Manager::new(Some(core.owner()));
        let bank_pin = super::bank_pin::Manager::new(Some(core.owner()));
        let scripts = world.processor(core.owner());
        let varp_manager = super::variables::Manager::new(Some(core.owner()))
            .map_err(|e| super::appearance::Error::Binding(format!("varp manager: {e:?}")))?;
        let factory = crate::inventory::container::Factory {
            null_item: core.null_item(),
        };
        let make_bank = || {
            Bank::new(&factory, definitions.clone(), Some(core.owner()))
                .map_err(|e| super::appearance::Error::Binding(format!("bank constructor: {e:?}")))
        };
        let bank_primary = make_bank()?;
        let bank_secondary = make_bank()?;
        let drop_log = make_bank()?;
        manager
            .bind_player(core.owner())
            .map_err(|e| super::appearance::Error::Binding(e.to_string()))?;
        Ok(Self(Shared::new(Fields {
            world,
            core,
            definitions,
            manager,
            pouch,
            masks,
            appearance,
            attributes: Default::default(),
            ironman,
            bank_primary,
            bank_secondary,
            drop_log,
            use_secondary_bank: false,
            variables: Default::default(),
            varp_manager,
            bank_pin,
            scripts,
        })))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn downgrade(&self) -> WeakPlayer {
        WeakPlayer(self.0.downgrade())
    }
    pub fn in_world(&self, world: &Identity) -> bool {
        self.0.read().world.same(world)
    }
    pub fn owner(&self) -> NodeHandle {
        self.0.read().core.owner()
    }
    pub fn core(&self) -> Capture {
        self.0.read().core.keeping_from(&self.0)
    }
    pub fn inventory(&self) -> Container {
        self.core().inventory()
    }
    pub fn equipment(&self) -> Container {
        self.core().equipment()
    }
    pub fn bank(&self) -> Bank {
        let fields = self.0.read();
        if fields.use_secondary_bank {
            fields.bank_secondary.clone()
        } else {
            fields.bank_primary.clone()
        }
    }
    pub fn bank_primary(&self) -> Bank {
        self.0.read().bank_primary.clone()
    }
    pub fn bank_secondary(&self) -> Bank {
        self.0.read().bank_secondary.clone()
    }
    pub fn drop_log(&self) -> Bank {
        self.0.read().drop_log.clone()
    }
    pub fn use_secondary_bank(&self) -> bool {
        self.0.read().use_secondary_bank
    }
    pub fn set_use_secondary_bank(&self, value: bool) {
        self.0.write().use_secondary_bank = value;
    }
    /// Original short-circuit order: inventory, current active bank, equipment.
    pub fn has_item(
        &self,
        item: Option<&crate::inventory::item::Handle>,
    ) -> crate::inventory::Result<bool> {
        super::item_ownership::has_item(item, &mut self.clone())
    }
    pub fn settings(&self) -> super::settings::Settings {
        self.core().settings()
    }
    pub fn appearance(&self) -> super::appearance::Appearance {
        self.0.read().appearance.clone()
    }
    pub fn attributes(&self) -> Attributes {
        Attributes(self.clone())
    }
    pub fn ironman(&self) -> super::ironman::Manager {
        self.0.read().ironman.clone()
    }
    pub fn variables(&self) -> Variables {
        Variables(self.clone())
    }
    pub fn varp_manager(&self) -> super::variables::Manager {
        self.0.read().varp_manager.clone()
    }
    pub fn bank_pin(&self) -> super::bank_pin::Manager {
        self.0.read().bank_pin.clone()
    }
    pub fn scripts(&self) -> crate::scripts::Runtime {
        self.0.read().scripts.clone()
    }
    pub fn render_info(&self) -> Shared<crate::world::player_render::RenderInfo> {
        let info = self.0.read().masks.owner().read().info.clone();
        info.keeping_from(&self.0)
    }
    pub fn prepare_body(
        &self,
        state: &crate::world::player_walk::State,
    ) -> super::appearance::Result<()> {
        let owner = self.owner();
        if !self.in_world(&state.identity())
            || !state.player_fields(&owner).is_ok_and(|p| p.same(self))
        {
            return Err(super::appearance::Error::Binding(
                "appearance belongs to another canonical world".into(),
            ));
        }
        self.appearance()
            .prepare_body(&mut appearance::BodyServices::new(state), Some(&owner))
    }
    pub fn definitions(&self) -> Arc<Registry> {
        self.0.read().definitions.clone()
    }
    pub fn manager(&self) -> Manager {
        self.0.read().manager.clone()
    }
    pub fn pouch(&self) -> Shared<bool> {
        self.0.read().pouch.keeping_from(&self.0)
    }
    pub fn masks(&self) -> PreparedMasks {
        self.0.read().masks.keeping_from(&self.0)
    }
    pub(crate) fn same_masks(&self, other: &Self) -> bool {
        self.0.read().masks.same(&other.0.read().masks)
    }
}
impl super::item_ownership::Containers for Player {
    fn inventory(&mut self) -> crate::inventory::Result<Container> {
        Ok(Player::inventory(self))
    }
    fn bank(&mut self) -> crate::inventory::Result<Container> {
        Ok(Player::bank(self).container())
    }
    fn equipment(&mut self) -> crate::inventory::Result<Container> {
        Ok(Player::equipment(self))
    }
}

/// Owned edges, not extra strong handles. All snapshot handles below drop before
/// State compares canonical node-lease counts or removes unreachable actors.
#[derive(Default)]
pub(crate) struct Graph {
    pub roots: BTreeSet<Node>,
    pub edges: BTreeMap<Node, BTreeSet<Node>>,
    pub internal_leases: BTreeMap<Node, usize>,
    reaper: Option<components::Reaper>,
}
impl Graph {
    pub(crate) fn reap_objects(&mut self, live: &BTreeSet<Node>) {
        if let Some(reaper) = self.reaper.take() {
            reaper.reap(live);
        }
    }
}
struct Masks {
    references: usize,
    strong: usize,
    owners: BTreeSet<Node>,
    nodes: Vec<NodeHandle>,
}
struct Backreference {
    references: usize,
    strong: usize,
    receiver: Option<NodeHandle>,
    owners: BTreeSet<Node>,
}
pub(crate) fn trace<'a>(
    players: impl Iterator<Item = &'a Player>,
    processors: impl Iterator<Item = (Node, &'a crate::scripts::Runtime)>,
    action_locks: impl Iterator<Item = (Node, &'a crate::world::action_locks::ActionLocks)>,
    local: impl Fn(&NodeHandle) -> bool,
    domain: &Identity,
) -> Graph {
    let mut graph = Graph::default();
    let mut containers: BTreeMap<usize, (Container, usize)> = BTreeMap::new();
    let mut owner_containers: BTreeMap<Node, Vec<usize>> = BTreeMap::new();
    let mut masks: BTreeMap<usize, Masks> = BTreeMap::new();
    let mut backreferences: BTreeMap<(u8, usize), Backreference> = BTreeMap::new();
    let mut components = components::Components::default();
    for (owner, processor) in processors {
        components.seed_processor(owner, processor);
    }
    for (owner, locks) in action_locks {
        components.seed_action_locks(owner, locks);
    }
    for player in players {
        let f = player.0.read();
        let (owner, core_strong, fields) = f.core.ownership();
        if !local(&owner) {
            continue;
        }
        let node = owner.node;
        for (kind, (id, strong, receiver)) in [
            (0, f.appearance.ownership()),
            (1, f.ironman.ownership()),
            (2, f.varp_manager.ownership()),
        ] {
            match backreferences.entry((kind, id)) {
                Entry::Vacant(e) => {
                    e.insert(Backreference {
                        references: 1,
                        strong,
                        receiver,
                        owners: BTreeSet::from([node]),
                    });
                }
                Entry::Occupied(mut e) => {
                    let value = e.get_mut();
                    value.references += 1;
                    value.owners.insert(node);
                }
            }
        }
        components.seed(node, &f.manager, &f.bank_pin);
        components.seed_processor(node, &f.scripts);
        components.seed_attributes(
            node,
            f.attributes
                .values
                .values()
                .filter_map(Option::as_ref)
                .chain(f.attributes.null_value.iter().filter_map(Option::as_ref)),
        );
        *graph.internal_leases.entry(node).or_default() += 1;
        if player.0.strong_count() > 1 || core_strong > 1 {
            graph.roots.insert(node);
        }
        for c in fields.into_iter().chain([
            f.bank_primary.container(),
            f.bank_secondary.container(),
            f.drop_log.container(),
        ]) {
            let id = c.identity();
            owner_containers.entry(node).or_default().push(id);
            match containers.entry(id) {
                Entry::Vacant(e) => {
                    e.insert((c, 1));
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().1 += 1;
                }
            }
        }
        let (id, strong, nodes) = f.masks.ownership();
        match masks.entry(id) {
            Entry::Vacant(e) => {
                e.insert(Masks {
                    references: 1,
                    strong,
                    owners: BTreeSet::from([node]),
                    nodes,
                });
            }
            Entry::Occupied(mut e) => {
                let m = e.get_mut();
                m.references += 1;
                m.owners.insert(node);
            }
        }
    }
    components.expand(&local, domain, &mut graph);
    for (id, (container, references)) in std::mem::take(&mut components.containers) {
        match containers.entry(id) {
            Entry::Vacant(e) => {
                e.insert((container, references));
            }
            Entry::Occupied(mut e) => e.get_mut().1 += references,
        }
    }
    // Each actual native backreference object is counted once, even if shared.
    for Backreference {
        references,
        strong,
        receiver,
        owners,
    } in backreferences.into_values()
    {
        if let Some(receiver) = receiver.filter(|n| local(n)) {
            *graph.internal_leases.entry(receiver.node).or_default() += 1;
            if strong > references {
                graph.roots.insert(receiver.node);
            }
            for owner in owners {
                graph.edges.entry(owner).or_default().insert(receiver.node);
            }
        }
    }
    for m in masks.into_values() {
        let mut targets = BTreeSet::new();
        for target in m.nodes {
            if local(&target) {
                *graph.internal_leases.entry(target.node).or_default() += 1;
                targets.insert(target.node);
            }
        }
        if m.strong > m.references {
            graph.roots.extend(&m.owners);
        }
        for owner in m.owners {
            graph.edges.entry(owner).or_default().extend(&targets);
        }
    }
    let mut lists: BTreeMap<usize, (Listeners, usize)> = BTreeMap::new();
    let mut container_lists = BTreeMap::new();
    let mut external_containers = BTreeSet::new();
    let mut container_nodes: BTreeMap<usize, BTreeSet<Node>> = BTreeMap::new();
    let mut objects: BTreeMap<usize, (Listener, usize)> = BTreeMap::new();
    let mut container_objects: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (&id, (c, references)) in &containers {
        if c.strong_count() > references + 1 {
            external_containers.insert(id);
        }
        if let Some(Some(owner)) = c.equipment_owner() {
            if local(&owner) {
                *graph.internal_leases.entry(owner.node).or_default() += 1;
                container_nodes.entry(id).or_default().insert(owner.node);
            }
        }
        if c.is_bank() {
            let bank = Bank::from_container(c.clone()).unwrap();
            if let Some(owner) = bank.owner().filter(|owner| local(owner)) {
                *graph.internal_leases.entry(owner.node).or_default() += 1;
                container_nodes.entry(id).or_default().insert(owner.node);
            }
            // Bank stores its final listener separately from the exposed list.
            let object = bank.listener();
            let key = object.identity();
            container_objects.entry(id).or_default().push(key);
            match objects.entry(key) {
                Entry::Vacant(e) => {
                    e.insert((object, 1));
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().1 += 1;
                }
            }
        }
        let list = c.listener_list();
        let key = list.identity();
        container_lists.insert(id, key);
        match lists.entry(key) {
            Entry::Vacant(e) => {
                e.insert((list, 1));
            }
            Entry::Occupied(mut e) => {
                e.get_mut().1 += 1;
            }
        }
    }
    let mut list_objects: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut external_lists = BTreeSet::new();
    for (&id, (list, references)) in &lists {
        if list.strong_count() > references + 1 {
            external_lists.insert(id);
        }
        for value in list.values().into_iter().flatten() {
            let key = value.identity();
            list_objects.entry(id).or_default().push(key);
            match objects.entry(key) {
                Entry::Vacant(e) => {
                    e.insert((value, 1));
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().1 += 1;
                }
            }
        }
    }
    let mut object_nodes = BTreeMap::new();
    // No player/container/list lock spans native ownership-metadata inspection.
    for (&id, (object, references)) in &objects {
        let mut nodes = BTreeSet::new();
        for owner in object.retained_nodes() {
            if local(&owner) {
                *graph.internal_leases.entry(owner.node).or_default() += 1;
                nodes.insert(owner.node);
            }
        }
        if object.strong_count() > references + 1 {
            graph.roots.extend(&nodes);
        }
        object_nodes.insert(id, nodes);
    }
    let list_nodes: BTreeMap<_, BTreeSet<_>> = lists
        .keys()
        .map(|id| {
            let nodes = list_objects
                .get(id)
                .into_iter()
                .flatten()
                .flat_map(|key| object_nodes[key].iter().copied())
                .collect();
            (*id, nodes)
        })
        .collect();
    for (container, objects) in container_objects {
        for object in objects {
            container_nodes
                .entry(container)
                .or_default()
                .extend(&object_nodes[&object]);
        }
    }
    for id in external_lists {
        graph.roots.extend(&list_nodes[&id]);
    }
    for id in external_containers {
        graph.roots.extend(&list_nodes[&container_lists[&id]]);
        graph
            .roots
            .extend(container_nodes.get(&id).into_iter().flatten());
    }
    components.finish(
        |id| {
            let mut nodes = container_nodes.get(&id).cloned().unwrap_or_default();
            if let Some(list) = container_lists.get(&id) {
                nodes.extend(&list_nodes[list]);
            }
            nodes
        },
        &mut graph,
    );
    for (owner, fields) in owner_containers {
        for id in fields {
            graph
                .edges
                .entry(owner)
                .or_default()
                .extend(container_nodes.get(&id).into_iter().flatten());
            graph
                .edges
                .entry(owner)
                .or_default()
                .extend(&list_nodes[&container_lists[&id]]);
        }
    }
    graph
}
