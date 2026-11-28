//! `FishingNPC` in the live world: the NPC behaviour of every fishing spot
//! id (`world::gathering::fishing_npc`), which moves a spot between its
//! area's tiles.
//!
//! The behaviour's hooks need the whole world (the shared generator, the
//! region chunks' NPC lists, the NPC's teleport location), so they do not go
//! through the start-up's behaviour registry: `Live::npc_tick_of` runs
//! `FishingNPC.tick` right before the NPC's tick kernel whenever the kernel
//! will reach `handleTickActions` (an active region, no pending respawn
//! tick), and the kernel's `behavior.tick` query answers `false` for a
//! fishing spot, as the original always does.
//!
//! `onCreation` runs on the spot's first such tick, with the NPC's spawn
//! location (where the NPC stands when the original calls it). The two
//! attributes it writes (`fishing:spot`, `fishing:switchdelay`) have no other
//! reader and live in [`FishingSpotWorld`].
use std::collections::BTreeMap;

use super::{P, host::Live};
use crate::world::{
    doors::Location,
    entity_regions::Select,
    gathering::fishing_npc::{self, FishSpots, Tile},
    npc,
    player_walk::Node,
};

/// Each fishing spot's `fishing:spot` and `fishing:switchdelay`.
#[derive(Default)]
pub struct FishingSpotWorld {
    pub spots: BTreeMap<Node, (Option<FishSpots>, i32)>,
}

fn bind(e: impl std::fmt::Debug) -> npc::Error {
    npc::Error::Binding(format!("FishingNPC: {e:?}"))
}

struct Host<'a> {
    live: &'a mut Live,
}

impl fishing_npc::Host for Host<'_> {
    type Npc = P;
    type Error = npc::Error;

    fn spot(&mut self, npc: &P) -> npc::Result<Option<FishSpots>> {
        let spots = &self.live.round.fishing_spots.spots;
        Ok(spots.get(&npc.node()).and_then(|s| s.0))
    }
    fn set_spot(&mut self, npc: &P, spot: FishSpots) -> npc::Result<()> {
        let spots = &mut self.live.round.fishing_spots.spots;
        spots.entry(npc.node()).or_default().0 = Some(spot);
        Ok(())
    }
    fn switch_delay(&mut self, npc: &P) -> npc::Result<i32> {
        let spots = &self.live.round.fishing_spots.spots;
        Ok(spots.get(&npc.node()).map_or(0, |s| s.1))
    }
    fn set_switch_delay(&mut self, npc: &P, value: i32) -> npc::Result<()> {
        let spots = &mut self.live.round.fishing_spots.spots;
        spots.entry(npc.node()).or_default().1 = value;
        Ok(())
    }
    fn location(&mut self, npc: &P) -> npc::Result<Tile> {
        // onCreation: the NPC stands on its spawn location.
        let node = npc.node();
        let spawn = self
            .live
            .inner
            .npcs
            .rows
            .get(&node)
            .and_then(|row| row.properties.spawn.clone());
        let at = match spawn {
            Some(at) => Some(at),
            None => self.live.inner.state.node_location(node).map_err(bind)?,
        };
        at.map(|l| l.coordinates()).ok_or(npc::Error::Null)
    }
    fn world_ticks(&mut self) -> npc::Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn random(&mut self, max: i32) -> npc::Result<i32> {
        Ok(crate::world::drop_tables::random::random(
            &mut self.live.inner.generators,
            max,
        ))
    }
    fn random_range(&mut self, a: i32, b: i32) -> npc::Result<i32> {
        Ok(crate::world::drop_tables::random::random_range(
            &mut self.live.inner.generators,
            a,
            b,
        ))
    }
    fn no_npc_at(&mut self, tile: Tile) -> npc::Result<bool> {
        let [x, y, z] = tile;
        self.live
            .local(Location::new(x, y, z), 0, Select::Npcs)
            .map(|found| found.is_empty())
            .map_err(bind)
    }
    fn teleport(&mut self, npc: &P, tile: Tile) -> npc::Result<()> {
        let [x, y, z] = tile;
        self.live
            .inner
            .state
            .actor(npc.node())
            .map_err(bind)?
            .queue
            .state
            .write()
            .entity
            .teleport_location = Some(Location::new(x, y, z));
        Ok(())
    }
}

impl Live {
    /// Whether `FishingNPC` is this NPC's behaviour.
    pub(super) fn is_fishing_spot(&self, node: Node) -> bool {
        let row = self.inner.npcs.rows.get(&node);
        row.is_some_and(|row| fishing_npc::is_spot(row.id))
    }

    /// `FishingNPC.tick(self)` (and `onCreation` before the first one).
    pub(super) fn fishing_spot_tick(&mut self, npc: &P) -> npc::Result<()> {
        let created = self.round.fishing_spots.spots.contains_key(&npc.node());
        let mut host = Host { live: self };
        if !created {
            fishing_npc::on_creation(&mut host, npc)?;
        }
        fishing_npc::tick(&mut host, npc).map(|_| ())
    }
}
