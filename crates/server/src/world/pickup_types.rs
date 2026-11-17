//! GodType.forCape/hasAny and RunePouch.forItem over their canonical mutable
//! Item values. Other god-cape and rune-pouch gameplay is separate content.
use crate::{
    definitions::item::{Registry, Shared},
    inventory::item::{Handle, Item},
};
#[derive(Clone)]
pub struct Types {
    pub capes: [Handle; 3],
    /// Ordinary and decayed items in original enum declaration order.
    pub pouches: [[Handle; 2]; 4],
}
impl Types {
    pub fn new(definitions: &Registry) -> Self {
        let item = |id| Shared::new(Item::new(id, 1, 1000, definitions));
        Self {
            capes: [2412, 2413, 2414].map(item),
            pouches: [5509, 5510, 5512, 5514].map(|id| [item(id), item(id + 1)]),
        }
    }
    pub fn cape(&self, item: &Handle) -> Option<usize> {
        self.capes
            .iter()
            .position(|cape| cape.read().id() == item.read().id())
    }
    pub fn pouch(&self, item: &Handle) -> Option<usize> {
        self.pouches.iter().enumerate().position(|(i, p)| {
            p[0].read().id() == item.read().id() || (i != 0 && p[1].read().id() == item.read().id())
        })
    }
    pub fn has_cape<E>(
        &self,
        mut has_item: impl FnMut(&Handle) -> Result<bool, E>,
    ) -> Result<bool, E> {
        // Capture enum identities, not their item IDs: a callback can mutate
        // a later canonical cape before the next query.
        for cape in &self.capes {
            if has_item(cape)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
