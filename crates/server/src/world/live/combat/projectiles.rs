//! Projectiles in the live world: `Projectile` (create, ranged, magic,
//! copy) and `Projectile.send`, which flags the region chunk of the source
//! location with a `ProjectileUpdateFlag`; the flag is written by
//! `RegionChunk.update` for every player who sees the chunk, in the order
//! the chunk's flags were added, and dropped with the other chunk flags by
//! `ChunkUpdateTracker.resetAll`.
//!
//! The flag is a `chunk_sync::Flag::Foreign` whose key names a projectile
//! here; its bytes are `ProjectileUpdateFlag.write`'s (opcode 16 inside the
//! chunk update packet), with the end location and the target index read
//! when it is written, as the original does.
//!
//! Ported from the pinned `Projectile.java` and `ProjectileUpdateFlag.java`
//! without a differential corpus of their own (the projectile oracle covers
//! the path and line-of-sight queries, not this class).
use super::super::{P, host::Live};
use crate::{
    definitions::item::Shared,
    world::{
        chunk_sync::{self, Buffer, Flag},
        combat_swing as cs,
        doors::Location,
        ground_manager as gm,
        player_walk::{Node, NodeHandle},
    },
};
use std::collections::HashMap;

/// The fields of one `Projectile` object.
#[derive(Clone, Debug)]
pub struct Projectile {
    pub source_location: Option<Location>,
    pub victim: Option<Node>,
    pub end_location: Option<Location>,
    pub id: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start_delay: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
}

/// The projectiles flagged on chunks during the current tick.
#[derive(Default)]
pub struct Projectiles {
    flagged: HashMap<usize, Projectile>,
    next: usize,
}

/// Keys of foreign chunk flags this module owns start here.
const BASE: usize = 1 << 40;

impl Projectiles {
    /// `ChunkUpdateTracker.resetAll`: the chunk flags are dropped, and with
    /// them the projectiles they named.
    pub fn clear(&mut self) {
        self.flagged.clear();
    }
    pub fn get(&self, key: usize) -> Option<&Projectile> {
        self.flagged.get(&key)
    }
    pub fn owns(key: usize) -> bool {
        key >= BASE
    }
    /// How many projectiles were sent since the last reset.
    pub fn len(&self) -> usize {
        self.flagged.len()
    }
    pub fn is_empty(&self) -> bool {
        self.flagged.is_empty()
    }
}

impl Live {
    /// `Node.size()`.
    fn entity_size(&self, node: Node) -> i32 {
        self.inner
            .npcs
            .rows
            .get(&node)
            .map_or(1, |row| row.definition.read().size)
    }
    /// `Node.getLocation()`.
    fn entity_location(&self, node: Node) -> Option<Location> {
        self.inner.state.node_location(node).ok().flatten()
    }
    /// `Projectile.getLocation(entity)`: `Node.getCenterLocation()`.
    fn center_location(&self, node: Node) -> Option<Location> {
        let offset = self.entity_size(node) >> 1;
        self.entity_location(node).map(|l| {
            let [x, y, z] = l.coordinates();
            Location::new(x.wrapping_add(offset), y.wrapping_add(offset), z)
        })
    }

    /// `Projectile.create(source, victim, id, startHeight, endHeight,
    /// startDelay, speed, angle)`: distance `source.size() << 5`.
    #[allow(clippy::too_many_arguments)]
    pub fn projectile_create(
        &self,
        source: &P,
        victim: Option<&P>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        speed: i32,
        angle: i32,
    ) -> Projectile {
        Projectile {
            source_location: self.center_location(source.node()),
            victim: victim.map(P::node),
            end_location: None,
            id,
            start_height,
            end_height,
            start_delay,
            speed,
            angle,
            distance: self.entity_size(source.node()) << 5,
        }
    }

    /// `Projectile.ranged`/`magic`: speed `(int) (46 + distance * factor)`
    /// from the source's center to the victim's location.
    #[allow(clippy::too_many_arguments)]
    pub fn projectile_speed_based(
        &self,
        source: &P,
        victim: &P,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        angle: i32,
        factor: f64,
    ) -> cs::Result<Projectile> {
        let from = self.center_location(source.node()).ok_or(cs::Error::Null)?;
        let to = self.entity_location(victim.node()).ok_or(cs::Error::Null)?;
        let speed = (46.0 + cs::distance(from.coordinates(), to.coordinates()) * factor) as i32;
        Ok(self.projectile_create(
            source,
            Some(victim),
            id,
            start_height,
            end_height,
            start_delay,
            speed,
            angle,
        ))
    }

