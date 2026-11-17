//! Test-only labels on real native objects. Labels never select runtime content.
use rs09_server::inventory::container_listener::{Handle, NativeListener};
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Recording(pub usize);
impl NativeListener for Recording {}
pub fn new(label: usize) -> Handle {
    Handle::new(Recording(label))
}
pub fn label(listener: &Handle) -> usize {
    listener.value::<Recording>().expect("recording listener").0
}
pub fn values(listeners: Vec<Option<Handle>>) -> Vec<Option<usize>> {
    listeners.iter().map(|v| v.as_ref().map(label)).collect()
}
#[derive(Default)]
pub struct Objects(BTreeMap<usize, Handle>);
impl Objects {
    pub fn optional(&mut self, value: &Value) -> Option<Handle> {
        value.as_u64().map(|label| {
            self.0
                .entry(label as usize)
                .or_insert_with(|| new(label as usize))
                .clone()
        })
    }
}
