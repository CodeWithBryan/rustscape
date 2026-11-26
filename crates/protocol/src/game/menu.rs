//! Original MiniMenu writers for NPC options, inventory item options and item
//! use-with targets, shared by the visual and headless clients.
//!
//! These are the byte sequences of the pinned RT4 client's menu actions with
//! opcode ISAAC disabled by the pinned profile; the transport adds framing.
//! The visual client re-exports them (`rs09_client::npc_interaction::packet`,
//! `rs09_client::ui::actions::item_packet`, `SelectedItem`'s targets), so both
//! clients publish identical bytes. Sending one says nothing about gameplay
//! acceptance: that is read from later server updates.
use super::Frame;
use crate::buffer::Writer;

/// An NPC menu action (`MiniMenu` codes 17, 16, 4, 19, 2 for the definition's
/// options 1..5, 1007 for Examine) on a local NPC index. Codes of 2000 and
/// above are the same actions shifted below the walk entry.
pub fn npc_packet(action: i32, index: i32, definition: i32) -> Option<Frame> {
    let mut w = Writer::new();
    let action = if action >= 2000 {
        action - 2000
    } else {
        action
    };
    let opcode = match action {
        17 => {
            w.ip2(index);
            78
        }
        16 => {
            w.ip2add(index);
            3
        }
        4 => {
            w.p2add(index);
            148
        }
        19 => {
            w.p2(index);
            30
        }
        2 => {
            w.ip2(index);
            218
        }
        1007 => {
            w.p2(definition);
            72
        }
        _ => return None,
    };
    Some(Frame {
        opcode,
        payload: w.into_vec(),
    })
}

/// The NPC definition's option at zero-based `option` 0..4 on a local NPC.
pub fn npc_option(option: u8, index: i32) -> Option<Frame> {
    npc_packet(*[17, 16, 4, 19, 2].get(usize::from(option))?, index, -1)
}

/// The item a player selected with Use (or a spell's target cursor's item),
/// with the component it sits in (`interface << 16 | child`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsedItem {
    pub id: i32,
    pub slot: i32,
    pub component: i32,
}
impl UsedItem {
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

/// RESUME_PAUSEBUTTON (menu code 41): a chatbox Continue or a dialogue option.
pub fn resume_pause_button(component: i32, slot: i32) -> Frame {
    item_packet(41, 0, slot, component).expect("original Continue writer")
}

/// CLOSE_MODAL: closes the open modal interface or dialogue.
pub fn close_modal() -> Frame {
    Frame {
        opcode: 184,
        payload: vec![],
    }
}

/// An item-in-component menu action by its MiniMenu code.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{buffer::Reader, game::ClientAction};
    fn decode(f: &Frame) -> ClientAction {
        let mut r = Reader::new(&f.payload);
        let action = ClientAction::decode(f.opcode, &mut r).unwrap().unwrap();
        assert_eq!(r.remaining(), 0, "opcode {} left bytes", f.opcode);
        action
    }
    #[test]
    fn npc_options_decode_as_the_server_reads_them() {
        for (option, opcode) in [78, 3, 148, 30, 218].into_iter().enumerate() {
            for index in [0, 1, 127, 128, 255, 256, 2047, 32767] {
                let f = npc_option(option as u8, index).unwrap();
                assert_eq!(f.opcode, opcode);
                assert_eq!(
                    decode(&f),
                    ClientAction::NpcAction {
                        opt_index: option as i32,
                        npc_index: index
                    }
                );
            }
        }
        assert!(npc_option(5, 1).is_none());
        assert_eq!(npc_packet(2017, 9, -1), npc_packet(17, 9, -1));
        assert_eq!(
            decode(&npc_packet(1007, 0, 41).unwrap()),
            ClientAction::NpcExamine { id: 41 }
        );
    }
    #[test]
    fn use_with_targets_decode_as_the_server_reads_them() {
        let inv = 149 << 16;
        for (id, slot) in [(1205, 0), (590, 27), (1511, 13), (32767, 255)] {
            let used = UsedItem {
                id,
                slot,
                component: inv,
            };
            assert_eq!(
                decode(&used.on_npc(300)),
                ClientAction::UseWithNpc {
                    item_id: id,
                    npc_index: 300,
                    iface: 149,
                    slot
                }
            );
            assert_eq!(
                decode(&used.on_player(5)),
                ClientAction::UseWithPlayer {
                    item_id: id,
                    other_index: 5,
                    iface: 149,
                    slot
                }
            );
            assert_eq!(
                decode(&used.on_item(1511, 3, inv)),
                ClientAction::UseWithItem {
                    used_id: id,
                    used_with_id: 1511,
                    used_slot: slot,
                    used_with_slot: 3,
                    used_iface: 149,
                    used_with_iface: 149,
                    used_child: 0,
                    used_with_child: 0
                }
            );
            assert_eq!(
                decode(&used.on_location(2732, [3222, 3218])),
                ClientAction::UseWithScenery {
                    item_id: id,
                    slot,
                    scenery_id: 2732,
                    x: 3222,
                    y: 3218
                }
            );
        }
    }
    #[test]
    fn continue_and_item_options_decode_as_the_server_reads_them() {
        for (iface, child) in [(241, 5), (64, 5), (228, 3), (234, 6), (210, 2)] {
            assert_eq!(
                decode(&resume_pause_button(iface << 16 | child, -1)),
                ClientAction::ContinueOption {
                    iface,
                    child,
                    slot: 65535,
                    opcode: 132
                }
            );
        }
        assert_eq!(decode(&close_modal()), ClientAction::CloseIface);
        for (action, opt_index) in [(47, 0), (5, 1), (43, 2), (35, 3), (58, 4)] {
            assert_eq!(
                decode(&item_packet(action, 1891, 7, 149 << 16).unwrap()),
                ClientAction::ItemAction {
                    opt_index,
                    item_id: 1891,
                    slot: 7,
                    iface: 149,
                    child: 0
                }
            );
        }
    }
}