    /// `template.copy(source, victim, speedMultiplier)`: the template's
    /// numbers, speed `(int) (speed + distance * multiplier)` over the two
    /// entities' locations, the template's distance.
    pub fn projectile_copy(
        &self,
        template: &crate::startup::loaders::ranged::ProjectileTemplate,
        source: &P,
        victim: &P,
        multiplier: f64,
    ) -> cs::Result<Projectile> {
        let from = self.entity_location(source.node()).ok_or(cs::Error::Null)?;
        let to = self.entity_location(victim.node()).ok_or(cs::Error::Null)?;
        let speed = (f64::from(template.speed)
            + cs::distance(from.coordinates(), to.coordinates()) * multiplier)
            as i32;
        Ok(Projectile {
            source_location: self.center_location(source.node()),
            victim: Some(victim.node()),
            end_location: None,
            id: template.projectile_id,
            start_height: template.start_height,
            end_height: template.end_height,
            start_delay: template.start_delay,
            speed,
            angle: template.angle,
            distance: template.distance,
        })
    }

    /// `Projectile.send()`: `RegionManager.getRegionChunk(sourceLocation)
    /// .flag(new ProjectileUpdateFlag(this))`.
    pub fn projectile_send(&mut self, projectile: Projectile) -> cs::Result<()> {
        let location = projectile.source_location.clone().ok_or(cs::Error::Null)?;
        let key = BASE + self.inner.combat.projectiles.next;
        self.inner.combat.projectiles.next += 1;
        self.inner
            .combat
            .projectiles
            .flagged
            .insert(key, projectile);
        let chunk =
            gm::Context::chunk(self, location).map_err(|e| cs::Error::Binding(format!("{e:?}")))?;
        chunk_sync::flag_with(&chunk, Some(Shared::new(Flag::Foreign(key))), &mut |c| {
            gm::Context::flagged(self, c).map_err(|e| cs::Error::Binding(format!("{e:?}")))
        })
    }

    /// `ProjectileUpdateFlag.write(buffer)` for the flag `key`.
    pub fn write_projectile_flag(&mut self, key: usize, buffer: &Buffer) -> chunk_sync::Result<()> {
        let projectile = self
            .inner
            .combat
            .projectiles
            .get(key)
            .cloned()
            .ok_or_else(|| chunk_sync::Error::Binding(format!("an unknown chunk flag {key}")))?;
        let start = projectile
            .source_location
            .clone()
            .ok_or(chunk_sync::Error::Null)?;
        let end = if projectile.end_location.is_some() {
            projectile.end_location.clone()
        } else {
            let victim = projectile.victim.ok_or(chunk_sync::Error::Null)?;
            self.entity_location(victim)
        };
        let end = end.ok_or(chunk_sync::Error::Null)?;
        let target = match projectile.victim {
            None => -1,
            Some(node) => match self.inner.players.get(&node) {
                Some(slot) => slot.index.wrapping_add(1).wrapping_neg(),
                None => self
                    .inner
                    .state
                    .actor(node)
                    .map(|a| a.wire_index)
                    .unwrap_or(-1)
                    .wrapping_add(1),
            },
        };
        let [sx, sy, _] = start.coordinates();
        let [ex, ey, _] = end.coordinates();
        // put(16), put(offsets), put(dx), put(dy), putShort(target),
        // putShort(id), put(heights), putShort(delay), putShort(speed),
        // put(angle), put(distance): the shared opcode-16 writer.
        let frame = rs09_protocol::game::updates::Projectile {
            offset: [sx & 7, sy & 7],
            delta: [ex.wrapping_sub(sx), ey.wrapping_sub(sy)],
            target,
            spot: projectile.id,
            start_height: projectile.start_height,
            end_height: projectile.end_height,
            start: projectile.start_delay,
            end: projectile.speed,
            angle: projectile.angle,
            distance: projectile.distance,
        }
        .frame();
        let mut out = buffer.write();
        out.payload.push(frame.opcode);
        out.payload.extend_from_slice(&frame.payload);
        Ok(())
    }
}

/// The entity of a projectile's victim, for callers holding a node handle.
pub fn victim_of(node: &NodeHandle) -> P {
    P(node.clone())
}
