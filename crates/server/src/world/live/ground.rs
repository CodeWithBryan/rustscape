//! Chunk synchronisation for the live world's map chunk renderer
//! (`world::chunk_sync`): `RegionChunk.synchronize`, `update` and
//! `ClearRegionChunk` over the scene's chunks, their object layers and
//! their ground-item lists, written into the player's write queue.
//!
//! Ground items reach these lists through the ground-item manager
//! ([`super::ground_items`]); a `GroundSpawn` answers its own privacy and
//! activity, as in the manager.
use super::{P, host::Live};
use crate::{
    inventory::item,
    world::{
        chunk::HandleChunk,
        chunk_sync::{self, Buffer},
        doors::{Handle as Scenery, Location},
        ground_item::{self as ground, Handle as Ground},
        player_walk::NodeHandle,
        scenery_node::{self, Layer},
    },
};

struct Sync<'a>(&'a mut Live);

fn serror(e: impl std::fmt::Debug) -> chunk_sync::Error {
    chunk_sync::Error::Binding(format!("{e:?}"))
}

impl ground::Context for Sync<'_> {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        // `new Item(id, amount, charge)`.
        let items = self.0.inner.items.clone();
        Ok(crate::definitions::item::Shared::new(item::Item::new(
            id, amount, charge, &items,
        )))
    }
    fn default_interaction(&mut self, _: &Ground) -> ground::Result<()> {
        Err(ground::Error::Binding(
            "not ported in the live world: ground item construction during chunk sync".into(),
        ))
    }
    fn uid(&mut self, player: &NodeHandle) -> ground::Result<i32> {
        match self.0.inner.players.get(&player.node) {
            Some(s) => Ok(s.uid),
            // A departed dropper's details (ground_items::Grounds::accounts).
            None => self
                .0
                .walk
                .ground
                .accounts
                .get(&player.node)
                .map(|(uid, _)| *uid)
                .ok_or(ground::Error::Null),
        }
    }
    fn username(&mut self, player: &NodeHandle) -> ground::Result<Option<String>> {
        Ok(match self.0.inner.players.get(&player.node) {
            Some(s) => Some(s.name.clone()),
            None => self
                .0
                .walk
                .ground
                .accounts
                .get(&player.node)
                .map(|(_, name)| name.clone()),
        })
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.0.inner.ticks)
    }
    fn virtual_private(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        // GroundSpawn.isPrivate() is false.
        Ok(self.0.walk.ground.spawns.get(item).map(|_| false))
    }
    fn virtual_active(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        // GroundSpawn.isActive() is true.
        Ok(self.0.walk.ground.spawns.get(item).map(|_| true))
    }
}

/// The scenery-node layer of an object (by shape): `layer` reaches only the
/// invalid-shape log, which is kept; the variable services refuse.
pub(super) struct Layers<'a>(pub(super) &'a mut Vec<String>);
impl crate::definitions::scenery::PlayerContext for Layers<'_> {
    fn varbit(&mut self, _: i32, _: usize) -> crate::definitions::scenery::Result<Option<i32>> {
        Err(crate::definitions::scenery::Error::Binding(
            "layer-only context".into(),
        ))
    }
    fn varp(&mut self, _: i32, _: usize) -> crate::definitions::scenery::Result<i32> {
        Err(crate::definitions::scenery::Error::Binding(
            "layer-only context".into(),
        ))
    }
}
impl scenery_node::Context for Layers<'_> {
    fn varbit_exists(&mut self, _: i32) -> scenery_node::Result<bool> {
        Err(scenery_node::Error::Null)
    }
    fn set_varbit(&mut self, _: usize, _: i32, _: i32) -> scenery_node::Result<()> {
        Err(scenery_node::Error::Null)
    }
    fn set_varp(&mut self, _: usize, _: i32, _: i32) -> scenery_node::Result<()> {
        Err(scenery_node::Error::Null)
    }
    fn log_invalid_shape(&mut self, shape: i32, constructed: bool) -> scenery_node::Result<()> {
        self.0.push(format!(
            "invalid scenery layer: {shape}, constructed={constructed}"
        ));
        Ok(())
    }
}

impl chunk_sync::Context for Sync<'_> {
    fn last_scene(&mut self, player: &NodeHandle) -> chunk_sync::Result<Location> {
        let p = P(player.clone());
        self.0
            .inner
            .canonical(&p)
            .render
            .read()
            .last_scene
            .clone()
            .ok_or(chunk_sync::Error::Null)
    }
    fn send(&mut self, player: &NodeHandle, buffer: &Buffer) -> chunk_sync::Result<()> {
        let frame = buffer.read().clone();
        self.0.inner.queue_native(&P(player.clone()), frame);
        Ok(())
    }
    fn attribute(&mut self, player: &NodeHandle, name: &str) -> chunk_sync::Result<bool> {
        let p = P(player.clone());
        self.0
            .inner
            .canonical(&p)
            .player
            .attributes()
            .boolean(name, false)
            .map_err(serror)
    }
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> chunk_sync::Result<i32> {
        let live = &mut *self.0;
        live.walk
            .scene
            .context(&mut live.walk.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| chunk_sync::Error::Binding(e.0))
    }
    fn layer(&mut self, object: &Scenery) -> chunk_sync::Result<Option<Layer>> {
        let mut lines = Vec::new();
        let layer = scenery_node::layer(&mut Layers(&mut lines), object).map_err(serror);
        for line in lines {
            self.0.inner.log_line(line);
        }
        layer
    }
    fn foreign_flag(
        &mut self,
        id: usize,
        buffer: &Buffer,
        _: &NodeHandle,
    ) -> chunk_sync::Result<()> {
        // ProjectileUpdateFlag (combat/projectiles.rs) is the only foreign
        // chunk flag the live world makes.
        if super::combat::projectiles::Projectiles::owns(id) {
            return self.0.write_projectile_flag(id, buffer);
        }
        Err(chunk_sync::Error::Binding(
            "not ported in the live world: foreign chunk flags".into(),
        ))
    }
}

/// `ClearRegionChunk` for a chunk that left the player's view.
pub fn chunk_clear(
    live: &mut Live,
    player: &NodeHandle,
    chunk: &HandleChunk,
) -> chunk_sync::Result<()> {
    chunk_sync::clear(&mut Sync(live), chunk, player)
}
/// `RegionChunk.update(player)`: this tick's flags of a chunk still in view.
pub fn chunk_update(
    live: &mut Live,
    player: &NodeHandle,
    chunk: &HandleChunk,
) -> chunk_sync::Result<()> {
    chunk_sync::update(&mut Sync(live), chunk, player)
}
/// `RegionChunk.synchronize(player)`: a chunk that came into view.
pub fn chunk_synchronize(
    live: &mut Live,
    player: &NodeHandle,
    chunk: &HandleChunk,
) -> chunk_sync::Result<()> {
    chunk_sync::synchronize(&mut Sync(live), chunk, player)
}
