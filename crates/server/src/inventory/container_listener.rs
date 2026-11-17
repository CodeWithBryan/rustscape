//! Actual native ContainerListener objects. Observation labels belong to test
//! adapters; a runtime listener is never reconstructed from a numeric ID.
use crate::world::player_walk::NodeHandle;
use std::{
    any::Any,
    sync::{Arc, Weak},
};

pub trait NativeListener: Any + Send + Sync {
    /// Every strong canonical node field stored by this object, once per field.
    /// This is side-effect-free ownership metadata, never a gameplay callback.
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        Vec::new()
    }
}
impl NativeListener for super::listener::Listener {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        self.owner().into_iter().collect()
    }
}

type Value = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
pub struct Handle(Value, fn(&Value) -> Vec<NodeHandle>);
#[derive(Clone)]
pub struct WeakHandle(Weak<dyn Any + Send + Sync>, fn(&Value) -> Vec<NodeHandle>);

impl Handle {
    pub fn new<T: NativeListener>(listener: T) -> Self {
        Self(Arc::new(listener), |value| {
            value.downcast_ref::<T>().unwrap().retained_nodes()
        })
    }
    /// Clone the exact receiver before dispatch. This immutable object header
    /// needs no lock; mutable listener fields use their own native handles.
    pub fn value<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0.clone().downcast().ok()
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    pub fn downgrade(&self) -> WeakHandle {
        WeakHandle(Arc::downgrade(&self.0), self.1)
    }
    pub(crate) fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
    pub(crate) fn retained_nodes(&self) -> Vec<NodeHandle> {
        (self.1)(&self.0)
    }
}
impl WeakHandle {
    pub fn upgrade(&self) -> Option<Handle> {
        self.0.upgrade().map(|value| Handle(value, self.1))
    }
}
impl PartialEq for Handle {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}
impl Eq for Handle {}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ContainerListener")
            .field(&self.identity())
            .finish()
    }
}
