//! Normal revision-530 scene rebuild fields. Dynamic rebuilds use another format.
use super::Frame;
use crate::{Error, Result, buffer::Reader};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalRebuild {
    pub local_x: i32,
    pub local_y: i32,
    pub plane: i32,
    pub zone_x: i32,
    pub zone_y: i32,
    pub keys: Vec<[i32; 4]>,
}
impl NormalRebuild {
    /// Original client enumeration order, including blacklisted region entries.
    pub fn regions(&self) -> Vec<i32> {
        let mut out = vec![];
        for x in (self.zone_x - 6) / 8..=(self.zone_x + 6) / 8 {
            for y in (self.zone_y - 6) / 8..=(self.zone_y + 6) / 8 {
                out.push((x << 8) + y);
            }
        }
        out
    }
    pub fn base(&self) -> [i32; 2] {
        [(self.zone_x - 6) << 3, (self.zone_y - 6) << 3]
    }
}
pub fn decode_normal(frame: &Frame) -> Result<NormalRebuild> {
    if frame.opcode != 162 {
        return Err(Error::new(0, "not a normal scene rebuild"));
    }
    let mut r = Reader::new(&frame.payload);
    let local_x = r.g2add()?;
    let count = r.remaining() / 16;
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        keys.push([r.mg4()?, r.mg4()?, r.mg4()?, r.mg4()?]);
    }
    let plane = r.g1sub()?;
    let zone_x = r.g2()?;
    let zone_y = r.g2add()?;
    let local_y = r.g2add()?;
    if r.remaining() != 0 {
        return Err(Error::new(r.position(), "trailing normal rebuild bytes"));
    }
    Ok(NormalRebuild {
        local_x,
        local_y,
        plane,
        zone_x,
        zone_y,
        keys,
    })
}
