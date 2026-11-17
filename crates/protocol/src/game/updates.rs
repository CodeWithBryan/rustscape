//! Wire state updates shared by the visual and headless clients.
//! Interface counters, collision and game rules remain authoritative server state.
use super::{Direction, Frame, Size};
use crate::{
    Error, Result,
    buffer::{Reader, Writer},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireItem {
    pub id: i32,
    pub amount: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotUpdate {
    pub slot: i32,
    pub item: WireItem,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptArg {
    Int(i32),
    Text(Vec<u8>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerUpdate {
    GameMessage(Vec<u8>),
    ChatSettings([u8; 3]),
    Skill {
        skill: i32,
        boosted: i32,
        experience: i32,
    },
    RunEnergy {
        energy: i32,
    },
    RunWeight {
        weight: i32,
    },
    Varp {
        id: i32,
        value: i32,
    },
    Varbit {
        id: i32,
        value: i32,
    },
    Varc {
        counter: i32,
        id: i32,
        value: i32,
    },
    RunClientScript {
        counter: i32,
        id: i32,
        types: Vec<u8>,
        arguments: Vec<ScriptArg>,
    },
    InterfaceEvents {
        counter: i32,
        pointer: i32,
        start: i32,
        end: i32,
        mask: i32,
    },
    ZoneOrigin {
        full: bool,
        x: i32,
        y: i32,
    },
    ZoneEnclosed {
        x: i32,
        y: i32,
        frames: Vec<Frame>,
    },
    LocAdd {
        x: i32,
        y: i32,
        shape: i32,
        rotation: i32,
        id: i32,
    },
    LocDel {
        x: i32,
        y: i32,
        shape: i32,
        rotation: i32,
    },
    /// Raw SYNTH_SOUND fields; preferences, synthesis and playback are client policy.
    SynthSound {
        id: i32,
        loops: i32,
        delay: i32,
    },
    LocAnim {
        x: i32,
        y: i32,
        shape: i32,
        rotation: i32,
        sequence: i32,
    },
    GroundAdd {
        x: i32,
        y: i32,
        id: i32,
        amount: i32,
        /// OBJ_ADD135 excludes this network player slot; OBJ_REVEAL33 has none.
        exclude_player: Option<i32>,
    },
    GroundRemove {
        x: i32,
        y: i32,
        id: i32,
    },
    GroundAmount {
        x: i32,
        y: i32,
        id: i32,
        old_amount: i32,
        amount: i32,
    },
    OpenInterface {
        kind: i32,
        pointer: i32,
        counter: i32,
        interface: i32,
    },
    WindowsPane {
        interface: i32,
        reset: i32,
        counter: i32,
    },
    CloseInterface {
        counter: i32,
        pointer: i32,
    },
    InterfaceHidden {
        hidden: i32,
        counter: i32,
        pointer: i32,
    },
    InterfaceText {
        pointer: i32,
        text: Vec<u8>,
        counter: i32,
    },
    InterfaceAnimation {
        pointer: i32,
        sequence: i32,
        counter: i32,
    },
    InterfaceModel {
        pointer: i32,
        model: i32,
        counter: i32,
    },
    InterfaceNpcHead {
        pointer: i32,
        npc: i32,
        counter: i32,
    },
    InterfacePlayerHead {
        pointer: i32,
        counter: i32,
    },
    InterfaceAngle {
        pointer: i32,
        pitch: i32,
        yaw: i32,
        zoom: i32,
        counter: i32,
    },
    InterfaceObject {
        pointer: i32,
        item: i32,
        value: i32,
        counter: i32,
    },
    InterfacePosition {
        pointer: i32,
        x: i32,
        y: i32,
        counter: i32,
    },
    InventoryClear {
        pointer: i32,
    },
    InventoryFull {
        pointer: i32,
        container: i32,
        items: Vec<WireItem>,
    },
    InventoryPartial {
        pointer: i32,
        container: i32,
        slots: Vec<SlotUpdate>,
    },
}
fn frame(opcode: u8, w: Writer) -> Frame {
    Frame {
        opcode,
        payload: w.into_vec(),
    }
}
fn packed(x: i32, y: i32) -> i32 {
    x.wrapping_shl(4) | (y & 7)
}
fn shape(s: i32, r: i32) -> i32 {
    s.wrapping_shl(2) | (r & 3)
}
/// The caller resolves the original prayer/lifepoint/current-level selection.
/// Java's double-to-int XP truncation/saturation precedes byte serialization.
pub fn skill_level(skill: i32, boosted: i32, experience: f64) -> Frame {
    let mut w = Writer::new();
    w.p1add(boosted);
    w.imp4(experience as i32);
    w.p1(skill);
    frame(38, w)
}
/// Java converts the double to int and then truncates to a byte.
pub fn run_energy(energy: f64) -> Frame {
    let mut w = Writer::new();
    w.p1(energy as i32);
    frame(234, w)
}

/// Original AudioPacket.write without an area location (SYNTH_SOUND).
pub fn synth_sound(id: i32, loops: i32, delay: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(id);
    w.p1(loops);
    w.p2(delay);
    Frame {
        opcode: 172,
        payload: w.into_vec(),
    }
}

/// Original AudioPacket.write with an area location; its zone origin is sent
/// separately by the owning world, as in UpdateAreaPosition.
pub fn area_sound(x: i32, y: i32, id: i32, radius: i32, loops: i32, delay: i32) -> Frame {
    let mut w = Writer::new();
    w.p1(((x & 7) << 4) | (y & 7));
    w.p2(id);
    w.p1(radius.wrapping_shl(4) | (loops & 7));
    w.p1(delay);
    Frame {
        opcode: 97,
        payload: w.into_vec(),
    }
}
/// The original writer uses its small form for every value <=255, including
/// negative values. RT4 receives that form as an unsigned transformed byte.
pub fn varc(id: i32, value: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    if value <= 255 {
        w.ip2(counter);
        w.p1neg(value);
        w.ip2add(id);
        frame(65, w)
    } else {
        w.ip2add(counter);
        w.p4(value);
        w.p2add(id);
        frame(69, w)
    }
}
/// Original Config writer; artificial-player suppression belongs to its caller.
pub fn varp(id: i32, value: i32) -> Frame {
    let mut w = Writer::new();
    if !(-128..=127).contains(&value) {
        w.p4(value);
        w.p2add(id);
        frame(226, w)
    } else {
        w.p2add(id);
        w.p1neg(value);
        frame(60, w)
    }
}
/// Preserves Varbit.java's lossy value and large-packet ID transformations.
/// Decoding a frame from this writer need not reproduce the requested ID/value.
pub fn varbit(id: i32, value: i32) -> Frame {
    let mut w = Writer::new();
    if value > 255 {
        w.ip4((128 | value) & 255);
    } else {
        w.p1(-128 | value);
    }
    w.ip2(id);
    frame(if value > 255 { 84 } else { 37 }, w)
}
/// `objects` uses original RunScriptContext order: first object corresponds to
/// the last signature character. Extra objects are ignored, as in the source.
pub fn run_client_script(
    id: i32,
    types: &[u8],
    objects: &[ScriptArg],
    counter: i32,
) -> Result<Frame> {
    if !types.is_ascii() {
        return Err(Error::new(0, "non-ASCII client-script signature"));
    }
    let mut w = Writer::new();
    w.p2(counter);
    w.pjstr(types)?;
    for (index, &kind) in types.iter().rev().enumerate() {
        let argument = objects
            .get(index)
            .ok_or(Error::new(0, "missing client-script argument"))?;
        match (kind, argument) {
            (b's', ScriptArg::Text(text)) => w.pjstr(text)?,
            (b's', _) | (_, ScriptArg::Text(_)) => {
                return Err(Error::new(0, "client-script argument type mismatch"));
            }
            (_, ScriptArg::Int(value)) => w.p4(*value),
        }
    }
    w.p4(id);
    let f = frame(115, w);
    if f.payload.len() > 65535 {
        return Err(Error::new(0, "client-script payload exceeds header"));
    }
    Ok(f)
}
/// Inclusive child-slot range; RT4 maps 65535 endpoints to -1.
pub fn interface_events(pointer: i32, start: i32, end: i32, mask: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2(counter);
    w.ip2(end);
    w.p4(pointer);
    w.p2add(start);
    w.imp4(mask);
    frame(165, w)
}

pub fn zone_origin(x: i32, y: i32, full: bool) -> Frame {
    let mut w = Writer::new();
    if full {
        w.p1(x);
        w.p1neg(y);
    } else {
        w.p1neg(x);
        w.p1(y);
    }
    frame(if full { 112 } else { 26 }, w)
}

/// MAP_PROJANIM_2, relative to UPDATE_ZONE_PARTIAL_FOLLOWS or its envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub offset: [i32; 2],
    pub delta: [i32; 2],
    pub target: i32,
    pub spot: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start: i32,
    pub end: i32,
    pub angle: i32,
    pub distance: i32,
}

impl Projectile {
    pub fn frame(self) -> Frame {
        let mut w = Writer::new();
        w.p1((self.offset[0] & 7) << 4 | self.offset[1] & 7);
        w.p1(self.delta[0]);
        w.p1(self.delta[1]);
        w.p2(self.target);
        w.p2(self.spot);
        w.p1(self.start_height);
        w.p1(self.end_height);
        w.p2(self.start);
        w.p2(self.end);
        w.p1(self.angle);
        w.p1(self.distance);
        Frame {
            opcode: 16,
            payload: w.into_vec(),
        }
    }

    pub fn decode(frame: &Frame) -> Result<Self> {
        if frame.opcode != 16 || frame.payload.len() != 15 {
            return Err(Error::new(0, "invalid projectile frame"));
        }
        let mut r = Reader::new(&frame.payload);
        let offset = r.g1()?;
        let delta = [r.g1b()?, r.g1b()?];
        let target = r.g2b()?;
        let spot = r.g2()?;
        let start_height = r.g1()?;
        let end_height = r.g1()?;
        let start = r.g2()?;
        let end = r.g2()?;
        let angle = r.g1()?;
        Ok(Self {
            offset: [(offset >> 4) & 7, offset & 7],
            delta,
            target,
            spot,
            start_height,
            end_height,
            start,
            end,
            angle: if angle == 255 { -1 } else { angle },
            distance: r.g1()?,
        })
    }
}
/// Coordinates are offsets within the current chunk, not absolute world tiles.
pub fn loc_add(x: i32, y: i32, s: i32, r: i32, id: i32) -> Frame {
    let mut w = Writer::new();
    w.p1add(shape(s, r));
    w.p1(packed(x, y));
    w.p2add(id);
    frame(179, w)
}
pub fn loc_del(x: i32, y: i32, s: i32, r: i32) -> Frame {
    let mut w = Writer::new();
    w.p1neg(shape(s, r));
    w.p1(packed(x, y));
    frame(195, w)
}
pub fn loc_anim(x: i32, y: i32, s: i32, r: i32, sequence: i32) -> Frame {
    let mut w = Writer::new();
    w.p1sub(packed(x, y));
    w.p1sub(shape(s, r));
    w.ip2(sequence);
    frame(20, w)
}
pub fn ground_add(x: i32, y: i32, id: i32, amount: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2(id);
    w.p1(packed(x, y));
    w.p2add(amount);
    frame(33, w)
}
pub fn ground_add_excluding(x: i32, y: i32, id: i32, amount: i32, player: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2add(player);
    w.p1neg(packed(x, y));
    w.ip2(amount);
    w.ip2(id);
    frame(135, w)
}
pub fn ground_remove(x: i32, y: i32, id: i32) -> Frame {
    let mut w = Writer::new();
    w.p1sub(packed(x, y));
    w.p2(id);
    frame(240, w)
}
pub fn ground_amount(x: i32, y: i32, id: i32, old_amount: i32, amount: i32) -> Frame {
    let mut w = Writer::new();
    w.p1(packed(x, y));
    w.p2(id);
    w.p2(old_amount);
    w.p2(amount);
    frame(14, w)
}
pub fn zone_enclosed(x: i32, y: i32, frames: &[Frame]) -> Result<Frame> {
    let mut w = Writer::new();
    w.p1add(y);
    w.p1sub(x);
    let mut payload = w.into_vec();
    for f in frames {
        if !is_zone_opcode(f.opcode) {
            return Err(Error::new(0, "not a zone update opcode"));
        }
        if payload
            .len()
            .checked_add(f.payload.len())
            .and_then(|n| n.checked_add(1))
            .is_none_or(|n| n > 65535)
        {
            return Err(Error::new(0, "enclosed zone exceeds header"));
        }
        f.write(Direction::ServerToClient, None, &mut payload)?;
    }
    let f = Frame {
        opcode: 230,
        payload,
    };
    if f.payload.len() > 65535 {
        return Err(Error::new(0, "enclosed zone exceeds header"));
    }
    Ok(f)
}
fn is_zone_opcode(op: u8) -> bool {
    matches!(
        op,
        195 | 33 | 121 | 17 | 179 | 20 | 202 | 14 | 135 | 16 | 104 | 97 | 240
    )
}
pub fn open_interface(kind: i32, pointer: i32, counter: i32, interface: i32) -> Frame {
    let mut w = Writer::new();
    w.p1(kind);
    w.mp4(pointer);
    w.p2add(counter);
    w.p2(interface);
    frame(155, w)
}
/// Original WindowsPane writer uses p1sub, while RT4 reads g1add. Preserve both.
pub fn windows_pane(interface: i32, kind: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2add(interface);
    w.p1sub(kind);
    w.ip2add(counter);
    frame(145, w)
}
pub fn close_interface(counter: i32, pointer: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(counter);
    w.p4(pointer);
    frame(149, w)
}
pub fn interface_hidden(hidden: bool, counter: i32, pointer: i32) -> Frame {
    let mut w = Writer::new();
    w.p1neg(i32::from(hidden));
    w.p2(counter);
    w.ip4(pointer);
    frame(21, w)
}
/// Text is already encoded for the original server's string writer.
pub fn interface_text(pointer: i32, text: &[u8], counter: i32) -> Result<Frame> {
    let mut w = Writer::new();
    w.mp4(pointer);
    w.pjstr(text)?;
    w.p2add(counter);
    let f = frame(171, w);
    if f.payload.len() > 65535 {
        return Err(Error::new(0, "interface text exceeds header"));
    }
    Ok(f)
}
pub fn inventory_clear(pointer: i32) -> Frame {
    let mut w = Writer::new();
    w.mp4(pointer);
    frame(144, w)
}
/// Original AnimateInterface writer. Sequence is read as a signed little-endian short.
pub fn interface_animation(pointer: i32, sequence: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.mp4(pointer);
    w.ip2(sequence);
    w.p2add(counter);
    frame(36, w)
}
pub fn interface_model(pointer: i32, model: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.ip4(pointer);
    w.ip2add(counter);
    w.p2add(model);
    frame(130, w)
}
pub fn interface_npc_head(pointer: i32, npc: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.p2add(npc);
    w.ip4(pointer);
    w.ip2(counter);
    frame(73, w)
}
pub fn interface_player_head(pointer: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2add(counter);
    w.imp4(pointer);
    frame(66, w)
}
pub fn interface_angle(pointer: i32, pitch: i32, yaw: i32, zoom: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(pitch);
    w.p2add(counter);
    w.ip2add(zoom);
    w.ip2add(yaw);
    w.p4(pointer);
    frame(132, w)
}
/// DisplayModel ITEM's value is the caller-resolved amount>0 ? amount : zoom.
pub fn interface_object(pointer: i32, item: i32, value: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.p4(value);
    w.mp4(pointer);
    w.ip2add(item);
    w.ip2(counter);
    frame(50, w)
}
/// Original RepositionChild writer; RT4 reads both positions as signed shorts.
pub fn interface_position(pointer: i32, x: i32, y: i32, counter: i32) -> Frame {
    let mut w = Writer::new();
    w.p2add(counter);
    w.ip4(pointer);
    w.p2(x);
    w.p2add(y);
    frame(119, w)
}
pub fn inventory_full(pointer: i32, container: i32, items: &[Option<WireItem>]) -> Result<Frame> {
    if items.len() > 65535 {
        return Err(Error::new(0, "inventory length exceeds u16"));
    }
    let mut w = Writer::new();
    w.p4(pointer);
    w.p2(container);
    w.p2(items.len() as i32);
    for item in items {
        let item = item.unwrap_or(WireItem { id: -1, amount: 0 });
        if !(0..=254).contains(&item.amount) {
            w.p1sub(255);
            w.p4(item.amount);
        } else {
            w.p1sub(item.amount);
        }
        w.p2(item.id.wrapping_add(1));
    }
    let f = frame(105, w);
    if f.payload.len() > 65535 {
        return Err(Error::new(0, "inventory payload exceeds header"));
    }
    Ok(f)
}
/// Caller resolves slots and NULL_ITEM equality against the authoritative container.
/// None represents either a null entry or an entry equal to that world's NULL_ITEM.
pub fn inventory_partial(
    pointer: i32,
    container: i32,
    slots: &[(i32, Option<WireItem>)],
) -> Result<Frame> {
    let mut w = Writer::new();
    w.p4(pointer);
    w.p2(container);
    for &(slot, item) in slots {
        w.psmarts(slot)?;
        if let Some(item) = item {
            w.p2(item.id.wrapping_add(1));
            if !(0..=254).contains(&item.amount) {
                w.p1(255);
                w.p4(item.amount);
            } else {
                w.p1(item.amount);
            }
        } else {
            w.p2(0);
        }
    }
    let f = frame(22, w);
    if f.payload.len() > 65535 {
        return Err(Error::new(0, "inventory payload exceeds header"));
    }
    Ok(f)
}
impl ServerUpdate {
    /// Decodes payload fields; scene bounds, layer mapping and UI effects belong
    /// to the client reducer. None preserves unported messages as raw frames.
    pub fn decode(f: &Frame) -> Result<Option<Self>> {
        use ServerUpdate::*;
        let mut r = Reader::new(&f.payload);
        let value = match f.opcode {
            70 => GameMessage(r.gjstr()?.to_vec()),
            232 => ChatSettings([r.g1()? as u8, r.g1()? as u8, r.g1()? as u8]),
            36 => {
                let pointer = r.mg4()?;
                let sequence = r.ig2b()?;
                let counter = r.g2add()?;
                InterfaceAnimation {
                    pointer,
                    sequence,
                    counter,
                }
            }
            130 => {
                let pointer = r.ig4()?;
                let counter = r.ig2add()?;
                let model = r.g2add()?;
                InterfaceModel {
                    pointer,
                    model: if model == 65535 { -1 } else { model },
                    counter,
                }
            }
            73 => {
                let npc = r.g2add()?;
                let pointer = r.ig4()?;
                let counter = r.ig2()?;
                InterfaceNpcHead {
                    pointer,
                    npc: if npc == 65535 { -1 } else { npc },
                    counter,
                }
            }
            66 => {
                let counter = r.ig2add()?;
                let pointer = r.img4()?;
                InterfacePlayerHead { pointer, counter }
            }
            132 => {
                let pitch = r.g2()?;
                let counter = r.g2add()?;
                let zoom = r.ig2add()?;
                let yaw = r.ig2add()?;
                let pointer = r.g4()?;
                InterfaceAngle {
                    pointer,
                    pitch,
                    yaw,
                    zoom,
                    counter,
                }
            }
            50 => {
                let value = r.g4()?;
                let pointer = r.mg4()?;
                let item = r.ig2add()?;
                let counter = r.ig2()?;
                InterfaceObject {
                    pointer,
                    item: if item == 65535 { -1 } else { item },
                    value,
                    counter,
                }
            }
            119 => {
                let counter = r.g2add()?;
                let pointer = r.ig4()?;
                let x = r.g2b()?;
                let y = r.g2badd()?;
                InterfacePosition {
                    pointer,
                    x,
                    y,
                    counter,
                }
            }
            38 => {
                let boosted = r.g1add()?;
                let experience = r.img4()?;
                Skill {
                    skill: r.g1()?,
                    boosted,
                    experience,
                }
            }
            234 => RunEnergy { energy: r.g1()? },
            159 => RunWeight { weight: r.g2b()? },
            60 => Varp {
                id: r.g2add()?,
                value: r.g1bneg()?,
            },
            226 => {
                let value = r.g4()?;
                Varp {
                    id: r.g2add()?,
                    value,
                }
            }
            37 => {
                let value = r.g1add()?;
                Varbit {
                    id: r.ig2()?,
                    value,
                }
            }
            84 => {
                let value = r.ig4()?;
                Varbit {
                    id: r.ig2add()?,
                    value,
                }
            }
            65 => {
                let counter = r.ig2()?;
                let value = r.g1neg()?;
                Varc {
                    counter,
                    id: r.ig2add()?,
                    value,
                }
            }
            69 => {
                let counter = r.ig2add()?;
                let value = r.g4()?;
                Varc {
                    counter,
                    id: r.g2add()?,
                    value,
                }
            }
            115 => {
                let counter = r.g2()?;
                let types = r.gjstr()?.to_vec();
                let mut arguments = vec![ScriptArg::Int(0); types.len()];
                for i in (0..types.len()).rev() {
                    arguments[i] = if types[i] == b's' {
                        ScriptArg::Text(r.gjstr()?.to_vec())
                    } else {
                        ScriptArg::Int(r.g4()?)
                    };
                }
                RunClientScript {
                    counter,
                    id: r.g4()?,
                    types,
                    arguments,
                }
            }
            165 => {
                let counter = r.ig2()?;
                let end = r.ig2()?;
                let pointer = r.g4()?;
                let start = r.g2add()?;
                InterfaceEvents {
                    counter,
                    pointer,
                    start: if start == 65535 { -1 } else { start },
                    end: if end == 65535 { -1 } else { end },
                    mask: r.img4()?,
                }
            }
            112 => {
                let x = r.g1()?;
                ZoneOrigin {
                    full: true,
                    x,
                    y: r.g1neg()?,
                }
            }
            26 => {
                let x = r.g1neg()?;
                ZoneOrigin {
                    full: false,
                    x,
                    y: r.g1()?,
                }
            }
            230 => {
                let y = r.g1add()?;
                let x = r.g1sub()?;
                let mut frames = Vec::new();
                while r.remaining() > 0 {
                    let opcode = r.g1()? as u8;
                    if !is_zone_opcode(opcode) {
                        return Err(Error::new(r.position() - 1, "not a zone update opcode"));
                    }
                    let Some(Size::Fixed(mut n)) = Direction::ServerToClient.size(opcode) else {
                        return Err(Error::new(r.position() - 1, "invalid zone update size"));
                    };
                    if opcode == 104 {
                        // readZonePacket reads MAP_PROJANIM's fields directly; its
                        // PACKET_LENGTHS entry is zero and never applies here.
                        n = super::events::PROJECTILE_104_LENGTH;
                    }
                    frames.push(Frame {
                        opcode,
                        payload: r.take(n)?.to_vec(),
                    });
                }
                ZoneEnclosed { x, y, frames }
            }
            172 => SynthSound {
                id: r.g2()?,
                loops: r.g1()?,
                delay: r.g2()?,
            },
            33 => {
                let id = r.ig2()?;
                let c = r.g1()?;
                let amount = r.g2add()?;
                GroundAdd {
                    x: (c >> 4) & 7,
                    y: c & 7,
                    id,
                    amount,
                    exclude_player: None,
                }
            }
            135 => {
                let player = r.ig2add()?;
                let c = r.g1neg()?;
                let amount = r.ig2()?;
                let id = r.ig2()?;
                GroundAdd {
                    x: (c >> 4) & 7,
                    y: c & 7,
                    id,
                    amount,
                    exclude_player: Some(player),
                }
            }
            240 => {
                let c = r.g1sub()?;
                GroundRemove {
                    x: (c >> 4) & 7,
                    y: c & 7,
                    id: r.g2()?,
                }
            }
            14 => {
                let c = r.g1()?;
                GroundAmount {
                    x: (c >> 4) & 7,
                    y: c & 7,
                    id: r.g2()?,
                    old_amount: r.g2()?,
                    amount: r.g2()?,
                }
            }
            179 | 195 | 20 => {
                let (p, c, id) = match f.opcode {
                    179 => {
                        let p = r.g1add()?;
                        let c = r.g1()?;
                        (p, c, r.g2add()?)
                    }
                    195 => {
                        let p = r.g1neg()?;
                        (p, r.g1()?, -1)
                    }
                    _ => {
                        let c = r.g1sub()?;
                        let p = r.g1sub()?;
                        let id = r.ig2()?;
                        (p, c, if id == 65535 { -1 } else { id })
                    }
                };
                let (x, y, shape, rotation) = ((c >> 4) & 7, c & 7, p >> 2, p & 3);
                match f.opcode {
                    179 => LocAdd {
                        x,
                        y,
                        shape,
                        rotation,
                        id,
                    },
                    195 => LocDel {
                        x,
                        y,
                        shape,
                        rotation,
                    },
                    _ => LocAnim {
                        x,
                        y,
                        shape,
                        rotation,
                        sequence: id,
                    },
                }
            }
            155 => {
                let kind = r.g1()?;
                let pointer = r.mg4()?;
                let counter = r.g2add()?;
                OpenInterface {
                    kind,
                    pointer,
                    counter,
                    interface: r.g2()?,
                }
            }
            145 => {
                let interface = r.ig2add()?;
                let reset = r.g1add()?;
                WindowsPane {
                    interface,
                    reset,
                    counter: r.ig2add()?,
                }
            }
            149 => {
                let counter = r.g2()?;
                CloseInterface {
                    counter,
                    pointer: r.g4()?,
                }
            }
            21 => {
                let hidden = r.g1neg()?;
                let counter = r.g2()?;
                InterfaceHidden {
                    hidden,
                    counter,
                    pointer: r.ig4()?,
                }
            }
            171 => {
                let pointer = r.mg4()?;
                let text = r.gjstr()?.to_vec();
                InterfaceText {
                    pointer,
                    text,
                    counter: r.g2add()?,
                }
            }
            144 => InventoryClear { pointer: r.mg4()? },
            105 | 22 => {
                let pointer = r.g4()?;
                let mut container = r.g2()?;
                if pointer < -70000 {
                    container += 32768;
                }
                if f.opcode == 105 {
                    let total = r.g2()?;
                    let mut items = Vec::with_capacity(total as usize);
                    for _ in 0..total {
                        let n = r.g1sub()?;
                        let amount = if n == 255 { r.g4()? } else { n };
                        items.push(WireItem {
                            id: r.g2()? - 1,
                            amount,
                        });
                    }
                    InventoryFull {
                        pointer,
                        container,
                        items,
                    }
                } else {
                    let mut slots = Vec::new();
                    while r.remaining() > 0 {
                        let slot = r.gsmarts()?;
                        let id = r.g2()? - 1;
                        let amount = if id != -1 {
                            let n = r.g1()?;
                            if n == 255 { r.g4()? } else { n }
                        } else {
                            0
                        };
                        slots.push(SlotUpdate {
                            slot,
                            item: WireItem { id, amount },
                        });
                    }
                    InventoryPartial {
                        pointer,
                        container,
                        slots,
                    }
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(value))
    }
}
