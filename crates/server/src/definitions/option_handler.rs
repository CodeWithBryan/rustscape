//! Native handler objects and ItemDefinition's default-option lookup.
//! Handler payloads are real native values; numeric observation IDs never
//! reconstruct executable content. The owning world binds them to dispatch.
use super::{
    item::{Handle as Definition, Shared},
    property::Property,
};
use std::{
    any::Any,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_HANDLER: AtomicUsize = AtomicUsize::new(1);
#[derive(Default)]
struct ArenaData {
    by_id: HashMap<usize, Handle>,
    by_identity: HashMap<usize, usize>,
}
/// Process-local bindings retain real native handler objects. IDs are not wire
/// values, are never reused, and cannot select a handler in another arena.
#[derive(Clone)]
pub struct Arena(Shared<ArenaData>);
impl Default for Arena {
    fn default() -> Self {
        Self(Shared::new(ArenaData::default()))
    }
}
impl Arena {
    pub fn bind(&self, handler: Handle) -> Result<usize> {
        let mut arena = self.0.write();
        if let Some(id) = arena.by_identity.get(&handler.identity()) {
            return Ok(*id);
        }
        let id = NEXT_HANDLER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Error::Binding("native handler identity exhausted".into()))?;
        arena.by_identity.insert(handler.identity(), id);
        arena.by_id.insert(id, handler);
        Ok(id)
    }
    pub fn handler(&self, id: usize) -> Result<Handle> {
        self.0
            .read()
            .by_id
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::Binding("handler belongs to another registry".into()))
    }
    pub fn retain<T: Any + Send + Sync>(&self, view: Shared<T>) -> Shared<T> {
        view.keeping_from(&self.0)
    }
}

pub trait NativeOptionHandler: Any + Send + Sync {}
pub struct Handler {
    label: String,
    value: Box<dyn Any + Send + Sync>,
}
pub type Handle = Shared<Handler>;
impl Handler {
    pub fn new<T: NativeOptionHandler>(label: impl Into<String>, value: T) -> Handle {
        Shared::new(Self {
            label: label.into(),
            value: Box::new(value),
        })
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    /// Clone any state/receiver needed for dispatch before releasing the lock;
    /// never invoke game callbacks while the handler read guard is held.
    pub fn value<T: Any>(&self) -> Option<&T> {
        self.value.downcast_ref()
    }
}
impl std::fmt::Debug for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handler")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// One global map per world. Exposed handles alias this same map. This API ports
/// lookup/put/remove/clear/contains; Java iterator/view behavior remains separate.
#[derive(Default)]
pub struct Defaults {
    values: HashMap<String, Option<Handle>>,
    null: Option<Option<Handle>>,
}
pub type DefaultHandle = Shared<Defaults>;
impl Defaults {
    pub fn get(&self, name: Option<&str>) -> Option<Handle> {
        match name {
            Some(name) => self.values.get(name).cloned().flatten(),
            None => self.null.clone().flatten(),
        }
    }
    pub fn put(&mut self, name: Option<String>, handler: Option<Handle>) -> Option<Handle> {
        match name {
            Some(name) => self.values.insert(name, handler).flatten(),
            None => self.null.replace(handler).flatten(),
        }
    }
    pub fn remove(&mut self, name: Option<&str>) -> Option<Handle> {
        match name {
            Some(name) => self.values.remove(name).flatten(),
            None => self.null.take().flatten(),
        }
    }
    pub fn contains_key(&self, name: Option<&str>) -> bool {
        match name {
            Some(name) => self.values.contains_key(name),
            None => self.null.is_some(),
        }
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.null = None;
    }
    pub fn len(&self) -> usize {
        self.values.len() + usize::from(self.null.is_some())
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.null.is_none()
    }
}
pub fn set_option_handler(
    defaults: &DefaultHandle,
    name: Option<String>,
    handler: Option<Handle>,
) -> bool {
    defaults.write().put(name, handler).is_some()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Type,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Context {
    /// Bind the original forId. Its normal absent-ID fallback is a fresh default
    /// definition; an explicit null return still follows the original branch.
    fn definition(&mut self, id: i32) -> Result<Option<Definition>>;
    fn configuration(&mut self, definition: &Definition, key: &str) -> Result<Option<Property>> {
        Ok(definition.read().handlers.get(key).cloned())
    }
    fn missing_definition(&mut self, id: i32) -> Result<()>;
}
pub fn item_handler(
    ctx: &mut impl Context,
    defaults: &DefaultHandle,
    id: i32,
    name: Option<&str>,
) -> Result<Option<Handle>> {
    let Some(definition) = ctx.definition(id)? else {
        if id == 22937 {
            ctx.missing_definition(id)?;
        }
        return Ok(None);
    };
    let key = format!("option:{}", name.unwrap_or("null"));
    match ctx.configuration(&definition, &key)? {
        Some(Property::OptionHandler(handler)) => return Ok(Some(handler)),
        None | Some(Property::Null) => {}
        Some(_) => return Err(Error::Type),
    }
    Ok(defaults.read().get(name))
}
