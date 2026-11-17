//! Original MiniMenu item-in-component action serialization (ISAAC disabled by pinned client).
use rs09_protocol::{buffer::Writer, game::Frame};
#[derive(Clone, Debug)]
pub struct SelectedItem {
    pub id: i32,
    pub slot: i32,
    pub component: i32,
    pub text: String,
}
impl SelectedItem {
    pub fn on_player(&self, index: i32) -> Frame {
        let mut w = Writer::new();
        w.ip2add(index);
        w.p2(self.id);
        w.p2(self.slot);
        w.mp4(self.component);
        Frame {
            opcode: 248,
            payload: w.into_vec(),
        }
    }
    pub fn on_npc(&self, index: i32) -> Frame {
        let mut w = Writer::new();
        w.mp4(self.component);
        w.ip2(self.slot);
        w.ip2(index);
        w.ip2add(self.id);
        Frame {
            opcode: 115,
            payload: w.into_vec(),
        }
    }
    pub fn on_item(&self, id: i32, slot: i32, component: i32) -> Frame {
        let mut w = Writer::new();
        w.p2(self.slot);
        w.ip4(component);
        w.ip2(slot);
        w.ip4(self.component);
        w.ip2add(self.id);
        w.ip2add(id);
        Frame {
            opcode: 27,
            payload: w.into_vec(),
        }
    }
    pub fn on_location(&self, id: i32, tile: [i32; 2]) -> Frame {
        let mut w = Writer::new();
        w.p2add(tile[0]);
        w.p2(self.id);
        w.ip2(tile[1]);
        w.p2(self.slot);
        w.mp4(self.component);
        w.p2add(id);
        Frame {
            opcode: 134,
            payload: w.into_vec(),
        }
    }
}
pub fn item_packet(action: i32, id: i32, slot: i32, component: i32) -> Option<Frame> {
    let mut w = Writer::new();
    let opcode = match if action >= 2000 {
        action - 2000
    } else {
        action
    } {
        41 => {
            w.imp4(component);
            w.ip2(slot);
            132
        }
        8 | 51 | 59 => {
            w.p4(component);
            10
        }
        47 => {
            w.ip2add(slot);
            w.p2add(id);
            w.ip4(component);
            156
        }
        5 => {
            w.ip2(id);
            w.p2add(slot);
            w.imp4(component);
            55
        }
        43 => {
            w.ip4(component);
            w.ip2(slot);
            w.ip2(id);
            153
        }
        35 => {
            w.ip4(component);
            w.ip2add(id);
            w.ip2add(slot);
            161
        }
        58 => {
            w.p2add(id);
            w.p2add(slot);
            w.mp4(component);
            135
        }
        25 => {
            w.p2add(slot);
            w.p2(id);
            w.imp4(component);
            81
        }
        23 => {
            w.p2add(id);
            w.ip2(slot);
            w.ip4(component);
            206
        }
        48 => {
            w.ip2(slot);
            w.imp4(component);
            w.ip2add(id);
            154
        }
        7 => {
            w.imp4(component);
            w.p2(slot);
            w.p2add(id);
            85
        }
        13 => {
            w.p4(component);
            w.p2add(slot);
            w.ip2(id);
            6
        }
        1006 => {
            w.ip2add(id);
            92
        }
        _ => return None,
    };
    Some(Frame {
        opcode,
        payload: w.into_vec(),
    })
}
