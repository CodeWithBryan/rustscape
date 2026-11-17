//! Outgoing packet writers the command sets and the system manager need
//! that had no native writer: `SystemUpdatePacket`, `CameraViewPacket`
//! (with `CameraContext.CameraType`) and `CSConfigPacket`, plus
//! `UpdateAreaPosition` and `AnimateObjectPacket` composed from the
//! existing native frames (`rs09_protocol::game::updates::zone_origin`
//! and the object animation fields of `loc_anim`).
//!
//! Each function takes what the original reads from its context and
//! player: the interface manager's packet counter (`getPacketCount(1)`,
//! read by the caller, which advances it) and the player's last scene
//! graph base. The frames carry the plain opcode; the session's ISAAC
//! stream is applied by the transport as for every other frame. The
//! command-sets oracle compiles the five writers and their contexts
//! unchanged and compares every byte (docs/command-sets.md).
use crate::world::commands::{Error, Loc, Result};
use rs09_protocol::buffer::Writer;
use rs09_protocol::game::{Frame, updates};

/// `CameraContext.CameraType` with its opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraType {
    Position,
    Rotation,
    Set,
    Shake,
    Reset,
}
impl CameraType {
    pub fn opcode(self) -> u8 {
        match self {
            Self::Position => 154,
            Self::Rotation => 125,
            Self::Set => 187,
            Self::Shake => 27,
            Self::Reset => 24,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Position => "POSITION",
            Self::Rotation => "ROTATION",
            Self::Set => "SET",
            Self::Shake => "SHAKE",
            Self::Reset => "RESET",
        }
    }
}

/// A camera packet's context fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    pub kind: CameraType,
    pub x: i32,
    pub y: i32,
    pub height: i32,
    pub speed: i32,
    pub zoom_speed: i32,
}

fn frame(opcode: u8, w: Writer) -> Frame {
    Frame {
        opcode,
        payload: w.into_vec(),
    }
}

/// `Location.getSceneX/Y(base)`: `x - ((base.x >> 3) - 6 << 3)`.
pub fn scene(location: [i32; 2], base: Option<Loc>) -> Result<[i32; 2]> {
    let [bx, by, _] = base.ok_or(Error::Null)?;
    Ok([
        location[0].wrapping_sub(((bx >> 3) - 6) << 3),
        location[1].wrapping_sub(((by >> 3) - 6) << 3),
    ])
}

/// `SystemUpdatePacket`: opcode 85, the time as a short.
pub fn system_update(time: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(time);
    frame(85, w)
}

/// `CameraViewPacket.send`: `counter` is `getPacketCount(1)`, read for
/// every type; `last_scene` only for POSITION and ROTATION.
pub fn camera(camera: Camera, counter: i32, last_scene: Option<Loc>) -> Result<Frame> {
    let mut w = Writer::new();
    match camera.kind {
        CameraType::Rotation | CameraType::Position => {
            w.p2(counter);
            let [x, y] = scene([camera.x, camera.y], last_scene)?;
            w.p1(x);
            w.p1(y);
            w.p2(camera.height);
            w.p1(camera.speed);
            w.p1(camera.zoom_speed);
        }
        CameraType::Set => {
            w.ip2(camera.x);
            w.p2(counter);
            w.p2(camera.y);
        }
        CameraType::Shake => {
            w.p2(counter);
            w.p1(camera.x);
            w.p1(camera.y);
            w.p1(camera.speed);
            w.p1(camera.zoom_speed);
            w.p2(camera.height);
        }
        CameraType::Reset => w.p2(counter),
    }
    Ok(frame(camera.kind.opcode(), w))
}

/// `CSConfigPacket.send`: opcode 65, the counter little-endian, the value
/// as a negated byte, the id little-endian with the add transform.
pub fn cs_config(counter: i32, id: i32, value: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2(counter);
    w.p1neg(i32::from(value as i8));
    w.ip2add(id);
    frame(65, w)
}

/// `UpdateAreaPosition.getBuffer(player, base)`: opcode 26 with the base's
/// scene coordinates.
pub fn update_area_position(base: Loc, last_scene: Option<Loc>) -> Result<Frame> {
    let [x, y] = scene([base[0], base[1]], last_scene)?;
    Ok(updates::zone_origin(x, y, false))
}

/// `UpdateAreaPosition.getChunkUpdateBuffer(player, base)`: opcode 230
/// (short-sized), the scene y then x; the caller appends the updates.
pub fn chunk_update_header(base: Loc, last_scene: Option<Loc>) -> Result<Frame> {
    let [x, y] = scene([base[0], base[1]], last_scene)?;
    let mut w = Writer::new();
    w.p1add(y);
    w.p1sub(x);
    Ok(frame(230, w))
}

/// An object animation's fields: the object's location, type, rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimatedObject {
    pub location: Loc,
    pub kind: i32,
    pub rotation: i32,
}

/// `AnimateObjectPacket.send`: the area position of the object's chunk
/// base, then 20, the packed chunk offset, the packed type and rotation
/// and the animation id, all in the one opcode-26 buffer.
pub fn animate_object(
    object: AnimatedObject,
    animation: i32,
    last_scene: Option<Loc>,
) -> Result<Frame> {
    let [x, y, z] = object.location;
    let base = [(x >> 3) << 3, (y >> 3) << 3, z];
    let mut out = update_area_position(base, last_scene)?;
    let mut w = Writer::new();
    w.p1(20);
    w.p1sub(((x & 63 & 7) << 4) | ((y & 63 & 7) & 0x7));
    w.p1sub((object.kind << 2).wrapping_add(object.rotation & 0x3));
    w.ip2(animation);
    out.payload.extend(w.into_vec());
    Ok(out)
}
