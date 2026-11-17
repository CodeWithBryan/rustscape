//! Remaining revision-530 server messages, transcribed from the pinned RT4
//! `Protocol.readPacketInternal`/`readZonePacket` read order.
//!
//! These are payload decoders only. Scene bounds, ignore lists, definition
//! lookups, camera state and every other effect belong to the client reducer.
//! `updates::ServerUpdate` keeps the messages it already owns; a frame decoded
//! there returns `None` here, so no opcode has two decoders.
use super::Frame;
use crate::{Error, Result, buffer::Reader};

/// Chat sender fields shared by private, clan and quick-chat messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sender {
    /// Base-37 name exactly as sent; quick-chat and friend flags are separate.
    pub name: i64,
    /// `(top << 32) + bot`, the original duplicate-suppression key.
    pub message_id: i64,
    pub rights: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanMember {
    pub name: i64,
    pub world: i32,
    pub rank: i32,
    pub world_name: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Offer {
    /// Low three bits are the status; bit 3 selects sell.
    pub status: i32,
    pub item: i32,
    pub price: i32,
    pub count: i32,
    pub completed_count: i32,
    pub completed_gold: i32,
}

/// MAP_PROJANIM (104) and MAP_PROJANIM_3 (121). Coordinates are half tiles
/// relative to twice the current zone origin; 16 keeps `updates::Projectile`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HalfTileProjectile {
    pub offset: [i32; 2],
    pub delta: [i32; 2],
    /// The actor the projectile tracks (positive NPC index + 1, negative player).
    pub target: i32,
    /// Only 104 names a launching actor; its BAS slot supplies the launch offset.
    pub source: Option<i32>,
    pub spot: i32,
    /// Height above the source tile in scene units: 104 sends it signed and
    /// unscaled, 121 sends an unsigned byte times four.
    pub start_offset: i32,
    /// Raw byte, multiplied by four at use like `updates::Projectile`.
    pub end_height: i32,
    pub start: i32,
    pub end: i32,
    pub angle: i32,
    pub distance: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceRebuild {
    pub local_x: i32,
    pub local_y: i32,
    pub plane: i32,
    pub zone_x: i32,
    pub zone_y: i32,
    /// `[plane][x][y]`, -1 for an absent chunk, otherwise the packed source.
    pub chunks: Vec<i32>,
    pub keys: Vec<[i32; 4]>,
}
impl InstanceRebuild {
    pub fn chunk(&self, plane: usize, x: usize, y: usize) -> i32 {
        self.chunks[(plane * 13 + x) * 13 + y]
    }
    pub fn base(&self) -> [i32; 2] {
        [(self.zone_x - 6) << 3, (self.zone_y - 6) << 3]
    }
    /// Distinct source regions in the original first-seen order.
    pub fn regions(&self) -> Vec<i32> {
        let mut out: Vec<i32> = vec![];
        for &packed in &self.chunks {
            if packed == -1 {
                continue;
            }
            let (cx, cy) = (packed >> 14 & 0x3ff, packed >> 3 & 0x7ff);
            let region = cy / 8 + ((cx / 8) << 8);
            if !out.contains(&region) {
                out.push(region);
            }
        }
        out
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerEvent {
    PrivateMessage {
        sender: Sender,
        body: Vec<u8>,
    },
    PrivateEcho {
        name: i64,
        body: Vec<u8>,
    },
    PrivateQuickChat {
        sender: Sender,
        phrase: i32,
        body: Vec<u8>,
    },
    PrivateQuickChatEcho {
        name: i64,
        phrase: i32,
        body: Vec<u8>,
    },
    ClanMessage {
        sender: Sender,
        clan: i64,
        body: Vec<u8>,
    },
    ClanQuickChat {
        sender: Sender,
        clan: i64,
        phrase: i32,
        body: Vec<u8>,
    },
    /// JOIN_CLAN_CHAT. `members` is None for the original 255 "unchanged" size.
    ClanChannel {
        owner: i64,
        name: i64,
        min_kick: i32,
        members: Option<Vec<ClanMember>>,
    },
    ClanLeft,
    ClanMemberJoined(ClanMember),
    ClanMemberLeft {
        name: i64,
        world: i32,
    },
    Friend {
        name: i64,
        world: i32,
        rank: i32,
        same_game: bool,
        world_name: Vec<u8>,
    },
    FriendListState(i32),
    IgnoreList(Vec<i64>),
    InterfaceColour {
        pointer: i32,
        colour: i32,
        counter: i32,
    },
    InterfaceScroll {
        pointer: i32,
        position: i32,
        counter: i32,
    },
    InterfaceRotation {
        pointer: i32,
        pitch_step: i32,
        yaw_step: i32,
        counter: i32,
    },
    /// GENERATE_CHAT_HEAD_FROM_BODY: model type 7 with three packed values.
    InterfaceBodyHead {
        pointer: i32,
        first: i32,
        second: i32,
        third: i32,
        counter: i32,
    },
    InterfaceTargetParam {
        pointer: i32,
        start: i32,
        end: i32,
        value: i32,
        counter: i32,
    },
    MoveInterface {
        source: i32,
        target: i32,
        counter: i32,
    },
    VarcString {
        id: i32,
        text: Vec<u8>,
        counter: i32,
    },
    RunTopLevelScripts,
    DeleteInventory {
        id: i32,
    },
    ExchangeOffer {
        slot: i32,
        offer: Option<Offer>,
    },
    ResetVarCache,
    RefreshVarps,
    ResetAnimations,
    MidiSong {
        id: i32,
    },
    MidiJingle {
        id: i32,
        delay: i32,
    },
    AreaSound {
        x: i32,
        y: i32,
        id: i32,
        radius: i32,
        loops: i32,
        delay: i32,
    },
    Projectile(HalfTileProjectile),
    /// The spatial branch of SPOTANIM_ENTITY stays with the world-spot reducer.
    ActorSpot {
        npc: bool,
        index: i32,
        spot: i32,
        height: i32,
        delay: i32,
    },
    NpcAnimation {
        npc: i32,
        delay: i32,
        sequence: i32,
    },
    LocAnimAt {
        x: i32,
        y: i32,
        plane: i32,
        shape: i32,
        rotation: i32,
        sequence: i32,
    },
    /// LOC_ADD_CHANGE. The pinned HD renderer discards it after reading.
    AttachLocation {
        x: i32,
        y: i32,
        shape: i32,
        rotation: i32,
        fine: [i32; 3],
        id: i32,
        sequence: i32,
        align: i32,
        duration: i32,
        ambience: i32,
    },
    TeleportSelf {
        x: i32,
        y: i32,
        plane: i32,
        jump: bool,
    },
    DynamicRebuild(InstanceRebuild),
    CameraReset {
        counter: i32,
    },
    CameraShake {
        counter: i32,
        axis: i32,
        jitter: i32,
        amplitude: i32,
        frequency: i32,
        phase: i32,
    },
    CameraLookAt {
        counter: i32,
        x: i32,
        y: i32,
        height: i32,
        min_step: i32,
        speed: i32,
    },
    CameraMoveTo {
        counter: i32,
        x: i32,
        y: i32,
        height: i32,
        min_step: i32,
        speed: i32,
    },
    CameraAngle {
        counter: i32,
        yaw: i32,
        pitch: i32,
    },
    RebootTimer {
        cycles: i32,
    },
    WalkText(Option<Vec<u8>>),
    LastLogin {
        address: i32,
    },
    SettingsString(Vec<u8>),
    OpenUrl(Vec<u8>),
    /// UPDATE_UID192 carries 24 opaque bytes for the client's `random.dat`.
    Uid(Vec<u8>),
    /// Reflection probes target the original JVM classes and have no native meaning.
    ReflectionCheck(Vec<u8>),
}

/// Bytes MAP_PROJANIM reads inside UPDATE_ZONE_PARTIAL_ENCLOSED.
pub const PROJECTILE_104_LENGTH: usize = 17;

fn none(value: i32) -> i32 {
    if value == 65535 { -1 } else { value }
}
fn sender(r: &mut Reader<'_>, name: i64) -> Result<Sender> {
    let top = i64::from(r.g2()?);
    let bot = i64::from(r.g3()?);
    Ok(Sender {
        name,
        message_id: (top << 32) + bot,
        rights: r.g1()?,
    })
}
fn rest(r: &mut Reader<'_>) -> Result<Vec<u8>> {
    Ok(r.take(r.remaining())?.to_vec())
}

impl ServerEvent {
    /// `None` leaves messages owned by other decoders (or unknown) as raw frames.
    pub fn decode(f: &Frame) -> Result<Option<Self>> {
        use ServerEvent::*;
        let mut r = Reader::new(&f.payload);
        let value = match f.opcode {
            0 => {
                let name = r.g8()?;
                PrivateMessage {
                    sender: sender(&mut r, name)?,
                    body: rest(&mut r)?,
                }
            }
            71 => PrivateEcho {
                name: r.g8()?,
                body: rest(&mut r)?,
            },
            247 => {
                let name = r.g8()?;
                let sender = sender(&mut r, name)?;
                PrivateQuickChat {
                    sender,
                    phrase: r.g2()?,
                    body: rest(&mut r)?,
                }
            }
            141 => PrivateQuickChatEcho {
                name: r.g8()?,
                phrase: r.g2()?,
                body: rest(&mut r)?,
            },
            54 => {
                let name = r.g8()?;
                r.g1b()?;
                let clan = r.g8()?;
                ClanMessage {
                    sender: sender(&mut r, name)?,
                    clan,
                    body: rest(&mut r)?,
                }
            }
            81 => {
                let name = r.g8()?;
                r.g1b()?;
                let clan = r.g8()?;
                let sender = sender(&mut r, name)?;
                ClanQuickChat {
                    sender,
                    clan,
                    phrase: r.g2()?,
                    body: rest(&mut r)?,
                }
            }
            55 => {
                let owner = r.g8()?;
                if owner == 0 {
                    return Ok(Some(ClanLeft));
                }
                let name = r.g8()?;
                let min_kick = r.g1b()?;
                let size = r.g1()?;
                let members = if size == 255 {
                    None
                } else {
                    let mut members = Vec::with_capacity(size as usize);
                    for _ in 0..size {
                        members.push(ClanMember {
                            name: r.g8()?,
                            world: r.g2()?,
                            rank: r.g1b()?,
                            world_name: r.gjstr()?.to_vec(),
                        });
                    }
                    Some(members)
                };
                // The original stops reading here; trailing bytes are not an error.
                return Ok(Some(ClanChannel {
                    owner,
                    name,
                    min_kick,
                    members,
                }));
            }
            196 => {
                let name = r.g8()?;
                let world = r.g2()?;
                let rank = r.g1b()?;
                if name < 0 {
                    ClanMemberLeft {
                        name: name & i64::MAX,
                        world,
                    }
                } else {
                    ClanMemberJoined(ClanMember {
                        name,
                        world,
                        rank,
                        world_name: r.gjstr()?.to_vec(),
                    })
                }
            }
            62 => {
                let name = r.g8()?;
                let world = r.g2()?;
                let rank = r.g1()?;
                Friend {
                    name: name & i64::MAX,
                    world,
                    rank,
                    same_game: name >= 0,
                    world_name: if world > 0 {
                        r.gjstr()?.to_vec()
                    } else {
                        vec![]
                    },
                }
            }
            197 => FriendListState(r.g1()?),
            126 => {
                let mut names = Vec::with_capacity(f.payload.len() / 8);
                for _ in 0..f.payload.len() / 8 {
                    names.push(r.g8()?);
                }
                return Ok(Some(IgnoreList(names)));
            }
            2 => {
                let pointer = r.img4()?;
                let counter = r.g2add()?;
                InterfaceColour {
                    pointer,
                    colour: r.ig2add()?,
                    counter,
                }
            }
            220 => {
                let pointer = r.mg4()?;
                let position = r.ig2()?;
                InterfaceScroll {
                    pointer,
                    position,
                    counter: r.g2()?,
                }
            }
            207 => {
                let pointer = r.mg4()?;
                let counter = r.g2add()?;
                let pitch_step = r.g2()?;
                InterfaceRotation {
                    pointer,
                    pitch_step,
                    yaw_step: r.g2add()?,
                    counter,
                }
            }
            111 => {
                let counter = r.g2add()?;
                let pointer = r.mg4()?;
                let first = r.ig2add()?;
                let second = r.ig2()?;
                InterfaceBodyHead {
                    pointer,
                    first,
                    second,
                    third: r.ig2add()?,
                    counter,
                }
            }
            9 => {
                let value = r.ig2add()?;
                let pointer = r.ig4()?;
                let counter = r.g2add()?;
                let end = none(r.ig2()?);
                InterfaceTargetParam {
                    pointer,
                    start: none(r.g2add()?),
                    end,
                    value,
                    counter,
                }
            }
            176 => {
                let source = r.img4()?;
                let counter = r.g2add()?;
                MoveInterface {
                    source,
                    target: r.img4()?,
                    counter,
                }
            }
            123 => {
                let id = r.ig2()?;
                let counter = r.g2add()?;
                VarcString {
                    id,
                    text: r.gjstr()?.to_vec(),
                    counter,
                }
            }
            48 => {
                let counter = r.g2()?;
                let text = r.gjstr()?.to_vec();
                VarcString {
                    id: r.ig2add()?,
                    text,
                    counter,
                }
            }
            209 => RunTopLevelScripts,
            191 => DeleteInventory { id: r.ig2()? },
            116 => {
                let slot = r.g1()?;
                let status = r.g1b()?;
                // An empty offer leaves the rest of the fixed 20-byte packet unread.
                return Ok(Some(ExchangeOffer {
                    slot,
                    offer: if status == 0 {
                        None
                    } else {
                        Some(Offer {
                            status,
                            item: r.g2()?,
                            price: r.g4()?,
                            count: r.g4()?,
                            completed_count: r.g4()?,
                            completed_gold: r.g4()?,
                        })
                    },
                }));
            }
            89 => ResetVarCache,
            128 => RefreshVarps,
            131 => ResetAnimations,
            4 => MidiSong {
                id: none(r.ig2add()?),
            },
            208 => {
                let delay = r.ig3()?;
                MidiJingle {
                    id: none(r.ig2()?),
                    delay,
                }
            }
            97 => {
                let c = r.g1()?;
                let id = none(r.g2()?);
                let packed = r.g1()?;
                AreaSound {
                    x: (c >> 4) & 7,
                    y: c & 7,
                    id,
                    radius: (packed >> 4) & 15,
                    loops: packed & 7,
                    delay: r.g1()?,
                }
            }
            104 | 121 => {
                // The original length table gives 104 no payload, so only the
                // zone envelope (which reads fields directly) can carry it.
                if f.opcode == 104 && f.payload.len() != PROJECTILE_104_LENGTH {
                    return Ok(None);
                }
                let c = r.g1()?;
                let delta = [r.g1b()?, r.g1b()?];
                // In 104 the first actor launches and the second is tracked.
                let first = r.g2b()?;
                let (source, target) = if f.opcode == 104 {
                    (Some(first), r.g2b()?)
                } else {
                    (None, first)
                };
                let spot = r.g2()?;
                let start_offset = if f.opcode == 104 {
                    r.g1b()?
                } else {
                    r.g1()? * 4
                };
                let end_height = r.g1()?;
                let start = r.g2()?;
                let end = r.g2()?;
                let angle = r.g1()?;
                Projectile(HalfTileProjectile {
                    offset: [(c >> 4) & 15, c & 15],
                    delta,
                    target,
                    source,
                    spot,
                    start_offset,
                    end_height,
                    start,
                    end,
                    angle: if angle == 255 { -1 } else { angle },
                    distance: r.g1()?,
                })
            }
            56 => {
                let delay = r.g2()?;
                let height = r.ig2()?;
                let target = r.img4()?;
                let spot = none(r.ig2add()?);
                if target >> 30 != 0 || target >> 28 == 0 {
                    return Ok(None);
                }
                ActorSpot {
                    npc: target >> 29 != 0,
                    index: target & 0xffff,
                    spot,
                    height,
                    delay,
                }
            }
            102 => {
                let npc = r.ig2()?;
                let delay = r.g1sub()?;
                NpcAnimation {
                    npc,
                    delay,
                    // The original compares this raw value against -1 unconverted.
                    sequence: r.g2()?,
                }
            }
            235 => {
                let packed = r.g1sub()?;
                let sequence = none(r.g2()?);
                let position = r.g4()?;
                LocAnimAt {
                    x: position >> 14 & 0x3fff,
                    y: position & 0x3fff,
                    plane: position >> 28 & 3,
                    shape: packed >> 2,
                    rotation: packed & 3,
                    sequence,
                }
            }
            202 => {
                let packed = r.g1()?;
                let c = r.g1()?;
                let fine = [r.g1badd()?, r.g1badd()?, r.g1bsub()?];
                let id = r.g2add()?;
                let sequence = r.ig2()?;
                let align = r.g1b()?;
                let duration = r.g2()?;
                AttachLocation {
                    x: (c >> 4) & 7,
                    y: c & 7,
                    shape: packed >> 2,
                    rotation: packed & 3,
                    fine,
                    id,
                    sequence,
                    align,
                    duration,
                    ambience: r.ig2badd()?,
                }
            }
            13 => {
                let x = r.g1sub()?;
                let flags = r.g1add()?;
                TeleportSelf {
                    x,
                    y: r.g1()?,
                    plane: flags >> 1,
                    jump: flags & 1 == 1,
                }
            }
            214 => {
                let local_x = r.ig2add()?;
                let zone_x = r.ig2add()?;
                let plane = r.g1sub()?;
                let local_y = r.ig2add()?;
                let mut chunks = Vec::with_capacity(4 * 13 * 13);
                let mut bits = r.bits();
                for _ in 0..4 * 13 * 13 {
                    chunks.push(if bits.read(1)? == 1 {
                        bits.read(26)? as i32
                    } else {
                        -1
                    });
                }
                bits.finish();
                let count = r.remaining() / 16;
                let mut keys = Vec::with_capacity(count);
                for _ in 0..count {
                    keys.push([r.mg4()?, r.mg4()?, r.mg4()?, r.mg4()?]);
                }
                DynamicRebuild(InstanceRebuild {
                    local_x,
                    local_y,
                    plane,
                    zone_x,
                    zone_y: r.g2()?,
                    chunks,
                    keys,
                })
            }
            24 => CameraReset { counter: r.g2()? },
            27 => CameraShake {
                counter: r.g2()?,
                axis: r.g1()?,
                jitter: r.g1()?,
                amplitude: r.g1()?,
                frequency: r.g1()?,
                phase: r.g2()?,
            },
            125 => {
                let counter = r.g2()?;
                let x = r.g1()?;
                let y = r.g1()?;
                CameraLookAt {
                    counter,
                    x,
                    y,
                    height: r.g2()?,
                    min_step: r.g1()?,
                    speed: r.g1()?,
                }
            }
            154 => {
                let counter = r.g2()?;
                let x = r.g1()?;
                let y = r.g1()?;
                CameraMoveTo {
                    counter,
                    x,
                    y,
                    height: r.g2()?,
                    min_step: r.g1()?,
                    speed: r.g1()?,
                }
            }
            187 => {
                let yaw = r.ig2()?;
                let counter = r.g2()?;
                CameraAngle {
                    counter,
                    yaw,
                    pitch: r.g2()?,
                }
            }
            85 => RebootTimer {
                cycles: r.g2()? * 30,
            },
            160 => WalkText(if f.payload.is_empty() {
                None
            } else {
                Some(r.gjstr()?.to_vec())
            }),
            164 => LastLogin { address: r.img4()? },
            142 => SettingsString(r.gjstr()?.to_vec()),
            42 => OpenUrl(rest(&mut r)?),
            169 => Uid(r.take(24)?.to_vec()),
            114 => ReflectionCheck(rest(&mut r)?),
            _ => return Ok(None),
        };
        if r.remaining() != 0 {
            return Err(Error::new(r.position(), "trailing server event bytes"));
        }
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        buffer::Writer,
        game::{Direction, Size},
    };

    fn frame(opcode: u8, w: Writer) -> Frame {
        Frame {
            opcode,
            payload: w.into_vec(),
        }
    }
    fn decode(opcode: u8, w: Writer) -> ServerEvent {
        ServerEvent::decode(&frame(opcode, w)).unwrap().unwrap()
    }

    /// Every fixed-length message consumes exactly the original PACKET_LENGTHS
    /// entry, which is an independent table from the read order above.
    #[test]
    fn fixed_messages_consume_the_original_packet_length() {
        let fixed = [
            2, 4, 9, 13, 24, 27, 56, 85, 89, 97, 102, 111, 116, 121, 125, 128, 131, 154, 164, 169,
            176, 187, 191, 197, 202, 207, 208, 209, 220, 235,
        ];
        for opcode in fixed {
            let Some(Size::Fixed(n)) = Direction::ServerToClient.size(opcode) else {
                panic!("opcode {opcode} is not fixed in the original table");
            };
            let mut payload = vec![0x91u8; n];
            if opcode == 56 {
                // Bit 29 of the middle-endian target selects the NPC branch.
                payload = vec![0; n];
                payload[6] = 0x20;
            }
            let full = Frame { opcode, payload };
            assert!(
                ServerEvent::decode(&full).unwrap().is_some(),
                "opcode {opcode}"
            );
            let mut short = full.clone();
            short.payload.pop();
            // 116 may legitimately leave unread tail bytes; all others
            // must fail when one byte is missing.
            if n > 0 && opcode != 116 {
                assert!(ServerEvent::decode(&short).is_err(), "short {opcode}");
            }
            let mut long = full.clone();
            long.payload.push(0);
            if opcode != 116 {
                assert!(ServerEvent::decode(&long).is_err(), "long {opcode}");
            }
        }
    }

    #[test]
    fn variable_messages_are_variable_in_the_original_table() {
        for opcode in [0, 54, 62, 71, 81, 123, 141, 196, 247, 42, 160, 142] {
            assert_eq!(
                Direction::ServerToClient.size(opcode),
                Some(Size::Byte),
                "{opcode}"
            );
        }
        for opcode in [48, 55, 114, 126, 214] {
            assert_eq!(
                Direction::ServerToClient.size(opcode),
                Some(Size::Short),
                "{opcode}"
            );
        }
    }

    /// PACKET_LENGTHS[104] is zero in the pinned client: a top-level
    /// MAP_PROJANIM has no fields, and only the envelope form is decodable.
    #[test]
    fn top_level_projectile_104_has_no_payload_in_the_original_table() {
        assert_eq!(Direction::ServerToClient.size(104), Some(Size::Fixed(0)));
        let empty = Frame {
            opcode: 104,
            payload: vec![],
        };
        assert_eq!(ServerEvent::decode(&empty).unwrap(), None);
        let enclosed = Frame {
            opcode: 104,
            payload: vec![0x11; PROJECTILE_104_LENGTH],
        };
        assert!(matches!(
            ServerEvent::decode(&enclosed).unwrap(),
            Some(ServerEvent::Projectile(_))
        ));
    }

    #[test]
    fn owned_elsewhere_or_unknown_frames_stay_raw() {
        // 192 and 217 belong to the minimap reducer.
        for opcode in [16, 17, 44, 70, 105, 153, 162, 192, 217, 225, 32, 86, 1, 255] {
            let f = Frame {
                opcode,
                payload: vec![0; 32],
            };
            assert_eq!(ServerEvent::decode(&f).unwrap(), None, "{opcode}");
        }
    }

    #[test]
    fn private_and_clan_chat_fields() {
        let mut w = Writer::new();
        w.p8(0x0123_4567_89ab_cdef);
        w.p2(0xbeef);
        w.p3(0x00c0_ffee);
        w.p1(2);
        w.pdata(&[3, 9, 9, 9]);
        assert_eq!(
            decode(0, w),
            ServerEvent::PrivateMessage {
                sender: Sender {
                    name: 0x0123_4567_89ab_cdef,
                    message_id: (0xbeef_i64 << 32) + 0x00c0_ffee,
                    rights: 2
                },
                body: vec![3, 9, 9, 9],
            }
        );
        let mut w = Writer::new();
        w.p8(5);
        w.p1(0x7f);
        w.p8(77);
        w.p2(1);
        w.p3(2);
        w.p1(0);
        w.p2(40);
        w.pdata(&[8]);
        assert_eq!(
            decode(81, w),
            ServerEvent::ClanQuickChat {
                sender: Sender {
                    name: 5,
                    message_id: (1i64 << 32) + 2,
                    rights: 0
                },
                clan: 77,
                phrase: 40,
                body: vec![8],
            }
        );
        let mut w = Writer::new();
        w.p8(5);
        w.p2(40);
        assert_eq!(
            decode(141, w),
            ServerEvent::PrivateQuickChatEcho {
                name: 5,
                phrase: 40,
                body: vec![]
            }
        );
    }

    #[test]
    fn clan_channel_join_leave_and_member_updates() {
        let mut w = Writer::new();
        w.p8(0);
        assert_eq!(decode(55, w), ServerEvent::ClanLeft);
        let mut w = Writer::new();
        w.p8(10);
        w.p8(20);
        w.p1(-3);
        w.p1(2);
        for (name, world, rank, text) in [(30i64, 1, 7, &b"World 1"[..]), (31, 2, -1, b"")] {
            w.p8(name);
            w.p2(world);
            w.p1(rank);
            w.pjstr(text).unwrap();
        }
        assert_eq!(
            decode(55, w),
            ServerEvent::ClanChannel {
                owner: 10,
                name: 20,
                min_kick: -3,
                members: Some(vec![
                    ClanMember {
                        name: 30,
                        world: 1,
                        rank: 7,
                        world_name: b"World 1".to_vec()
                    },
                    ClanMember {
                        name: 31,
                        world: 2,
                        rank: -1,
                        world_name: vec![]
                    },
                ]),
            }
        );
        let mut w = Writer::new();
        w.p8(10);
        w.p8(20);
        w.p1(0);
        w.p1(255);
        assert!(matches!(
            decode(55, w),
            ServerEvent::ClanChannel { members: None, .. }
        ));
        let mut w = Writer::new();
        w.p8(i64::MIN | 30);
        w.p2(4);
        w.p1(0);
        assert_eq!(
            decode(196, w),
            ServerEvent::ClanMemberLeft { name: 30, world: 4 }
        );
        let mut w = Writer::new();
        w.p8(30);
        w.p2(4);
        w.p1(-1);
        w.pjstr(b"W4").unwrap();
        assert_eq!(
            decode(196, w),
            ServerEvent::ClanMemberJoined(ClanMember {
                name: 30,
                world: 4,
                rank: -1,
                world_name: b"W4".to_vec()
            })
        );
    }

    #[test]
    fn friends_and_ignores() {
        let mut w = Writer::new();
        w.p8(i64::MIN | 9);
        w.p2(0);
        w.p1(3);
        assert_eq!(
            decode(62, w),
            ServerEvent::Friend {
                name: 9,
                world: 0,
                rank: 3,
                same_game: false,
                world_name: vec![]
            }
        );
        let mut w = Writer::new();
        w.p8(9);
        w.p2(12);
        w.p1(0);
        w.pjstr(b"RS 12").unwrap();
        assert_eq!(
            decode(62, w),
            ServerEvent::Friend {
                name: 9,
                world: 12,
                rank: 0,
                same_game: true,
                world_name: b"RS 12".to_vec()
            }
        );
        let mut w = Writer::new();
        w.p8(1);
        w.p8(2);
        w.p1(9);
        // The original uses length / 8 and ignores a partial trailing name.
        assert_eq!(decode(126, w), ServerEvent::IgnoreList(vec![1, 2]));
    }

    #[test]
    fn interface_messages_use_the_original_byte_orders() {
        let mut w = Writer::new();
        w.imp4(0x0102_0304);
        w.p2add(0x1234);
        w.ip2add(0x7fff);
        assert_eq!(
            decode(2, w),
            ServerEvent::InterfaceColour {
                pointer: 0x0102_0304,
                colour: 0x7fff,
                counter: 0x1234
            }
        );
        let mut w = Writer::new();
        w.mp4(0x0102_0304);
        w.ip2(300);
        w.p2(7);
        assert_eq!(
            decode(220, w),
            ServerEvent::InterfaceScroll {
                pointer: 0x0102_0304,
                position: 300,
                counter: 7
            }
        );
        let mut w = Writer::new();
        w.ip2add(11);
        w.ip4(0x0055_0002);
        w.p2add(3);
        w.ip2(65535);
        w.p2add(65535);
        assert_eq!(
            decode(9, w),
            ServerEvent::InterfaceTargetParam {
                pointer: 0x0055_0002,
                start: -1,
                end: -1,
                value: 11,
                counter: 3
            }
        );
        let mut w = Writer::new();
        w.imp4(1);
        w.p2add(2);
        w.imp4(3);
        assert_eq!(
            decode(176, w),
            ServerEvent::MoveInterface {
                source: 1,
                target: 3,
                counter: 2
            }
        );
        let mut w = Writer::new();
        w.ip2(5);
        w.p2add(6);
        w.pjstr(b"text").unwrap();
        let third = decode(123, w);
        let mut w = Writer::new();
        w.p2(6);
        w.pjstr(b"text").unwrap();
        w.ip2add(5);
        assert_eq!(decode(48, w), third);
        assert_eq!(
            third,
            ServerEvent::VarcString {
                id: 5,
                text: b"text".to_vec(),
                counter: 6
            }
        );
    }

    #[test]
    fn exchange_offer_and_empty_slot() {
        let mut w = Writer::new();
        w.p1(3);
        w.p1(0);
        w.pdata(&[0xaa; 18]);
        assert_eq!(
            decode(116, w),
            ServerEvent::ExchangeOffer {
                slot: 3,
                offer: None
            }
        );
        let mut w = Writer::new();
        w.p1(5);
        w.p1(0x8a);
        w.p2(4151);
        w.p4(2_000_000);
        w.p4(7);
        w.p4(3);
        w.p4(6_000_000);
        assert_eq!(
            decode(116, w),
            ServerEvent::ExchangeOffer {
                slot: 5,
                offer: Some(Offer {
                    status: -118,
                    item: 4151,
                    price: 2_000_000,
                    count: 7,
                    completed_count: 3,
                    completed_gold: 6_000_000
                })
            }
        );
    }

    #[test]
    fn world_effects() {
        let mut w = Writer::new();
        w.p1(0x35);
        w.p2(65535);
        w.p1(0xa3);
        w.p1(9);
        assert_eq!(
            decode(97, w),
            ServerEvent::AreaSound {
                x: 3,
                y: 5,
                id: -1,
                radius: 10,
                loops: 3,
                delay: 9
            }
        );
        let mut w = Writer::new();
        w.p1(0xfa);
        w.p1(-2);
        w.p1(3);
        w.p2(-5);
        w.p2(-9);
        w.p2(100);
        w.p1(-7);
        w.p1(10);
        w.p2(20);
        w.p2(60);
        w.p1(255);
        w.p1(16);
        assert_eq!(
            decode(104, w),
            ServerEvent::Projectile(HalfTileProjectile {
                offset: [15, 10],
                delta: [-2, 3],
                target: -9,
                source: Some(-5),
                spot: 100,
                start_offset: -7,
                end_height: 10,
                start: 20,
                end: 60,
                angle: -1,
                distance: 16
            })
        );
        let mut w = Writer::new();
        w.p1(0x12);
        w.p1(1);
        w.p1(-1);
        w.p2(0);
        w.p2(100);
        w.p1(200);
        w.p1(10);
        w.p2(20);
        w.p2(60);
        w.p1(8);
        w.p1(16);
        assert_eq!(
            decode(121, w),
            ServerEvent::Projectile(HalfTileProjectile {
                offset: [1, 2],
                delta: [1, -1],
                target: 0,
                source: None,
                spot: 100,
                start_offset: 800,
                end_height: 10,
                start: 20,
                end: 60,
                angle: 8,
                distance: 16
            })
        );
        for (target, expected) in [
            (
                (1 << 29) | 77,
                Some(ServerEvent::ActorSpot {
                    npc: true,
                    index: 77,
                    spot: -1,
                    height: 92,
                    delay: 4,
                }),
            ),
            (
                (1 << 28) | 2000,
                Some(ServerEvent::ActorSpot {
                    npc: false,
                    index: 2000,
                    spot: -1,
                    height: 92,
                    delay: 4,
                }),
            ),
            // Spatial and empty targets stay with the world-spot reducer.
            ((1 << 30) | 5, None),
            (5, None),
        ] {
            let mut w = Writer::new();
            w.p2(4);
            w.ip2(92);
            w.imp4(target);
            w.ip2add(65535);
            assert_eq!(ServerEvent::decode(&frame(56, w)).unwrap(), expected);
        }
        let mut w = Writer::new();
        w.ip2(300);
        w.p1sub(0);
        w.p2(65535);
        assert_eq!(
            decode(102, w),
            ServerEvent::NpcAnimation {
                npc: 300,
                delay: 0,
                sequence: 65535
            }
        );
        let mut w = Writer::new();
        w.p1sub((10 << 2) | 3);
        w.p2(65535);
        w.p4((2 << 28) | (3200 << 14) | 3400);
        assert_eq!(
            decode(235, w),
            ServerEvent::LocAnimAt {
                x: 3200,
                y: 3400,
                plane: 2,
                shape: 10,
                rotation: 3,
                sequence: -1
            }
        );
    }

    #[test]
    fn movement_and_camera() {
        let mut w = Writer::new();
        w.p1sub(50);
        w.p1add((2 << 1) | 1);
        w.p1(60);
        assert_eq!(
            decode(13, w),
            ServerEvent::TeleportSelf {
                x: 50,
                y: 60,
                plane: 2,
                jump: true
            }
        );
        let mut w = Writer::new();
        w.p2(9);
        w.p1(40);
        w.p1(50);
        w.p2(600);
        w.p1(3);
        w.p1(100);
        let look = decode(125, w.clone());
        assert_eq!(
            look,
            ServerEvent::CameraLookAt {
                counter: 9,
                x: 40,
                y: 50,
                height: 600,
                min_step: 3,
                speed: 100
            }
        );
        assert_eq!(
            decode(154, w),
            ServerEvent::CameraMoveTo {
                counter: 9,
                x: 40,
                y: 50,
                height: 600,
                min_step: 3,
                speed: 100
            }
        );
        let mut w = Writer::new();
        w.ip2(1024);
        w.p2(9);
        w.p2(200);
        assert_eq!(
            decode(187, w),
            ServerEvent::CameraAngle {
                counter: 9,
                yaw: 1024,
                pitch: 200
            }
        );
    }

    #[test]
    fn dynamic_rebuild_bits_keys_and_region_order() {
        let mut w = Writer::new();
        w.ip2add(52);
        w.ip2add(400);
        w.p1sub(1);
        w.ip2add(53);
        // Two present chunks; every other cell is a single zero bit.
        let first = (1 << 24) | (400 << 14) | (401 << 3) | (2 << 1);
        let second = (400 << 14) | (410 << 3);
        let mut bits: Vec<bool> = vec![];
        for index in 0..4 * 13 * 13 {
            let packed = match index {
                0 => Some(second),
                200 => Some(first),
                201 => Some(second),
                _ => None,
            };
            match packed {
                Some(v) => {
                    bits.push(true);
                    bits.extend((0..26).rev().map(|bit| (v >> bit) & 1 == 1));
                }
                None => bits.push(false),
            }
        }
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, bit) in bits.into_iter().enumerate() {
            if bit {
                bytes[i / 8] |= 1 << (7 - i % 8);
            }
        }
        w.pdata(&bytes);
        for key in [[1, 2, 3, 4], [5, 6, 7, 8]] {
            for word in key {
                w.mp4(word);
            }
        }
        w.p2(401);
        let ServerEvent::DynamicRebuild(rebuild) = decode(214, w) else {
            panic!("dynamic rebuild")
        };
        assert_eq!(
            (
                rebuild.local_x,
                rebuild.local_y,
                rebuild.plane,
                rebuild.zone_x,
                rebuild.zone_y
            ),
            (52, 53, 1, 400, 401)
        );
        assert_eq!(rebuild.chunk(0, 0, 0), second);
        assert_eq!(rebuild.chunk(1, 2, 5), first);
        assert_eq!(rebuild.chunk(1, 2, 6), second);
        assert_eq!(rebuild.chunk(3, 12, 12), -1);
        assert_eq!(rebuild.keys, vec![[1, 2, 3, 4], [5, 6, 7, 8]]);
        // (400/8 << 8) + 410/8 first, then + 401/8: both are region 50,51 / 50,50.
        assert_eq!(rebuild.regions(), vec![(50 << 8) + 51, (50 << 8) + 50]);
        assert_eq!(rebuild.base(), [(400 - 6) << 3, (401 - 6) << 3]);
    }

    #[test]
    fn small_and_opaque_messages() {
        let mut w = Writer::new();
        w.ip2add(65535);
        assert_eq!(decode(4, w), ServerEvent::MidiSong { id: -1 });
        let mut w = Writer::new();
        w.pdata(&[1, 2, 3]);
        w.ip2(350);
        assert_eq!(
            decode(208, w),
            ServerEvent::MidiJingle {
                id: 350,
                delay: (3 << 24) | (2 << 8) | 1
            }
        );
        let mut w = Writer::new();
        w.p2(10);
        assert_eq!(decode(85, w), ServerEvent::RebootTimer { cycles: 300 });
        assert_eq!(decode(160, Writer::new()), ServerEvent::WalkText(None));
        let mut w = Writer::new();
        w.pjstr(b"Dig here").unwrap();
        assert_eq!(
            decode(160, w),
            ServerEvent::WalkText(Some(b"Dig here".to_vec()))
        );
        let mut w = Writer::new();
        w.imp4(0x7f00_0001);
        assert_eq!(
            decode(164, w),
            ServerEvent::LastLogin {
                address: 0x7f00_0001
            }
        );
        for (opcode, expected) in [
            (89, ServerEvent::ResetVarCache),
            (128, ServerEvent::RefreshVarps),
            (131, ServerEvent::ResetAnimations),
            (209, ServerEvent::RunTopLevelScripts),
        ] {
            assert_eq!(decode(opcode, Writer::new()), expected);
        }
        let mut w = Writer::new();
        w.pdata(&[7; 24]);
        assert_eq!(decode(169, w), ServerEvent::Uid(vec![7; 24]));
    }
}
