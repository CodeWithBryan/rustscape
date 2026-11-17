//! CS2 keeps component object references after a child slot is deleted/replaced.
//! These leases retain only referenced detached components; ordinary lookup still
//! resolves the current child at the numeric interface/child address.
use super::{Component, state::Key};
use std::{
    collections::BTreeMap,
    rc::{Rc, Weak},
};

#[derive(Debug)]
struct Identity {
    key: Key,
    serial: u64,
}
#[derive(Clone, Debug)]
pub(crate) struct Handle(Rc<Identity>);
impl Handle {
    pub fn key(&self) -> Key {
        self.0.key
    }
}
#[derive(Default)]
pub(crate) struct Handles {
    next: u64,
    current: BTreeMap<Key, Weak<Identity>>,
    detached: BTreeMap<u64, (Weak<Identity>, Component)>,
}
impl Handles {
    pub fn retain(&mut self, key: Key) -> Handle {
        self.detached
            .retain(|_, (lease, _)| lease.strong_count() != 0);
        if let Some(identity) = self.current.get(&key).and_then(Weak::upgrade) {
            return Handle(identity);
        }
        self.next = self
            .next
            .checked_add(1)
            .expect("component identity exhausted");
        let identity = Rc::new(Identity {
            key,
            serial: self.next,
        });
        self.current.insert(key, Rc::downgrade(&identity));
        Handle(identity)
    }
    pub fn retire(&mut self, key: Key, component: Component) {
        self.detached
            .retain(|_, (lease, _)| lease.strong_count() != 0);
        if let Some(identity) = self.current.remove(&key).and_then(|w| w.upgrade()) {
            self.detached
                .insert(identity.serial, (Rc::downgrade(&identity), component));
        }
    }
    pub fn is_current(&self, handle: &Handle) -> bool {
        self.current
            .get(&handle.key())
            .and_then(Weak::upgrade)
            .is_some_and(|id| Rc::ptr_eq(&id, &handle.0))
    }
    pub fn get(&self, handle: &Handle) -> Option<&Component> {
        self.detached.get(&handle.0.serial).map(|(_, c)| c)
    }
    pub fn get_mut(&mut self, handle: &Handle) -> Option<&mut Component> {
        self.detached.get_mut(&handle.0.serial).map(|(_, c)| c)
    }
}
