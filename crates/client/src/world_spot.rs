//! Original world-positioned SpotAnim placement, finite clocks and scene lifetime.
//! Height/cache lookup and actual scene draw submission are caller services.
use crate::animation::Sequence;
use rs09_protocol::{Error, Result, buffer::Reader, game::Frame};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Effect {
    pub id: i32,
    pub plane: i32,
    /// Scene-local fine coordinates, retained as in the original SpotAnim.
    pub fine: [i32; 2],
    pub z: i32,
    pub start: i32,
    pub sequence: i32,
    pub frame: usize,
    pub clock: i32,
    pub finished: bool,
}
impl Effect {
    pub fn advance(&mut self, delta: i32, sequence: &Sequence) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.clock = self.clock.wrapping_add(delta);
        loop {
            let delay = *sequence
                .delays
                .get(self.frame)
                .ok_or(Error::new(0, "World spot frame unavailable"))?;
            if self.clock <= i32::from(delay) {
                break;
            }
            self.clock = self.clock.wrapping_sub(i32::from(delay));
            self.frame += 1;
            if self.frame >= sequence.frames.len() {
                self.finished = true;
                break;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct State {
    pub zone: [i32; 2],
    pub effects: Vec<Effect>,
    pub projectiles: Vec<crate::projectile::Flight>,
    /// Spot ids received whose definition or sequence was not to be had;
    /// the caller takes and reports them.
    pub unavailable: Vec<i32>,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub origin: [i32; 2],
    pub plane: i32,
    pub cycle: i32,
}
impl State {
    /// Accepts original zone17 and the spatial branch of56. Actor56 requests
    /// remain a separate binding; returning false keeps that gap visible.
    pub fn receive(
        &mut self,
        frame: &Frame,
        context: Context,
        spots: &BTreeMap<i32, i32>,
        sequences: &BTreeMap<u32, Sequence>,
        height: &mut impl FnMut(i32, [i32; 2]) -> i32,
    ) -> Result<bool> {
        use rs09_protocol::game::updates::ServerUpdate;
        if matches!(frame.opcode, 26 | 112 | 230) {
            match ServerUpdate::decode(frame)? {
                Some(ServerUpdate::ZoneOrigin { x, y, .. }) => self.zone = [x, y],
                Some(ServerUpdate::ZoneEnclosed { x, y, frames }) => {
                    self.zone = [x, y];
                    for child in frames {
                        self.receive(&child, context, spots, sequences, height)?;
                    }
                }
                _ => {}
            }
            return Ok(true);
        }
        if frame.opcode == 16 {
            let packet = rs09_protocol::game::updates::Projectile::decode(frame)?;
            let tile: [i32; 2] = std::array::from_fn(|i| self.zone[i] + packet.offset[i]);
            let end: [i32; 2] = std::array::from_fn(|i| tile[i] + packet.delta[i]);
            if packet.spot == 65535
                || packet.end < packet.start
                || !tile.into_iter().chain(end).all(|v| (0..104).contains(&v))
                || self.projectiles.len() >= 1024
            {
                return Ok(true);
            }
            let fine = tile.map(|n| n * 128 + 64);
            let z = height(context.plane, fine);
            let source = std::array::from_fn(|i| fine[i] + context.origin[i] * 128);
            self.projectiles.push(crate::projectile::Flight::new(
                packet,
                context.plane,
                context.cycle,
                source,
                z,
            ));
            return Ok(true);
        }
        let mut r = Reader::new(&frame.payload);
        let (tile, plane, id, offset, delay) = match frame.opcode {
            17 => {
                let coords = r.g1()?;
                let tile = [
                    self.zone[0].wrapping_add((coords >> 4) & 7),
                    self.zone[1].wrapping_add(coords & 7),
                ];
                (tile, context.plane, r.g2()?, r.g1()?, r.g2()?)
            }
            56 => {
                let delay = r.g2()?;
                let offset = r.ig2()?;
                let target = r.img4()?;
                let id = r.ig2add()?;
                if target >> 30 == 0 {
                    return Ok(false);
                }
                (
                    [
                        (target >> 14 & 0x3fff).wrapping_sub(context.origin[0]),
                        (target & 0x3fff).wrapping_sub(context.origin[1]),
                    ],
                    target >> 28 & 3,
                    id,
                    offset,
                    delay,
                )
            }
            _ => return Ok(false),
        };
        if r.remaining() != 0 {
            return Err(Error::new(r.position(), "Trailing world spot packet"));
        }
        if tile.into_iter().any(|v| !(0..104).contains(&v)) {
            return Ok(true);
        }
        if self.effects.len() >= 1024 {
            return Err(Error::new(0, "World spot scene limit exceeded"));
        }
        let fine = tile.map(|n| n * 128 + 64);
        // Original height is captured once, before the definition/sequence query.
        let z = height(plane, fine).wrapping_sub(offset);
        // SpotAnimTypeList.get always answers. A definition or sequence this
        // client does not have (prepared packs without a cache) places
        // nothing; the caller reports it and the session goes on.
        let Some(&sequence) = spots.get(&id) else {
            self.unavailable.push(id);
            return Ok(true);
        };
        if sequence != -1 && !sequences.contains_key(&(sequence as u32)) {
            self.unavailable.push(id);
            return Ok(true);
        }
        self.effects.push(Effect {
            id,
            plane,
            fine,
            z,
            start: context.cycle.wrapping_add(delay),
            sequence,
            frame: 0,
            clock: 0,
            finished: sequence == -1,
        });
        Ok(true)
    }
    /// MAP_PROJANIM (104) and MAP_PROJANIM_3 (121): half-tile endpoints relative
    /// to twice the zone origin. `launcher` resolves 104's launching actor to its
    /// BAS slot transform and current angle; the rotated transform moves the
    /// source before the terrain height is sampled, as in the original.
    pub fn launch(
        &mut self,
        packet: &rs09_protocol::game::events::HalfTileProjectile,
        context: Context,
        launcher: impl FnOnce(bool, usize, usize) -> Option<([i32; 6], i32)>,
        height: &mut impl FnMut(i32, [i32; 2]) -> i32,
    ) {
        let source: [i32; 2] = std::array::from_fn(|i| self.zone[i] * 2 + packet.offset[i]);
        let end: [i32; 2] = std::array::from_fn(|i| source[i] + packet.delta[i]);
        if packet.spot == 65535
            || packet.end < packet.start
            || !source.into_iter().chain(end).all(|v| (0..208).contains(&v))
            || self.projectiles.len() >= 1024
        {
            return;
        }
        let mut fine = source.map(|n| n * 64);
        let mut offset = packet.start_offset;
        if let Some(id) = packet.source.filter(|&id| id != 0) {
            let data = if id >= 0 { id - 1 } else { -id - 1 };
            let (index, slot) = ((data & 0x7ff) as usize, (data >> 11 & 15) as usize);
            if let Some((transform, angle)) = launcher(id >= 0, index, slot) {
                let (sin, cos) = crate::animation::sin_cos(angle);
                let [x, y, z, ..] = transform;
                offset = offset.wrapping_sub(y);
                fine[0] += x.wrapping_mul(cos).wrapping_add(z.wrapping_mul(sin)) >> 16;
                fine[1] += cos.wrapping_mul(z).wrapping_sub(x.wrapping_mul(sin)) >> 16;
            }
        }
        let z = height(context.plane, fine).wrapping_sub(offset);
        let absolute = |local: [i32; 2]| -> [i32; 2] {
            std::array::from_fn(|i| local[i] + context.origin[i] * 128)
        };
        self.projectiles.push(crate::projectile::Flight::launch(
            rs09_protocol::game::updates::Projectile {
                offset: [0; 2],
                delta: [0; 2],
                target: packet.target,
                spot: packet.spot,
                start_height: 0,
                end_height: packet.end_height,
                start: packet.start,
                end: packet.end,
                angle: packet.angle,
                distance: packet.distance,
            },
            context.plane,
            context.cycle,
            absolute(fine),
            absolute(end.map(|n| n * 64)),
            z,
        ));
    }
    /// Original ScriptRunner.updateSceneSpotAnims: wrong-plane/finished removal
    /// precedes the start test; advance occurs before the draw decision.
    pub fn update(
        &mut self,
        plane: i32,
        cycle: i32,
        delta: i32,
        sequences: &BTreeMap<u32, Sequence>,
    ) -> Result<()> {
        let mut i = 0;
        while i < self.effects.len() {
            let e = &mut self.effects[i];
            if e.plane != plane || e.finished {
                self.effects.remove(i);
                continue;
            }
            if e.start <= cycle {
                e.advance(
                    delta,
                    sequences
                        .get(&(e.sequence as u32))
                        .ok_or(Error::new(0, "World spot sequence unavailable"))?,
                )?;
                if e.finished {
                    self.effects.remove(i);
                    continue;
                }
            }
            i += 1;
        }
        Ok(())
    }
    pub fn visible(&self, cycle: i32) -> impl Iterator<Item = &Effect> {
        self.effects
            .iter()
            .filter(move |e| !e.finished && e.start <= cycle)
    }
}
