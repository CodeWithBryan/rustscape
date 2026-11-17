//! World-list payload and packet framing used by revision 530.
use crate::{Error, Result, buffer::Writer};

#[derive(Clone, Debug)]
pub struct World {
    pub id: u16,
    pub location: u8,
    pub flags: u32,
    pub activity: Vec<u8>,
    pub host: Vec<u8>,
    pub region: Vec<u8>,
    pub country: u16,
    pub players: u16,
}
impl Default for World {
    fn default() -> Self {
        Self {
            id: 1,
            location: 0,
            flags: 9,
            activity: b"2009Scape Classic".to_vec(),
            host: b"127.0.0.1".to_vec(),
            region: b"Anywhere, USA".to_vec(),
            country: 225,
            players: 0,
        }
    }
}
pub fn world_list(worlds: &[World], current_stamp: i32, requested_stamp: i32) -> Result<Vec<u8>> {
    let mut payload = Writer::new();
    payload.p1(1);
    let updated = current_stamp != requested_stamp;
    payload.p1(i32::from(updated));
    if updated {
        payload
            .psmarts(i32::try_from(worlds.len()).map_err(|_| Error::new(0, "too many worlds"))?)?;
        for world in worlds {
            payload.psmarts(i32::from(world.country))?;
            payload.p1(0);
            payload.pjstr(&world.region)?;
        }
        payload.psmarts(0)?;
        payload.psmarts(worlds.len() as i32)?;
        payload.psmarts(worlds.len() as i32)?;
        for world in worlds {
            payload.psmarts(i32::from(world.id))?;
            payload.p1(i32::from(world.location));
            payload.p4(world.flags as i32);
            payload.p1(0);
            payload.pjstr(&world.activity)?;
            payload.p1(0);
            payload.pjstr(&world.host)?;
        }
        payload.p4(current_stamp);
    }
    for world in worlds {
        payload.psmarts(i32::from(world.id))?;
        payload.p2(i32::from(world.players));
    }
    if payload.as_slice().len() > u16::MAX as usize {
        return Err(Error::new(0, "world list exceeds packet length"));
    }
    let mut out = Writer::new();
    out.p1(0);
    out.p2(payload.as_slice().len() as i32);
    out.pdata(payload.as_slice());
    Ok(out.into_vec())
}
