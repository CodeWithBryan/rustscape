//! Actual native plugin payloads installed by content registration.
//! Serialization observes a label only; it cannot reconstruct runtime objects.
use std::{any::Any, sync::Arc};

pub trait NativePlugin: Any + Send + Sync {
    fn label(&self) -> &str;
}
#[derive(Clone)]
pub struct Handle(Arc<dyn Any + Send + Sync>, Arc<str>);
impl Handle {
    pub fn new<T: NativePlugin>(value: T) -> Self {
        let label = Arc::from(value.label());
        Self(Arc::new(value), label)
    }
    pub fn value<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0.clone().downcast().ok()
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn label(&self) -> &str {
        &self.1
    }
}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Plugin").field(&self.label()).finish()
    }
}
