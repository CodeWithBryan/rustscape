//! Numeric and input-prompt action decoders from pinned Decoders530.
//! Other chat/social decoding is separate; unsupported opcodes remain raw frames.
use super::Frame;
use crate::{
    Result,
    buffer::{Reader, Writer},
};

/// One absolute destination in the original worldspace-walk format. The owning
/// server chooses the authoritative path. Coordinates use the original writer's
/// low 16 bits; callers may validate their own game-world coordinate range.
pub fn worldspace_walk(x: i32, y: i32, running: bool) -> Frame {
    let mut w = Writer::new();
    w.p1add(i32::from(running));
    w.p2(x);
    w.p2add(y);
    Frame {
        opcode: 215,
        payload: w.into_vec(),
    }
}

pub const SUPPORTED_CLIENT_OPCODES: &[u8] = &[
    156, 55, 153, 161, 135, 78, 3, 148, 30, 218, 254, 194, 84, 247, 170, 68, 71, 180, 114, 175, 66,
    33, 115, 248, 27, 134, 101, 155, 196, 124, 199, 234, 168, 166, 64, 53, 9, 10, 132, 184, 73,
    195, 233, 239, 253, 79, 231, 81, 206, 94, 92, 72, 215, 39, 77, 22, 21, 243, 245, 75, 23, 111,
    157, 137, 177, 93, 20, 110, 98, 244, 65,
];
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientAction {
    ItemAction {
        opt_index: i32,
        item_id: i32,
        slot: i32,
        iface: i32,
        child: i32,
    },
    NpcAction {
        opt_index: i32,
        npc_index: i32,
    },
    SceneryAction {
        opt_index: i32,
        id: i32,
        x: i32,
        y: i32,
    },
    PlayerAction {
        opt_index: i32,
        other_index: i32,
    },
    GroundItemAction {
        opt_index: i32,
        id: i32,
        x: i32,
        y: i32,
    },
    UseWithNpc {
        item_id: i32,
        npc_index: i32,
        iface: i32,
        slot: i32,
    },
    UseWithPlayer {
        item_id: i32,
        other_index: i32,
        iface: i32,
        slot: i32,
    },
    UseWithItem {
        used_id: i32,
        used_with_id: i32,
        used_slot: i32,
        used_with_slot: i32,
        used_iface: i32,
        used_with_iface: i32,
        used_child: i32,
        used_with_child: i32,
    },
    UseWithScenery {
        item_id: i32,
        slot: i32,
        scenery_id: i32,
        x: i32,
        y: i32,
    },
    UseWithGroundItem {
        used_id: i32,
        with_id: i32,
        iface: i32,
        child: i32,
        slot: i32,
        x: i32,
        y: i32,
    },
    IfAction {
        opcode: i32,
        opt_index: i32,
        iface: i32,
        child: i32,
        slot: i32,
        item_id: i32,
    },
    ContinueOption {
        iface: i32,
        child: i32,
        slot: i32,
        opcode: i32,
    },
    CloseIface,
    ComponentGroundItemAction {
        iface: i32,
        child: i32,
        slot: i32,
        item_id: i32,
        x: i32,
        y: i32,
    },
    ComponentPlayerAction {
        other_index: i32,
        iface: i32,
        child: i32,
    },
    ComponentSceneryAction {
        iface: i32,
        child: i32,
        scenery_id: i32,
        x: i32,
        y: i32,
    },
    ComponentNpcAction {
        iface: i32,
        child: i32,
        npc_index: i32,
    },
    ComponentItemAction {
        iface: i32,
        child: i32,
        item_id: i32,
        slot: i32,
    },
    SlotSwitchMultiComponent {
        source_iface: i32,
        source_child: i32,
        source_slot: i32,
        dest_iface: i32,
        dest_child: i32,
        dest_slot: i32,
    },
    SlotSwitchSingleComponent {
        iface: i32,
        child: i32,
        source_slot: i32,
        dest_slot: i32,
        is_insert: bool,
    },
    SceneryExamine {
        id: i32,
    },
    ItemExamine {
        id: i32,
    },
    NpcExamine {
        id: i32,
    },
    WorldspaceWalk {
        dest_x: i32,
        dest_y: i32,
        is_run: bool,
    },
    InteractWalk {
        dest_x: i32,
        dest_y: i32,
        is_run: bool,
    },
    MinimapWalk {
        dest_x: i32,
        dest_y: i32,
        clicked_x: i32,
        clicked_y: i32,
        rotation: i32,
        is_run: bool,
    },
    TrackingFocus {
        is_focused: bool,
    },
    TrackingCameraPos {
        x: i32,
        y: i32,
    },
    TrackingDisplayUpdate {
        window_mode: i32,
        screen_width: i32,
        screen_height: i32,
        display_mode: i32,
    },
    TrackingAfkTimeout,
    TrackingMouseClick {
        x: i32,
        y: i32,
        is_right_click: bool,
        delay: i32,
    },
    InputIntResponse {
        response: i32,
    },
    InputStringResponse {
        response: String,
    },
    GESetOfferItem {
        item_id: i32,
    },
    ChatSetting {
        public: i32,
        private: i32,
        trade: i32,
    },
    TrackFinished {
        track_id: i32,
    },
    PacketCountUpdate {
        count: i32,
    },
    Ping,
    NoProcess,
    PlayerPrefsUpdate {
        prefs: i32,
    },
}
// IoBuffer's transformed shorts are unsigned; its plain short is signed.
fn sa(r: &mut Reader<'_>) -> Result<i32> {
    Ok((r.g1()? << 8) | ((r.g1()? - 128) & 255))
}
fn le(r: &mut Reader<'_>) -> Result<i32> {
    Ok(r.g1()? | (r.g1()? << 8))
}
fn la(r: &mut Reader<'_>) -> Result<i32> {
    Ok(((r.g1()? - 128) & 255) | (r.g1()? << 8))
}
fn ia(r: &mut Reader<'_>) -> Result<i32> {
    Ok((r.g1()? << 8) | r.g1()? | (r.g1()? << 24) | (r.g1()? << 16))
}
fn ib(r: &mut Reader<'_>) -> Result<i32> {
    Ok((r.g1()? << 16) | (r.g1()? << 24) | r.g1()? | (r.g1()? << 8))
}
fn li(r: &mut Reader<'_>) -> Result<i32> {
    Ok(r.g1()? | (r.g1()? << 8) | (r.g1()? << 16) | (r.g1()? << 24))
}
fn hash(v: i32) -> (i32, i32) {
    (v >> 16, v & 65535)
}
fn walk(r: &mut Reader<'_>, minimap: bool) -> Result<(bool, i32, i32)> {
    let running = r.g1()? - 128 == 1;
    let mut x = r.g2b()?;
    let mut y = sa(r)?;
    let steps = (r.remaining() as i32 - if minimap { 14 } else { 0 }) >> 1;
    for i in 0..steps {
        let dx = r.g1()? - 128;
        let dy = 128 - r.g1()?;
        if i == steps - 1 {
            x = x.wrapping_add(dx);
            y = y.wrapping_add(dy);
        }
    }
    Ok((running, x, y))
}
impl ClientAction {
    /// Returns None without consuming payload for actions not yet ported.
    /// Trailing bytes retain the original decoder's ignored-data behavior.
    pub fn decode(opcode: u8, r: &mut Reader<'_>) -> Result<Option<Self>> {
        use ClientAction::*;
        let action = match opcode {
            156 | 55 | 153 | 161 | 135 => {
                let (opt_index, item_id, slot, h) = match opcode {
                    156 => {
                        let s = la(r)?;
                        let id = sa(r)?;
                        (0, id, s, li(r)?)
                    }
                    55 => {
                        let id = le(r)?;
                        let s = sa(r)?;
                        (1, id, s, ia(r)?)
                    }
                    153 => {
                        let h = li(r)?;
                        let s = le(r)?;
                        (2, le(r)?, s, h)
                    }
                    161 => {
                        let h = li(r)?;
                        let id = la(r)?;
                        (3, id, la(r)?, h)
                    }
                    _ => {
                        let id = sa(r)?;
                        let s = sa(r)?;
                        (4, id, s, ib(r)?)
                    }
                };
                let (iface, child) = hash(h);
                ItemAction {
                    opt_index,
                    item_id,
                    slot,
                    iface,
                    child,
                }
            }
            78 | 3 | 148 | 30 | 218 => {
                let (opt_index, npc_index) = match opcode {
                    78 => (0, le(r)?),
                    3 => (1, la(r)?),
                    148 => (2, sa(r)?),
                    30 => (3, r.g2b()?),
                    _ => (4, le(r)?),
                };
                NpcAction {
                    opt_index,
                    npc_index,
                }
            }
            254 | 194 | 84 | 247 | 170 => {
                let (opt_index, id, x, y) = match opcode {
                    254 => {
                        let x = le(r)?;
                        let id = sa(r)?;
                        (0, id, x, r.g2b()?)
                    }
                    194 => {
                        let y = la(r)?;
                        let x = le(r)?;
                        (1, r.g2b()? & 65535, x, y)
                    }
                    84 => {
                        let id = la(r)?;
                        let y = la(r)?;
                        (2, id, le(r)?, y)
                    }
                    247 => {
                        let y = le(r)?;
                        let x = la(r)?;
                        (3, r.g2b()? & 65535, x, y)
                    }
                    _ => {
                        let id = la(r)?;
                        let x = la(r)?;
                        (4, id, x, la(r)?)
                    }
                };
                SceneryAction {
                    opt_index,
                    id,
                    x,
                    y,
                }
            }
            68 | 71 | 180 | 114 | 175 => {
                let opt_index = match opcode {
                    68 => 0,
                    71 => 2,
                    180 => 3,
                    114 => 6,
                    _ => 7,
                };
                let other_index = if opcode == 175 { sa(r)? } else { la(r)? };
                PlayerAction {
                    opt_index,
                    other_index,
                }
            }
            66 | 33 => {
                let (opt_index, id, x, y) = if opcode == 66 {
                    let x = le(r)?;
                    let id = r.g2b()?;
                    (2, id, x, la(r)?)
                } else {
                    let id = r.g2b()?;
                    let x = r.g2b()?;
                    (3, id, x, le(r)?)
                };
                GroundItemAction {
                    opt_index,
                    id,
                    x,
                    y,
                }
            }
            115 => {
                let (iface, _) = hash(ib(r)?);
                let slot = le(r)?;
                let npc_index = le(r)?;
                UseWithNpc {
                    item_id: la(r)?,
                    npc_index,
                    iface,
                    slot,
                }
            }
            248 => {
                let other_index = la(r)?;
                let item_id = r.g2b()?;
                let slot = r.g2b()?;
                let (iface, _) = hash(ib(r)?);
                UseWithPlayer {
                    item_id,
                    other_index,
                    iface,
                    slot,
                }
            }
            27 => {
                let used_slot = r.g2b()?;
                let (used_iface, used_child) = hash(li(r)?);
                let used_with_slot = le(r)?;
                let (used_with_iface, used_with_child) = hash(li(r)?);
                let used_id = la(r)?;
                let used_with_id = la(r)?;
                UseWithItem {
                    used_id,
                    used_with_id,
                    used_slot,
                    used_with_slot,
                    used_iface,
                    used_with_iface,
                    used_child,
                    used_with_child,
                }
            }
            134 => {
                let x = sa(r)?;
                let item_id = r.g2b()?;
                let y = le(r)?;
                let slot = r.g2b()?;
                le(r)?;
                r.g2b()?;
                UseWithScenery {
                    item_id,
                    slot,
                    scenery_id: sa(r)?,
                    x,
                    y,
                }
            }
            101 => {
                let x = la(r)?;
                let slot = le(r)?;
                let used_id = le(r)?;
                let with_id = le(r)?;
                let y = la(r)?;
                let (iface, child) = hash(ib(r)?);
                UseWithGroundItem {
                    used_id,
                    with_id,
                    iface,
                    child,
                    slot,
                    x,
                    y,
                }
            }
            155 | 196 | 124 | 199 | 234 | 168 | 166 | 64 | 53 | 9 | 10 => {
                let opt_index = match opcode {
                    155 => 0,
                    196 => 1,
                    124 => 2,
                    199 => 3,
                    234 => 4,
                    168 => 5,
                    166 => 6,
                    64 => 7,
                    53 => 8,
                    9 => 9,
                    _ => -1,
                };
                let (iface, child) = hash(r.g4()?);
                let slot = if opcode == 10 { -1 } else { r.g2b()? };
                IfAction {
                    opcode: i32::from(opcode),
                    opt_index,
                    iface,
                    child,
                    slot,
                    item_id: -1,
                }
            }
            132 => {
                let (iface, child) = hash(ia(r)?);
                ContinueOption {
                    iface,
                    child,
                    slot: le(r)?,
                    opcode: 132,
                }
            }
            184 => CloseIface,
            73 => {
                let (iface, child) = hash(ia(r)?);
                let y = r.g2b()?;
                let item_id = la(r)?;
                let x = la(r)?;
                ComponentGroundItemAction {
                    iface,
                    child,
                    slot: le(r)?,
                    item_id,
                    x,
                    y,
                }
            }
            195 => {
                sa(r)?;
                let child = le(r)?;
                let iface = le(r)?;
                ComponentPlayerAction {
                    other_index: la(r)?,
                    iface,
                    child,
                }
            }
            233 => {
                let y = la(r)?;
                let x = sa(r)?;
                la(r)?;
                let (iface, child) = hash(ia(r)?);
                ComponentSceneryAction {
                    iface,
                    child,
                    scenery_id: sa(r)?,
                    x,
                    y,
                }
            }
            239 => {
                let child = le(r)?;
                let iface = le(r)?;
                sa(r)?;
                ComponentNpcAction {
                    iface,
                    child,
                    npc_index: la(r)?,
                }
            }
            253 => {
                let child = le(r)?;
                let iface = le(r)?;
                let slot = la(r)?;
                li(r)?;
                let item_id = sa(r)?;
                sa(r)?;
                ComponentItemAction {
                    iface,
                    child,
                    item_id,
                    slot,
                }
            }
            79 => {
                let (source_iface, source_child) = hash(ib(r)?);
                let dest_slot = le(r)?;
                let (dest_iface, dest_child) = hash(r.g4()?);
                SlotSwitchMultiComponent {
                    source_iface,
                    source_child,
                    source_slot: le(r)?,
                    dest_iface,
                    dest_child,
                    dest_slot,
                }
            }
            231 => {
                let source_slot = r.g2b()?;
                let (iface, child) = hash(li(r)?);
                let dest_slot = sa(r)?;
                SlotSwitchSingleComponent {
                    iface,
                    child,
                    source_slot,
                    dest_slot,
                    is_insert: r.g1b()? == 1,
                }
            }
            81 => {
                let slot = sa(r)?;
                let item_id = r.g2b()?;
                let child = r.g2b()?;
                IfAction {
                    opcode: 81,
                    opt_index: 0,
                    iface: r.g2b()?,
                    child,
                    slot,
                    item_id,
                }
            }
            206 => {
                let item_id = sa(r)?;
                let slot = le(r)?;
                let (iface, child) = hash(li(r)?);
                IfAction {
                    opcode: 206,
                    opt_index: 1,
                    iface,
                    child,
                    slot,
                    item_id,
                }
            }
            94 => SceneryExamine { id: la(r)? },
            92 => ItemExamine { id: la(r)? },
            72 => NpcExamine { id: r.g2b()? },
            215 | 77 => {
                let (is_run, dest_x, dest_y) = walk(r, false)?;
                if opcode == 215 {
                    WorldspaceWalk {
                        dest_x,
                        dest_y,
                        is_run,
                    }
                } else {
                    InteractWalk {
                        dest_x,
                        dest_y,
                        is_run,
                    }
                }
            }
            39 => {
                let (is_run, dest_x, dest_y) = walk(r, true)?;
                let clicked_x = r.g1b()?;
                let clicked_y = r.g1b()?;
                let rotation = r.g2b()?;
                r.g1b()?;
                r.g1b()?;
                r.g1b()?;
                r.g1b()?;
                r.g2b()?;
                r.g2b()?;
                r.g1b()?;
                r.g1b()?;
                MinimapWalk {
                    dest_x,
                    dest_y,
                    clicked_x,
                    clicked_y,
                    rotation,
                    is_run,
                }
            }
            22 => TrackingFocus {
                is_focused: r.g1b()? == 1,
            },
            21 => {
                let x = sa(r)?;
                TrackingCameraPos { x, y: le(r)? }
            }
            243 => {
                let window_mode = r.g1b()?;
                let screen_width = r.g2b()?;
                let screen_height = r.g2b()?;
                TrackingDisplayUpdate {
                    window_mode,
                    screen_width,
                    screen_height,
                    display_mode: r.g1b()?,
                }
            }
            245 => TrackingAfkTimeout,
            75 => {
                let data = la(r)?;
                let pos = ib(r)?;
                TrackingMouseClick {
                    x: pos >> 16,
                    y: pos & 65535,
                    is_right_click: ((data >> 15) & 1) == 1,
                    delay: data & 2047,
                }
            }
            23 => InputIntResponse { response: r.g4()? },
            244 => InputStringResponse {
                response: crate::login::decode_name(r.g8()?),
            },
            65 => {
                // ByteBufferUtils.getString consumes each byte, including on
                // an unterminated-string failure. Reader.gjstr is atomic.
                let mut bytes = Vec::new();
                loop {
                    let byte = r.g1()? as u8;
                    if byte == 0 {
                        break;
                    }
                    bytes.push(byte);
                }
                InputStringResponse {
                    response: crate::login::server_string(&bytes),
                }
            }
            111 => GESetOfferItem { item_id: r.g2b()? },
            157 => ChatSetting {
                public: r.g1b()?,
                private: r.g1b()?,
                trade: r.g1b()?,
            },
            137 => TrackFinished { track_id: la(r)? },
            177 => PacketCountUpdate { count: r.g2b()? },
            93 => Ping,
            20 | 110 => NoProcess,
            98 => PlayerPrefsUpdate { prefs: r.g4()? },
            _ => return Ok(None),
        };
        Ok(Some(action))
    }
}

/// Original MiniMenu byte order, with opcode ISAAC disabled by the pinned profile.
/// Shared transport framing/ISAAC is applied by protocol::game when connected.
pub fn location_packet(action: i32, id: i32, tile: [i32; 2]) -> Option<Vec<u8>> {
    let action = if action >= 2000 {
        action - 2000
    } else {
        action
    };
    let [x, y] = tile;
    let mut w = Writer::new();
    match action {
        42 => {
            w.p1(254);
            w.ip2(x);
            w.p2add(id);
            w.p2(y);
        }
        50 => {
            w.p1(194);
            w.ip2add(y);
            w.ip2(x);
            w.p2(id);
        }
        49 => {
            w.p1(84);
            w.ip2add(id);
            w.ip2add(y);
            w.ip2(x);
        }
        46 => {
            w.p1(247);
            w.ip2(y);
            w.ip2add(x);
            w.p2(id);
        }
        1001 => {
            w.p1(170);
            w.ip2add(id);
            w.ip2add(x);
            w.ip2add(y);
        }
        1004 => {
            w.p1(94);
            w.ip2add(id);
        }
        _ => return None,
    }
    Some(w.into_vec())
}
