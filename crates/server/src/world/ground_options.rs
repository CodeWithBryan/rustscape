//! GroundItem's real option table, cached item metadata and typed native handler
//! registry. Packet/legacy arrival and complete content startup are separate.
use super::{
    ground_item::{self as ground, Handle as Ground},
    ground_spawn,
    interaction_options::{self as options, Constants, Error, Kind, Names, NamesKind, Result},
    pickup,
    player_walk::{Identity, Node, NodeHandle, State},
};
use crate::definitions::{
    item::{self, Shared},
    option_handler::{self as handlers, Handle as Handler},
};
use std::sync::Arc;
struct Data {
    defaults: handlers::DefaultHandle,
    constants: Constants,
    arena: handlers::Arena,
}
/// One registry per owning world. IDs are never reused or reconstructed from
/// JSON/labels; every bound ID retains the actual native object. The arena is
/// conservatively retained until its registry and all ground bindings drop.
#[derive(Clone)]
pub struct Registry(Arc<Data>);
impl Default for Registry {
    fn default() -> Self {
        Self(Arc::new(Data {
            defaults: Shared::new(handlers::Defaults::default()),
            constants: Constants::default(),
            arena: handlers::Arena::default(),
        }))
    }
}
impl Registry {
    pub fn defaults(&self) -> handlers::DefaultHandle {
        self.0.defaults.clone()
    }
    pub fn register_pickup(&self) -> super::ground_manager::Result<Handler> {
        pickup::register(&mut self.defaults(), pickup::native_handler())
    }
    pub fn bind(&self, handler: Handler) -> Result<usize> {
        self.0.arena.bind(handler).map_err(arena_error)
    }
    pub fn handler(&self, id: usize) -> Result<Handler> {
        self.0.arena.handler(id).map_err(arena_error)
    }
    fn validate(&self, state: &State, binding: &Binding) -> Result<()> {
        if !Arc::ptr_eq(&self.0, &binding.registry.0) || !state.identity().same(&binding.world) {
            return Err(Error::Binding(
                "ground interaction belongs to another world".into(),
            ));
        }
        Ok(())
    }
    /// Install before init: failures leave the same partially initialized table,
    /// just as the original Node's InteractPlugin remains observable on failure.
    /// The caller supplies the actual server default-locale operation.
    pub fn initialize(
        &self,
        state: &mut State,
        spawns: &ground_spawn::Registry,
        definitions: &item::Registry,
        item: &Ground,
        mut lowercase: impl FnMut(&str) -> Result<String>,
    ) -> Result<()> {
        if let Some(binding) = &item.read().interaction {
            self.validate(state, binding)?;
        }
        let target = state
            .ground_target(item.clone(), spawns)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let table = {
            let mut item = item.write();
            item.interaction
                .get_or_insert_with(|| Binding {
                    registry: self.clone(),
                    world: state.identity(),
                    table: options::Table::new(Some(target.node)),
                })
                .table
                .clone()
        };
        if table.read().node() != Some(target.node) {
            return Err(Error::Binding(
                "ground interaction target identity changed".into(),
            ));
        }
        options::set_default(
            &mut Context {
                registry: self,
                definitions,
                item,
                node: target.node,
                lowercase: &mut lowercase,
            },
            &table,
        )
    }
    /// Capture both the canonical node and the exact current table. The exposed
    /// table keeps the actual GroundItem/GroundSpawn alive after manager removal.
    pub fn capture(
        &self,
        state: &mut State,
        spawns: &ground_spawn::Registry,
        item: &Ground,
    ) -> Result<Captured> {
        let binding = item.read().interaction.clone().ok_or(Error::Null)?;
        self.validate(state, &binding)?;
        let target = state
            .ground_target(item.clone(), spawns)
            .map_err(|e| Error::Binding(e.to_string()))?;
        if binding.table.read().node() != Some(target.node) {
            return Err(Error::Binding(
                "ground interaction target identity changed".into(),
            ));
        }
        Ok(Captured {
            target,
            table: binding.table.keeping_from(item),
            registry: self.clone(),
        })
    }
}

#[derive(Clone)]
pub struct Binding {
    registry: Registry,
    world: Identity,
    table: options::Handle,
}
#[derive(Clone)]
pub struct Captured {
    pub target: NodeHandle,
    pub table: options::Handle,
    registry: Registry,
}
impl Captured {
    pub fn null_option(&self) -> super::interaction::OptionHandle {
        self.registry.0.constants.null.clone()
    }
    pub fn handler(&self, id: usize) -> Result<Handler> {
        self.registry.handler(id)
    }
}

struct Lookup<'a>(&'a item::Registry);
impl handlers::Context for Lookup<'_> {
    fn definition(&mut self, id: i32) -> handlers::Result<Option<item::Handle>> {
        Ok(Some(self.0.for_id(id)))
    }
    fn missing_definition(&mut self, _: i32) -> handlers::Result<()> {
        Err(handlers::Error::Binding(
            "nonnull forId fallback became null".into(),
        ))
    }
}
struct Context<'a, L> {
    registry: &'a Registry,
    definitions: &'a item::Registry,
    item: &'a Ground,
    node: Node,
    lowercase: &'a mut L,
}
impl<L: FnMut(&str) -> Result<String>> options::Context for Context<'_, L> {
    type Definition = item::Handle;
    fn kind(&self, node: Option<Node>) -> Kind {
        if node == Some(self.node) {
            Kind::Item
        } else {
            Kind::Other
        }
    }
    fn node_id(&mut self, _: Node) -> Result<i32> {
        Ok(ground::as_item(self.item).read().id())
    }
    fn item_has_location(&mut self, _: Node) -> Result<bool> {
        Ok(self.item.read().location.is_some())
    }
    fn definition(&mut self, _: Node) -> Result<Option<item::Handle>> {
        Ok(ground::as_item(self.item).read().definition.clone())
    }
    fn names(&mut self, definition: &item::Handle, kind: NamesKind) -> Result<Option<Names>> {
        let definition = definition.read();
        Ok(match kind {
            NamesKind::Ground => definition.ground_actions.clone(),
            NamesKind::Inventory => definition.options.clone(),
            NamesKind::Options => return Err(Error::Binding("non-item action array".into())),
        })
    }
    fn lowercase(&mut self, text: &str) -> Result<String> {
        (self.lowercase)(text)
    }
    fn option_handler(&mut self, kind: Kind, id: i32, name: &str) -> Result<Option<usize>> {
        if kind != Kind::Item {
            return Err(Error::Binding("non-item option lookup".into()));
        }
        let handler = handlers::item_handler(
            &mut Lookup(self.definitions),
            &self.registry.defaults(),
            id,
            Some(name),
        )
        .map_err(|e| Error::Binding(e.to_string()))?;
        handler
            .map(|handler| self.registry.bind(handler))
            .transpose()
    }
    fn send_option(&mut self, _: Node, _: i32, _: Option<String>) -> Result<()> {
        Err(Error::Binding(
            "ground table sent a player option packet".into(),
        ))
    }
    fn node_string(&mut self, _: Option<Node>) -> Result<String> {
        Ok(ground::as_item(self.item).read().text())
    }
    fn constants(&self) -> &Constants {
        &self.registry.0.constants
    }
    fn retain_option(
        &self,
        option: super::interaction::OptionHandle,
    ) -> super::interaction::OptionHandle {
        self.registry.0.arena.retain(option)
    }
}
fn arena_error(e: handlers::Error) -> Error {
    match e {
        handlers::Error::Binding(s) => Error::Binding(s),
        handlers::Error::Type => Error::Binding("native handler type".into()),
    }
}
