//! Physical component/callback capture graph. Each header is inspected once;
//! incoming fields count separately, aliases do not duplicate captured fields.
use super::Graph;
use crate::{
    definitions::item::Shared,
    inventory::container::Container,
    player::{
        attributes::{AttributeObject, AttributeValue},
        bank_pin,
    },
    world::{
        component_objects::{Captures, Close, Plugin},
        interfaces,
        player_walk::{Node, NodeHandle},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
type Key = (u8, usize);
#[derive(Clone)]
enum Object {
    Manager(Shared<interfaces::Fields>),
    Tabs(Shared<Vec<Option<interfaces::Handle>>>),
    Component(interfaces::Handle),
    Definition(interfaces::Def),
    Plugin(Plugin),
    Close(Close),
    Pin(bank_pin::Manager),
    Attribute(Arc<dyn AttributeObject>),
    Processor(crate::scripts::Runtime),
    Script(crate::scripts::RuntimeScript),
    ActionLocks(crate::world::action_locks::ActionLocks),
    Lock(crate::world::action_locks::Lock),
    LockMap(crate::world::action_locks::Custom),
}
impl Object {
    fn key(&self) -> Key {
        match self {
            Self::Manager(v) => (0, v.identity()),
            Self::Tabs(v) => (1, v.identity()),
            Self::Component(v) => (2, v.identity()),
            Self::Definition(v) => (3, v.identity()),
            Self::Plugin(v) => (4, v.identity()),
            Self::Close(v) => (4, v.identity()),
            Self::Pin(v) => (6, v.ownership().0),
            // Plugin/Close.value<T>() can expose this exact Arc as another
            // trait object. Its allocation and physical fields are still one
            // object, regardless of the view used to reach it.
            Self::Attribute(v) => (4, Arc::as_ptr(v) as *const () as usize),
            Self::Processor(v) => (8, v.fields.identity()),
            Self::Script(v) => (9, v.identity()),
            Self::ActionLocks(v) => (10, v.fields.identity()),
            Self::Lock(v) => (11, v.fields.identity()),
            Self::LockMap(v) => (12, v.identity()),
        }
    }
    fn strong(&self) -> usize {
        match self {
            Self::Manager(v) => v.strong_count(),
            Self::Tabs(v) => v.strong_count(),
            Self::Component(v) => v.strong_count(),
            Self::Definition(v) => v.strong_count(),
            Self::Plugin(v) => v.strong_count(),
            Self::Close(v) => v.strong_count(),
            Self::Pin(v) => v.ownership().1,
            Self::Attribute(v) => Arc::strong_count(v),
            Self::Processor(v) => v.fields.strong_count(),
            Self::Script(v) => v.strong_count(),
            Self::ActionLocks(v) => v.fields.strong_count(),
            Self::Lock(v) => v.fields.strong_count(),
            Self::LockMap(v) => v.strong_count(),
        }
    }
}
#[derive(Default)]
pub(super) struct Components {
    objects: BTreeMap<Key, (Object, usize)>,
    pending: Vec<Key>,
    edges: BTreeMap<Key, BTreeSet<Key>>,
    leaves: BTreeMap<Key, BTreeSet<Node>>,
    owners: BTreeMap<Node, BTreeSet<Key>>,
    container_edges: BTreeMap<Key, BTreeSet<usize>>,
    pub(super) containers: BTreeMap<usize, (Container, usize)>,
    foreign: BTreeSet<Key>,
}
impl Components {
    pub(super) fn seed_action_locks(
        &mut self,
        owner: Node,
        locks: &crate::world::action_locks::ActionLocks,
    ) {
        let key = self.retain(Object::ActionLocks(locks.clone()));
        self.owners.entry(owner).or_default().insert(key);
    }
    pub(super) fn seed_processor(&mut self, owner: Node, processor: &crate::scripts::Runtime) {
        let key = self.retain(Object::Processor(processor.clone()));
        self.owners.entry(owner).or_default().insert(key);
    }
    pub(super) fn seed_attributes<'a>(
        &mut self,
        owner: Node,
        values: impl Iterator<Item = &'a AttributeValue>,
    ) {
        for value in values {
            if let AttributeValue::Object(object) = value {
                let key = self.retain(Object::Attribute(object.clone()));
                self.owners.entry(owner).or_default().insert(key);
            }
        }
    }
    fn retain(&mut self, object: Object) -> Key {
        let key = object.key();
        match self.objects.entry(key) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert((object, 1));
                self.pending.push(key);
            }
            std::collections::btree_map::Entry::Occupied(mut e) => e.get_mut().1 += 1,
        }
        key
    }
    fn edge(&mut self, from: Key, object: Object) {
        let key = self.retain(object);
        self.edges.entry(from).or_default().insert(key);
    }
    pub(super) fn seed(
        &mut self,
        owner: Node,
        manager: &interfaces::Manager,
        pin: &bank_pin::Manager,
    ) {
        let key = self.retain(Object::Manager(manager.fields.clone()));
        self.owners.entry(owner).or_default().insert(key);
        let key = self.retain(Object::Pin(pin.clone()));
        self.owners.entry(owner).or_default().insert(key);
    }
    fn nodes(
        &mut self,
        source: Key,
        nodes: impl IntoIterator<Item = NodeHandle>,
        local: &impl Fn(&NodeHandle) -> bool,
        graph: &mut Graph,
    ) {
        for node in nodes {
            if local(&node) {
                *graph.internal_leases.entry(node.node).or_default() += 1;
                self.leaves.entry(source).or_default().insert(node.node);
            }
        }
    }
    fn captures(
        &mut self,
        key: Key,
        captures: Captures,
        local: &impl Fn(&NodeHandle) -> bool,
        graph: &mut Graph,
    ) {
        self.nodes(key, captures.nodes, local, graph);
        for object in captures.objects {
            self.edge(key, Object::Plugin(object));
        }
        for object in captures.attributes {
            self.edge(key, Object::Attribute(object));
        }
        for object in captures.processors {
            self.edge(key, Object::Processor(object));
        }
        for object in captures.scripts {
            self.edge(key, Object::Script(object));
        }
        for object in captures.action_locks {
            self.edge(key, Object::ActionLocks(object));
        }
        for object in captures.locks {
            self.edge(key, Object::Lock(object));
        }
        for object in captures.lock_maps {
            self.edge(key, Object::LockMap(object));
        }
        for pin in captures.pins {
            self.edge(key, Object::Pin(pin));
        }
        for container in captures.containers {
            let id = container.identity();
            self.container_edges.entry(key).or_default().insert(id);
            match self.containers.entry(id) {
                std::collections::btree_map::Entry::Vacant(e) => {
                    e.insert((container, 1));
                }
                std::collections::btree_map::Entry::Occupied(mut e) => e.get_mut().1 += 1,
            }
        }
    }
    pub(super) fn expand(
        &mut self,
        local: &impl Fn(&NodeHandle) -> bool,
        domain: &crate::world::player_walk::Identity,
        graph: &mut Graph,
    ) {
        while let Some(key) = self.pending.pop() {
            let object = self.objects[&key].0.clone();
            match object {
                Object::Manager(fields) => {
                    let (component, tabs) = {
                        let f = fields.read();
                        (
                            [
                                f.windows_pane.clone(),
                                f.opened.clone(),
                                f.chatbox.clone(),
                                f.single_tab.clone(),
                                f.overlay.clone(),
                                f.wildy_overlay.clone(),
                            ],
                            f.tabs.clone(),
                        )
                    };
                    self.nodes(
                        key,
                        interfaces::Manager {
                            fields: fields.clone(),
                        }
                        .player(),
                        local,
                        graph,
                    );
                    for c in component.into_iter().flatten() {
                        self.edge(key, Object::Component(c));
                    }
                    if let Some(tabs) = tabs {
                        self.edge(key, Object::Tabs(tabs));
                    }
                }
                Object::Tabs(tabs) => {
                    let entries = tabs.read().clone();
                    for c in entries.into_iter().flatten() {
                        self.edge(key, Object::Component(c));
                    }
                }
                Object::Component(component) => {
                    let (definition, plugin, event) = {
                        let f = component.read();
                        (
                            f.definition.clone(),
                            f.plugin.clone(),
                            f.close_event.clone(),
                        )
                    };
                    if let Some(d) = definition {
                        self.edge(key, Object::Definition(d));
                    }
                    if let Some(p) = plugin {
                        self.edge(key, Object::Plugin(p));
                    }
                    if let Some(e) = event {
                        self.edge(key, Object::Close(e));
                    }
                }
                Object::Definition(definition) => {
                    let plugin = definition.read().plugin.clone();
                    if let Some(plugin) = plugin {
                        self.edge(key, Object::Plugin(plugin));
                    }
                }
                Object::Plugin(plugin) => self.captures(key, plugin.captures(), local, graph),
                Object::Close(event) => self.captures(key, event.captures(), local, graph),
                Object::Pin(pin) => self.nodes(key, pin.player(), local, graph),
                Object::Attribute(value) => self.captures(key, value.captures(), local, graph),
                Object::Processor(processor) => {
                    if !processor.domain().is_some_and(|d| d.same(domain)) {
                        self.foreign.insert(key);
                    }
                    let (nodes, scripts) = processor.stored_fields();
                    self.nodes(key, nodes, local, graph);
                    for script in scripts {
                        self.edge(key, Object::Script(script));
                    }
                }
                Object::Script(script) => {
                    if !script.read().domain().is_some_and(|d| d.same(domain)) {
                        self.foreign.insert(key);
                    }
                    let (execution, form) = {
                        let f = script.read();
                        (f.execution.clone(), f.form.clone())
                    };
                    if let Some(execution) = execution {
                        self.edge(key, Object::Plugin(execution.header()));
                    }
                    match form {
                        crate::scripts::Form::UseWith { used, with, .. }
                        | crate::scripts::Form::QueuedUseWith { used, with, .. } => {
                            self.nodes(key, [used, with], local, graph)
                        }
                        _ => {}
                    }
                }
                Object::ActionLocks(locks) => {
                    if !locks.domain().is_some_and(|d| d.same(domain)) {
                        self.foreign.insert(key);
                    }
                    let (fields, custom) = locks.stored_fields();
                    for lock in fields {
                        self.edge(key, Object::Lock(lock));
                    }
                    self.edge(key, Object::LockMap(custom));
                }
                Object::Lock(lock) => {
                    if !lock.domain().is_some_and(|d| d.same(domain)) {
                        self.foreign.insert(key);
                    }
                    if let Some(event) = lock.elapse() {
                        self.edge(key, Object::Plugin(event));
                    }
                }
                Object::LockMap(map) => {
                    let fields = map.read().values().cloned().collect::<Vec<_>>();
                    for lock in fields {
                        self.edge(key, Object::Lock(lock));
                    }
                }
            }
        }
    }
    pub(super) fn finish(self, containers: impl Fn(usize) -> BTreeSet<Node>, graph: &mut Graph) {
        let mut targets = self.leaves;
        for (key, ids) in self.container_edges {
            for id in ids {
                targets.entry(key).or_default().extend(containers(id));
            }
        }
        loop {
            let mut changed = false;
            for (source, edges) in &self.edges {
                for target in edges {
                    let nodes = targets.get(target).cloned().unwrap_or_default();
                    let entry = targets.entry(*source).or_default();
                    let old = entry.len();
                    entry.extend(nodes);
                    changed |= old != entry.len();
                }
            }
            if !changed {
                break;
            }
        }
        let mut roots = self.foreign;
        for (key, (object, references)) in &self.objects {
            if object.strong() > references + 1 {
                roots.insert(*key);
            }
        }
        for key in &roots {
            graph.roots.extend(targets.get(key).into_iter().flatten());
        }
        for (owner, keys) in &self.owners {
            for key in keys {
                graph
                    .edges
                    .entry(*owner)
                    .or_default()
                    .extend(targets.get(key).into_iter().flatten());
            }
        }
        graph.reaper = Some(Reaper {
            objects: self.objects.into_iter().map(|(k, (v, _))| (k, v)).collect(),
            owners: self.owners,
            edges: self.edges,
            roots,
        });
    }
}
/// Retain inspection objects until the node graph is marked, then sever only
/// unreachable typed script fields. This also collects function→processor→script
/// cycles; externally retained handles remain untouched.
pub(super) struct Reaper {
    objects: BTreeMap<Key, Object>,
    owners: BTreeMap<Node, BTreeSet<Key>>,
    edges: BTreeMap<Key, BTreeSet<Key>>,
    roots: BTreeSet<Key>,
}
impl Reaper {
    pub(super) fn reap(self, nodes: &BTreeSet<Node>) {
        let mut live = self.roots;
        for node in nodes {
            if let Some(keys) = self.owners.get(node) {
                live.extend(keys);
            }
        }
        loop {
            let old = live.len();
            for key in live.clone() {
                if let Some(edges) = self.edges.get(&key) {
                    live.extend(edges);
                }
            }
            if old == live.len() {
                break;
            }
        }
        for (key, object) in &self.objects {
            if !live.contains(key) {
                match object {
                    Object::Processor(p) => p.clear_unreachable(),
                    Object::Script(s) => {
                        let old = {
                            let mut f = s.write();
                            (
                                f.execution.take(),
                                std::mem::replace(&mut f.form, crate::scripts::Form::Basic),
                            )
                        };
                        drop(old);
                    }
                    Object::Lock(lock) => lock.clear_unreachable(),
                    _ => {}
                }
            }
        }
    }
}
