//! `GroundSpawnLoader` in the native start-up: the verified loader
//! (`world::ground_spawn_loader`) over the world's own ground-item manager,
//! `GroundSpawn` registry, canonical actors, item definitions and chunk dirty
//! listener.
//!
//! What a spawn reaches while it is created, and what answers it here:
//!
//! - `new Item(id, amount)`: `inventory::item::Item::new` over the world's
//!   item definitions (charge 1000);
//! - `GroundItem`'s `interactPlugin.setDefault()`: `world::ground_options`
//!   over the world's option-handler table (`ItemDefinition.OPTION_HANDLERS`),
//!   which holds what the class scan registered natively (`take`, from
//!   `PickupPlugin`) and nothing a content plugin registers;
//! - `GameWorld.getTicks()`: the world's tick counter (0 during start-up);
//! - `RegionManager.getRegionChunk(location)`: the region is entered in the
//!   world's region table (`REGION_CACHE`) and the chunk is created the first
//!   time it is asked for, as `RegionPlane.getRegionChunk` does;
//! - `RegionChunk.flag` -> `RegionChunk.dirtyListener.onFlagged`: the world's
//!   installed listener (`chunk_sync::DirtyTracker`, the
//!   `ChunkUpdateTracker` port), or the original's initial no-op listener when
//!   the `RegionChunk.dirtyListener` step has not run;
//! - the log lines: `[GroundSpawnLoader] Initialized N ground items.`, and an
//!   ERR line per refused row (the original catches each row).
//!
//! Dropped items, players, packets and item plugins are never reached by a
//! spawn without a dropper; those services refuse.
//!
//! Chunks are created one by one here, not as part of a `world::region::Region`
//! (whose constructor builds all 256 chunks of a region): the start-up reaches
//! about a thousand regions. A later join to `world::scene` has to adopt these
//! chunk objects (their item lists and update flags) rather than build new ones.
use super::loaders::Log;
use super::regions::Regions;
use crate::definitions::item::{self as definitions, Shared};
use crate::inventory::item;
use crate::world::{
    chunk::{HandleChunk, RegionChunk},
    chunk_sync::{DirtyTracker, Items as ItemList},
    doors::Location,
    drop_tables::LogLevel,
    ground_item::{self as ground, Handle as Ground},
    ground_manager::{self as manager, Error, Manager, Packet, Result},
    ground_options,
    ground_spawn::Registry as Spawns,
    ground_spawn_loader as loader,
    player_walk::{NodeHandle, State},
};
use rs09_game_core::java_json::Json;
use std::collections::HashMap;

/// The chunks `RegionManager.getRegionChunk` created, by plane and chunk
/// coordinates of their base.
#[derive(Default)]
pub struct Chunks {
    chunks: HashMap<(i32, i32, i32), HandleChunk>,
}
impl Chunks {
    pub fn len(&self) -> usize {
        self.chunks.len()
    }
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
    /// `RegionManager.getRegionChunk(location)`.
    pub fn get(&mut self, regions: &mut Regions, location: &Location) -> HandleChunk {
        let [x, y, z] = location.coordinates();
        regions.for_id(((x >> 6) << 8) | (y >> 6));
        self.chunks
            .entry((x >> 3, y >> 3, z))
            .or_insert_with(|| {
                Shared::new(RegionChunk::new(
                    Location::new((x >> 3) << 3, (y >> 3) << 3, z),
                    0,
                ))
            })
            .clone()
    }
}

/// The world's ground items: `GroundItemManager.GROUND_ITEMS` and the
/// `GroundSpawn` subtype registry.
#[derive(Default)]
pub struct GroundItems {
    pub manager: Manager,
    pub spawns: Spawns,
}

