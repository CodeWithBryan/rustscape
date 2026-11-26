//! Original MiniMenu item-in-component action serialization (ISAAC disabled by pinned client).
//! The writers live in `rs09_protocol::game::menu`, shared with the headless client.
pub use rs09_protocol::game::menu::item_packet;
use rs09_protocol::game::{Frame, menu::UsedItem};
#[derive(Clone, Debug)]
pub struct SelectedItem {
    pub id: i32,
    pub slot: i32,
    pub component: i32,
    pub text: String,
}
impl SelectedItem {
    fn used(&self) -> UsedItem {
        UsedItem {
            id: self.id,
            slot: self.slot,
            component: self.component,
        }
    }
    pub fn on_player(&self, index: i32) -> Frame {
        self.used().on_player(index)
    }
    pub fn on_npc(&self, index: i32) -> Frame {
        self.used().on_npc(index)
    }
    pub fn on_item(&self, id: i32, slot: i32, component: i32) -> Frame {
        self.used().on_item(id, slot, component)
    }
    pub fn on_location(&self, id: i32, tile: [i32; 2]) -> Frame {
        self.used().on_location(id, tile)
    }
}
