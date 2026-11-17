//! Scenery's actual InteractPlugin table and per-world native handler bindings.
use super::{
    door_option,
    doors::Handle as Scenery,
    interaction::OptionHandle,
    interaction_options::{self as options, Constants, Error, Kind, Names, NamesKind, Result},
    player_walk::{Identity, Node, NodeHandle, State},
};
use crate::definitions::{
    item::Shared,
    option_handler::{self as handlers, Handle as Handler},
    property::Property,
    scenery as defs,
};
use std::sync::Arc;
struct Data {
    defaults: handlers::DefaultHandle,
    arena: handlers::Arena,
    constants: Constants,
}
#[derive(Clone)]
pub struct Registry(Arc<Data>);
impl Default for Registry {
    fn default() -> Self {
        Self(Arc::new(Data {
            defaults: Shared::new(handlers::Defaults::default()),
            arena: handlers::Arena::default(),
            constants: Constants::default(),
        }))
    }
}
fn error(e: handlers::Error) -> Error {
    match e {
        handlers::Error::Binding(s) => Error::Binding(s),
        handlers::Error::Type => Error::Binding("scenery option handler type".into()),
    }
}
impl Registry {
    pub fn defaults(&self) -> handlers::DefaultHandle {
        self.0.defaults.clone()
    }
    pub fn register_doors(&self) -> Result<Handler> {
        let handler = door_option::native_handler();
        door_option::register_options(|name| {
            handlers::set_option_handler(
                &self.defaults(),
                Some(name.into()),
                Some(handler.clone()),
            );
            Ok::<_, Error>(())
        })?;
        Ok(handler)
    }
    pub fn bind(&self, handler: Handler) -> Result<usize> {
        self.0.arena.bind(handler).map_err(error)
    }
    pub fn handler(&self, id: usize) -> Result<Handler> {
        self.0.arena.handler(id).map_err(error)
    }
    fn validate(&self, state: &State, binding: &Binding) -> Result<()> {
        if !Arc::ptr_eq(&self.0, &binding.registry.0) || !state.identity().same(&binding.world) {
            return Err(Error::Binding(
                "scenery interaction belongs to another world".into(),
            ));
        }
        Ok(())
    }
    pub fn initialize(
        &self,
        state: &mut State,
        definitions: &mut defs::Registry,
        object: &Scenery,
        mut lowercase: impl FnMut(&str) -> Result<String>,
    ) -> Result<()> {
        if let Some(binding) = &object.read().interaction {
            self.validate(state, binding)?;
        }
        let target = state
            .scenery_target(object.clone())
            .map_err(|e| Error::Binding(e.to_string()))?;
        let table = {
            let mut object = object.write();
            object
                .interaction
                .get_or_insert_with(|| Binding {
                    registry: self.clone(),
                    world: state.identity(),
                    target: target.internal_ref(),
                    table: options::Table::new(Some(target.node)),
                })
                .table
                .clone()
        };
        if table.read().node() != Some(target.node) {
            return Err(Error::Binding("scenery target identity changed".into()));
        }
        options::set_default(
            &mut Context {
                registry: self,
                definitions,
                object,
                node: target.node,
                lowercase: &mut lowercase,
            },
            &table,
        )
    }
    pub fn capture(&self, state: &mut State, object: &Scenery) -> Result<Captured> {
        let binding = object.read().interaction.clone().ok_or(Error::Null)?;
        self.validate(state, &binding)?;
        let target = state
            .scenery_target(object.clone())
            .map_err(|e| Error::Binding(e.to_string()))?;
        if target != binding.target {
            return Err(Error::Binding("scenery target identity changed".into()));
        }
        let owner = super::scenery_node::retained_view(object)
            .map_err(|e| Error::Binding(format!("{e:?}")))?;
        Ok(Captured {
            target,
            table: binding.table.keeping_from(&owner),
            registry: self.clone(),
        })
    }
}
#[derive(Clone)]
pub struct Binding {
    registry: Registry,
    world: Identity,
    target: NodeHandle,
    table: options::Handle,
}
impl Binding {
    pub(crate) fn owner(&self) -> NodeHandle {
        self.target.clone()
    }
}
#[derive(Clone)]
pub struct Captured {
    pub target: NodeHandle,
    pub table: options::Handle,
    registry: Registry,
}
impl Captured {
    pub fn handler(&self, id: usize) -> Result<Handler> {
        self.registry.handler(id)
    }
}
struct Context<'a, L> {
    registry: &'a Registry,
    definitions: &'a mut defs::Registry,
    object: &'a Scenery,
    node: Node,
    lowercase: &'a mut L,
}
impl<L: FnMut(&str) -> Result<String>> options::Context for Context<'_, L> {
    type Definition = defs::Handle;
    fn kind(&self, node: Option<Node>) -> Kind {
        if node == Some(self.node) {
            Kind::Scenery
        } else {
            Kind::Other
        }
    }
    fn node_id(&mut self, _: Node) -> Result<i32> {
        Ok(self.object.read().id)
    }
    fn item_has_location(&mut self, _: Node) -> Result<bool> {
        Err(Error::Binding("scenery is not an item".into()))
    }
    fn definition(&mut self, _: Node) -> Result<Option<Self::Definition>> {
        Ok(self.object.read().definition.clone())
    }
    fn names(&mut self, definition: &Self::Definition, kind: NamesKind) -> Result<Option<Names>> {
        if kind != NamesKind::Options {
            return Err(Error::Binding("non-scenery action array".into()));
        }
        Ok(definition.read().options.clone())
    }
    fn lowercase(&mut self, text: &str) -> Result<String> {
        (self.lowercase)(text)
    }
    fn option_handler(&mut self, kind: Kind, id: i32, name: &str) -> Result<Option<usize>> {
        if kind != Kind::Scenery {
            return Err(Error::Binding("non-scenery option lookup".into()));
        }
        let definition = self.definitions.for_id(id);
        let value = definition
            .read()
            .handlers
            .read()
            .get(&format!("option:{name}"))
            .cloned();
        let handler = match value {
            Some(Property::OptionHandler(h)) => Some(h),
            None | Some(Property::Null) => self.registry.defaults().read().get(Some(name)),
            _ => return Err(Error::Binding("scenery option handler type".into())),
        };
        handler.map(|h| self.registry.bind(h)).transpose()
    }
    fn send_option(&mut self, _: Node, _: i32, _: Option<String>) -> Result<()> {
        Err(Error::Binding("scenery table sent player options".into()))
    }
    fn node_string(&mut self, _: Option<Node>) -> Result<String> {
        Ok(format!("Scenery [{}]", self.object.read().id))
    }
    fn constants(&self) -> &Constants {
        &self.registry.0.constants
    }
    fn retain_option(&self, option: OptionHandle) -> OptionHandle {
        self.registry.0.arena.retain(option)
    }
}