/// One use of the ground-item libraries against the start-up world.
pub struct Context<'a, L: Log> {
    pub actors: &'a mut State,
    pub definitions: &'a definitions::Registry,
    pub options: &'a ground_options::Registry,
    pub spawns: &'a Spawns,
    pub chunks: &'a mut Chunks,
    pub regions: &'a mut Regions,
    pub dirty: &'a mut Option<DirtyTracker>,
    pub ticks: i32,
    /// The JVM's default-locale `String.toLowerCase()` (a host input).
    pub lowercase: fn(&str) -> String,
    pub log: &'a mut L,
}
fn refused(what: &str) -> Error {
    Error::Binding(format!(
        "{what} is not reached by a start-up spawn and is not bound"
    ))
}
fn grefused(what: &str) -> ground::Error {
    ground::Error::Binding(format!(
        "{what} is not reached by a start-up spawn and is not bound"
    ))
}
impl<L: Log> ground::Context for Context<'_, L> {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        Ok(Shared::new(item::Item::new(
            id,
            amount,
            charge,
            self.definitions,
        )))
    }
    fn default_interaction(&mut self, item: &Ground) -> ground::Result<()> {
        let lowercase = self.lowercase;
        self.options
            .initialize(self.actors, self.spawns, self.definitions, item, |text| {
                Ok(lowercase(text))
            })
            .map_err(|e| ground::Error::Binding(format!("{e:?}")))
    }
    fn uid(&mut self, _: &NodeHandle) -> ground::Result<i32> {
        Err(grefused("a dropper's uid"))
    }
    fn username(&mut self, _: &NodeHandle) -> ground::Result<Option<String>> {
        Err(grefused("a dropper's name"))
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.ticks)
    }
    fn virtual_private(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        // GroundSpawn.isPrivate() is false; other items use GroundItem's body.
        Ok(self.spawns.get(item).map(|_| false))
    }
    fn virtual_active(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        Ok(self.spawns.get(item).map(|_| true))
    }
}
impl<L: Log> manager::Context for Context<'_, L> {
    type Plugin = ();
    fn plugin(&mut self, _: &definitions::Handle) -> Result<Option<()>> {
        Err(refused("ItemDefinition.getItemPlugin"))
    }
    fn plugin_remove(&mut self, _: (), _: Option<NodeHandle>, _: &Ground, _: i32) -> Result<()> {
        Err(refused("ItemPlugin.remove"))
    }
    fn chunk(&mut self, location: Location) -> Result<HandleChunk> {
        Ok(self.chunks.get(self.regions, &location))
    }
    fn flagged(&mut self, chunk: &HandleChunk) -> Result<()> {
        // RegionChunk.dirtyListener: the original's initial value is a no-op.
        if let Some(listener) = self.dirty.as_mut() {
            listener.on_flagged(chunk);
        }
        Ok(())
    }
    fn player_location(&mut self, _: &NodeHandle) -> Result<Option<Location>> {
        Err(refused("a player's location"))
    }
    fn playing(&mut self, _: &NodeHandle) -> Result<bool> {
        Err(refused("Player.isPlaying"))
    }
    fn artificial(&mut self, _: &NodeHandle) -> Result<bool> {
        Err(refused("Player.isArtificial"))
    }
    fn ai_items(&mut self, _: &NodeHandle) -> Result<Option<ItemList>> {
        Err(refused("AIPlayer items"))
    }
    fn packet(&mut self, _: Packet, _: Option<NodeHandle>, _: &Ground, _: i32) -> Result<()> {
        Err(refused("a private ground-item packet"))
    }
    fn auto_spawn(&mut self, item: &Ground) -> Result<bool> {
        Ok(self.spawns.get(item).is_some() || ground::is_auto_spawn(item))
    }
    fn respawn(&mut self, _: &Ground) -> Result<()> {
        Err(refused("GroundSpawn.respawn (the world pulse runner)"))
    }
}
impl<L: Log> loader::Context for Context<'_, L> {
    fn entry_error(&mut self, entry: &Json, error: &loader::Error) -> loader::Result<()> {
        // The original prints the row and the stack trace at ERR (standard
        // error); the row is skipped.
        self.log.log(
            "GroundSpawnLoader",
            LogLevel::Err,
            &format!("Error parsing config entry {}: {error}", entry.text()),
        );
        Ok(())
    }
    fn initialized(&mut self, count: i32) -> loader::Result<()> {
        self.log.log(
            "GroundSpawnLoader",
            LogLevel::Fine,
            &format!("Initialized {count} ground items."),
        );
        Ok(())
    }
}
