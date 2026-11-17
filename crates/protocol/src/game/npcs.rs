//! Pure original RT4 NPC_INFO numeric reads. Actor motion, cache lookup and
//! animations remain consumer bindings; graphics are unnecessary for observations.
use crate::{
    Error, Result,
    buffer::{BitReader, Reader},
};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Local {
    Keep,
    Mask,
    Remove,
    Steps {
        directions: [usize; 2],
        count: u8,
        speed: u8,
        masked: bool,
    },
}
pub fn local(bits: &mut BitReader<'_, '_>) -> Result<Local> {
    if bits.read(1)? == 0 {
        return Ok(Local::Keep);
    }
    match bits.read(2)? {
        0 => Ok(Local::Mask),
        3 => Ok(Local::Remove),
        kind => {
            let (speed, count) = if kind == 1 {
                (1, 1)
            } else if bits.read(1)? == 1 {
                (2, 2)
            } else {
                (0, 1)
            };
            let mut directions = [0; 2];
            for direction in directions.iter_mut().take(count as usize) {
                *direction = bits.read(3)? as usize;
            }
            Ok(Local::Steps {
                directions,
                count,
                speed,
                masked: bits.read(1)? != 0,
            })
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Addition {
    pub index: usize,
    pub discard: bool,
    pub direction: usize,
    pub masked: bool,
    pub offset: [i32; 2],
    pub type_id: i32,
}
pub fn addition(bits: &mut BitReader<'_, '_>) -> Result<Option<Addition>> {
    // The original termination test is 27 bits even though an addition needs44.
    if bits.remaining() < 27 {
        return Ok(None);
    }
    let index = bits.read(15)? as usize;
    if index == 32767 {
        return Ok(None);
    }
    let discard = bits.read(1)? != 0;
    let direction = bits.read(3)? as usize;
    let masked = bits.read(1)? != 0;
    let y = bits.read(5)? as i32;
    let type_id = bits.read(14)? as i32;
    let x = bits.read(5)? as i32;
    let signed = |n: i32| if n > 15 { n - 32 } else { n };
    Ok(Some(Addition {
        index,
        discard,
        direction,
        masked,
        offset: [signed(x), signed(y)],
        type_id,
    }))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub damage: i32,
    pub kind: i32,
    pub ratio: Option<i32>,
}
/// Sequence ID, delay and slot mask, directly usable by actor animation queues.
pub type Slot = (i32, i32, i32);
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mask {
    pub flags: i32,
    pub primary: Option<Hit>,
    pub secondary: Option<Hit>,
    pub animation: Option<[i32; 2]>,
    pub face_entity: Option<i32>,
    pub spot: Option<[i32; 2]>,
    pub type_id: Option<i32>,
    pub chat: Option<Vec<u8>>,
    pub slots: Option<Vec<Slot>>,
    pub face_tile: Option<[i32; 2]>,
}
fn sentinel(n: i32) -> i32 {
    if n == 65535 { -1 } else { n }
}
pub fn mask(r: &mut Reader<'_>) -> Result<Mask> {
    let mut flags = r.g1()?;
    if flags & 8 != 0 {
        flags += r.g1()? << 8;
    }
    let mut m = Mask {
        flags,
        ..Mask::default()
    };
    if flags & 0x40 != 0 {
        m.primary = Some(Hit {
            damage: r.g1()?,
            kind: r.g1neg()?,
            ratio: Some(r.g1sub()?),
        });
    }
    if flags & 2 != 0 {
        m.secondary = Some(Hit {
            damage: r.g1neg()?,
            kind: r.g1sub()?,
            ratio: None,
        });
    }
    if flags & 0x10 != 0 {
        m.animation = Some([sentinel(r.g2()?), r.g1()?]);
    }
    if flags & 4 != 0 {
        m.face_entity = Some(sentinel(r.g2add()?));
    }
    if flags & 0x80 != 0 {
        m.spot = Some([sentinel(r.g2add()?), r.ig4()?]);
    }
    if flags & 1 != 0 {
        m.type_id = Some(r.ig2()?);
    }
    if flags & 0x20 != 0 {
        m.chat = Some(r.gjstr()?.to_vec());
    }
    if flags & 0x100 != 0 {
        let mut slots = vec![];
        for _ in 0..r.g1neg()? {
            slots.push((sentinel(r.ig2()?), r.g1sub()?, r.g2()?));
        }
        m.slots = Some(slots);
    }
    if flags & 0x200 != 0 {
        m.face_tile = Some([r.g2add()?, r.g2()?]);
    }
    Ok(m)
}
#[derive(Clone, Debug)]
pub struct Npc {
    pub type_id: i32,
    pub tile: [i32; 2],
    pub direction: usize,
    pub last_seen: i32,
    pub last_mask: Mask,
}
#[derive(Default)]
pub struct State {
    pub actors: BTreeMap<usize, Npc>,
    pub local_ids: Vec<usize>,
}
impl State {
    /// Queue-head grid positions from the authoritative stream. This does not
    /// simulate fine coordinates, animations, NPC metadata or combat rules.
    pub fn decode(&mut self, frame: &super::Frame, self_tile: [i32; 2], cycle: i32) -> Result<()> {
        if frame.opcode != 32 {
            return Err(Error::new(0, "not NPC_INFO"));
        }
        let mut r = Reader::new(&frame.payload);
        let mut bits = r.bits();
        let count = bits.read(8)? as usize;
        if count > self.local_ids.len() {
            return Err(Error::new(0, "NPC local count exceeds previous list"));
        }
        let old = std::mem::take(&mut self.local_ids);
        let mut removed = old[count..].to_vec();
        let mut extended = vec![];
        for &index in &old[..count] {
            let npc = self
                .actors
                .get_mut(&index)
                .ok_or(Error::new(0, "missing local NPC"))?;
            let command = local(&mut bits)?;
            if command == Local::Remove {
                removed.push(index);
                continue;
            }
            self.local_ids.push(index);
            npc.last_seen = cycle;
            match command {
                Local::Mask => extended.push(index),
                Local::Steps {
                    directions,
                    count,
                    masked,
                    ..
                } => {
                    for d in directions.into_iter().take(count as usize) {
                        let [dx, dy] = [
                            [-1, 1],
                            [0, 1],
                            [1, 1],
                            [-1, 0],
                            [1, 0],
                            [-1, -1],
                            [0, -1],
                            [1, -1],
                        ][d];
                        npc.tile = [npc.tile[0].wrapping_add(dx), npc.tile[1].wrapping_add(dy)];
                    }
                    if masked {
                        extended.push(index);
                    }
                }
                _ => {}
            }
        }
        while let Some(a) = addition(&mut bits)? {
            let npc = self.actors.entry(a.index).or_insert_with(|| Npc {
                type_id: -1,
                tile: [0; 2],
                direction: a.direction,
                last_seen: cycle,
                last_mask: Mask::default(),
            });
            npc.type_id = a.type_id;
            npc.tile = [
                self_tile[0].wrapping_add(a.offset[0]),
                self_tile[1].wrapping_add(a.offset[1]),
            ];
            npc.last_seen = cycle;
            self.local_ids.push(a.index);
            if a.masked {
                extended.push(a.index);
            }
        }
        bits.finish();
        for index in extended {
            let m = mask(&mut r)?;
            let npc = self
                .actors
                .get_mut(&index)
                .ok_or(Error::new(r.position(), "missing extended NPC"))?;
            if let Some(id) = m.type_id {
                npc.type_id = id;
            }
            npc.last_mask = m;
        }
        for index in removed {
            if self
                .actors
                .get(&index)
                .is_some_and(|n| n.last_seen != cycle)
            {
                self.actors.remove(&index);
            }
        }
        if r.remaining() != 0 {
            return Err(Error::new(r.position(), "NPC_INFO trailing bytes"));
        }
        for i in &self.local_ids {
            if !self.actors.contains_key(i) {
                return Err(Error::new(r.position(), "missing NPC local membership"));
            }
        }
        Ok(())
    }
}
