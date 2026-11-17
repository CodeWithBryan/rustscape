//! Ground items bound to canonical actors, real Scene chunks and one scheduler.
//! Profile/plugin/interaction, viewport/activity, foreign payload and transport
//! services are mandatory host bindings; no empty game-content implementation
//! is supplied by this module.
use super::{
    chunk::HandleChunk,
    chunk_sync::{self as sync, Buffer, Items as ItemList},
    doors::{Handle as Scenery, Location},
    ground_item::{self as ground, Handle as Ground},
    ground_manager::{self as manager, Error, Packet, Result},
    ground_spawn::{self as spawn, Registry as Spawns},
    ground_spawn_loader as loader,
    map_chunk_render::{self as viewport, Viewport},
    player_walk::{NodeHandle, State as Actors},
    region, scene,
    scenery_node::Layer,
};
use crate::{
    definitions::item::{Handle as Definition, Registry, Shared},
    inventory::item,
    pulses,
};
use rs09_game_core::java_json::Json;
use rs09_protocol::game::Frame;
use std::sync::Arc;

pub struct Items {
    pub definitions: Arc<Registry>,
    pub manager: manager::Manager,
    pub spawns: Spawns,
    pub pickup_types: super::pickup_types::Types,
}
impl Items {
    pub fn new(definitions: Registry) -> Self {
        let pickup_types = super::pickup_types::Types::new(&definitions);
        Self {
            definitions: Arc::new(definitions),
            manager: manager::Manager::default(),
            spawns: Spawns::default(),
            pickup_types,
        }
    }
}
/// The owner lends its existing state; this assembly creates no second actor,
/// location, chunk, flag, ground-item list or task clock.
pub struct Parts<'a, A: scene::Assets> {
    pub scene: &'a mut scene::Scene<A>,
    pub actors: &'a mut Actors,
    pub items: &'a Items,
}
impl<'a, A: scene::Assets> Parts<'a, A> {
    pub fn new(
        scene: &'a mut scene::Scene<A>,
        actors: &'a mut Actors,
        items: &'a Items,
    ) -> Result<Self> {
        if !scene.runner.same(&actors.runner()) {
            return Err(Error::Binding(
                "scene and actors must share one world runner".into(),
            ));
        }
        Ok(Self {
            scene,
            actors,
            items,
        })
    }
    pub fn context<'b, S: Services<A>>(&'b mut self, services: &'b mut S) -> World<'b, 'a, A, S> {
        World {
            world: self,
            services,
        }
    }
}
/// Each callback may reenter through world.context(self). Native locks are
/// released first. Hosts retain profile data while a canonical node is retained,
/// including old disconnected droppers and their UID/name/AI services.
pub trait Services<A: scene::Assets>: scene::Services<A> {
    type Plugin;
    fn uid(&mut self, world: &mut Parts<'_, A>, player: &NodeHandle) -> Result<i32>;
    fn username(&mut self, world: &mut Parts<'_, A>, player: &NodeHandle)
    -> Result<Option<String>>;
    fn default_interaction(&mut self, world: &mut Parts<'_, A>, item: &Ground) -> Result<()>;
    fn plugin(
        &mut self,
        world: &mut Parts<'_, A>,
        definition: &Definition,
    ) -> Result<Option<Self::Plugin>>;
    fn plugin_remove(
        &mut self,
        world: &mut Parts<'_, A>,
        plugin: Self::Plugin,
        player: Option<NodeHandle>,
        item: &Ground,
        option: i32,
    ) -> Result<()>;
    fn playing(&mut self, world: &mut Parts<'_, A>, player: &NodeHandle) -> Result<bool>;
    fn artificial(&mut self, world: &mut Parts<'_, A>, player: &NodeHandle) -> Result<bool>;
    fn ai_items(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
    ) -> Result<Option<ItemList>>;
    /// One batch preserves the original composite private IoBuffer's adjacency
    /// while each native frame independently satisfies the bounded writer.
    fn send_frames(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        frames: &[Frame],
    ) -> Result<()>;
    fn attribute(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        name: &str,
    ) -> Result<bool>;
    fn layer(&mut self, world: &mut Parts<'_, A>, object: &Scenery) -> Result<Option<Layer>>;
    fn foreign_flag(
        &mut self,
        world: &mut Parts<'_, A>,
        id: usize,
        buffer: &Buffer,
        player: &NodeHandle,
    ) -> Result<()>;
    fn max_players(&mut self, world: &mut Parts<'_, A>) -> Result<i32>;
    fn player_count(&mut self, world: &mut Parts<'_, A>) -> Result<i32>;
    fn viewport(&mut self, world: &mut Parts<'_, A>, player: &NodeHandle) -> Result<Viewport>;
    fn last_scene(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
    ) -> Result<Option<Location>>;
    fn decrement_view(
        &mut self,
        world: &mut Parts<'_, A>,
        region: &region::Handle,
        name: String,
    ) -> Result<()>;
    fn increment_view(
        &mut self,
        world: &mut Parts<'_, A>,
        region: &region::Handle,
        name: String,
    ) -> Result<()>;
    fn region_active(&mut self, world: &mut Parts<'_, A>, region: &region::Handle) -> Result<bool>;
    fn check_inactive(&mut self, world: &mut Parts<'_, A>, region: &region::Handle) -> Result<()>;
    fn flag_active(&mut self, world: &mut Parts<'_, A>, region: &region::Handle) -> Result<()>;
    fn loader_error(
        &mut self,
        world: &mut Parts<'_, A>,
        entry: &Json,
        error: &loader::Error,
    ) -> loader::Result<()>;
    fn loaded(&mut self, world: &mut Parts<'_, A>, count: i32) -> loader::Result<()>;
    /// Foreign task checks belong to their own retained registry, never to a
    /// guessed canonical actor ID. Ordinary GroundSpawn pulses have no checks.
    fn foreign_node_active(&mut self, _: &mut Parts<'_, A>, _: usize) -> pulses::Result<bool> {
        Err(pulses::Error::Binding(
            "unregistered ground-task check".into(),
        ))
    }
}
pub struct World<'a, 'b, A: scene::Assets, S: Services<A>> {
    pub world: &'a mut Parts<'b, A>,
    pub services: &'a mut S,
}
fn binding(error: impl std::fmt::Display) -> Error {
    Error::Binding(error.to_string())
}
fn gerror(error: Error) -> ground::Error {
    match error {
        Error::Null => ground::Error::Null,
        Error::Ground(e) => e,
        e => ground::Error::Binding(e.to_string()),
    }
}
fn serror(error: Error) -> sync::Error {
    match error {
        Error::Null => sync::Error::Null,
        Error::Ground(e) => sync::Error::Ground(e),
        e => sync::Error::Binding(e.to_string()),
    }
}
fn verror(error: Error) -> viewport::Error {
    match error {
        Error::Null => viewport::Error::Null,
        Error::Index => viewport::Error::Index,
        e => viewport::Error::Binding(e.to_string()),
    }
}
impl<A: scene::Assets, S: Services<A>> World<'_, '_, A, S> {
    fn canonical(&mut self, player: &NodeHandle) -> Result<()> {
        let owner = self
            .world
            .actors
            .retain_node(player.node)
            .map_err(binding)?;
        if owner != *player {
            return Err(binding("player handle belongs to another world"));
        }
        Ok(())
    }
    pub fn render(&mut self, player: &NodeHandle) -> viewport::Result<()> {
        viewport::render(self, player)
    }
    pub fn manager_pulse(&mut self) -> Result<()> {
        let manager = self.world.items.manager.clone();
        manager::pulse(self, &manager)
    }
    pub fn recover_ammunition(
        &mut self,
        player: &NodeHandle,
        id: i32,
        amount: i32,
        location: Location,
    ) -> Result<()> {
        self.canonical(player)?;
        let manager = self.world.items.manager.clone();
        let item = Shared::new(item::Item::new(
            id,
            amount,
            1000,
            &self.world.items.definitions,
        ));
        manager::increase(
            self,
            &manager,
            Some(item),
            Some(location),
            Some(player.clone()),
        )?;
        Ok(())
    }
    pub fn load(&mut self, loader: &loader::Loader, text: &str) -> loader::Result<i32> {
        let registry = self.world.items.spawns.clone();
        let manager = self.world.items.manager.clone();
        loader.load_text(text, self, &registry, &manager)
    }
    pub fn owns_pulse(&self, pulse: &pulses::Handle) -> bool {
        self.world.items.spawns.owns_pulse(pulse)
    }
    pub fn pulse_body(&mut self, pulse: &pulses::Handle) -> pulses::Result<bool> {
        let registry = self.world.items.spawns.clone();
        let manager = self.world.items.manager.clone();
        registry
            .pulse(self, &manager, pulse)
            .map_err(|e| pulses::Error::Binding(e.to_string()))
    }
    pub fn pulse_inactive(&mut self, pulse: &pulses::Handle) -> pulses::Result<bool> {
        if !self.owns_pulse(pulse) {
            return Err(pulses::Error::Binding("foreign ground pulse".into()));
        }
        pulses::has_inactive_base(pulse, &mut (), &mut Checks(self))
    }
    pub fn stop_pulse(&mut self, pulse: &pulses::Handle) -> pulses::Result<()> {
        if !self.owns_pulse(pulse) {
            return Err(pulses::Error::Binding("foreign ground pulse".into()));
        }
        pulses::stop_base(pulse);
        Ok(())
    }
}
impl<A: scene::Assets, S: Services<A>> ground::Context for World<'_, '_, A, S> {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        Ok(Shared::new(item::Item::new(
            id,
            amount,
            charge,
            &self.world.items.definitions,
        )))
    }
    fn default_interaction(&mut self, item: &Ground) -> ground::Result<()> {
        self.services
            .default_interaction(self.world, item)
            .map_err(gerror)
    }
    fn uid(&mut self, player: &NodeHandle) -> ground::Result<i32> {
        self.canonical(player).map_err(gerror)?;
        self.services.uid(self.world, player).map_err(gerror)
    }
    fn username(&mut self, player: &NodeHandle) -> ground::Result<Option<String>> {
        self.canonical(player).map_err(gerror)?;
        self.services.username(self.world, player).map_err(gerror)
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.world.actors.ticks)
    }
    fn virtual_private(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        Ok(self.world.items.spawns.get(item).map(|_| false))
    }
    fn virtual_active(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        Ok(self.world.items.spawns.get(item).map(|_| true))
    }
}
impl<A: scene::Assets, S: Services<A>> manager::Context for World<'_, '_, A, S> {
    type Plugin = S::Plugin;
    fn plugin(&mut self, definition: &Definition) -> Result<Option<Self::Plugin>> {
        self.services.plugin(self.world, definition)
    }
    fn plugin_remove(
        &mut self,
        plugin: Self::Plugin,
        player: Option<NodeHandle>,
        item: &Ground,
        option: i32,
    ) -> Result<()> {
        self.services
            .plugin_remove(self.world, plugin, player, item, option)
    }
    fn chunk(&mut self, location: Location) -> Result<HandleChunk> {
        self.world.scene.chunk(&location).map_err(binding)
    }
    fn flagged(&mut self, chunk: &HandleChunk) -> Result<()> {
        self.world.scene.dirty_chunks.on_flagged(chunk);
        Ok(())
    }
    fn player_location(&mut self, player: &NodeHandle) -> Result<Option<Location>> {
        self.canonical(player)?;
        self.world
            .actors
            .node_location(player.node)
            .map_err(binding)
    }
    fn playing(&mut self, player: &NodeHandle) -> Result<bool> {
        self.canonical(player)?;
        self.services.playing(self.world, player)
    }
    fn artificial(&mut self, player: &NodeHandle) -> Result<bool> {
        self.canonical(player)?;
        self.services.artificial(self.world, player)
    }
    fn ai_items(&mut self, player: &NodeHandle) -> Result<Option<ItemList>> {
        self.canonical(player)?;
        self.services.ai_items(self.world, player)
    }
    fn packet(
        &mut self,
        kind: Packet,
        player: Option<NodeHandle>,
        item: &Ground,
        old_amount: i32,
    ) -> Result<()> {
        let player = player.ok_or(Error::Null)?;
        self.canonical(&player)?;
        let frames = manager::direct_frames(self, kind, Some(player.clone()), item, old_amount)
            .map_err(|e| Error::Binding(e.to_string()))?;
        self.services.send_frames(self.world, &player, &frames)
    }
    fn auto_spawn(&mut self, item: &Ground) -> Result<bool> {
        Ok(self.world.items.spawns.get(item).is_some() || ground::is_auto_spawn(item))
    }
    fn respawn(&mut self, item: &Ground) -> Result<()> {
        if let Some(spawn) = self.world.items.spawns.get(item) {
            let registry = self.world.items.spawns.clone();
            registry.respawn(self, &spawn)?;
        } else {
            ground::respawn(item);
        }
        Ok(())
    }
}
impl<A: scene::Assets, S: Services<A>> spawn::Context for World<'_, '_, A, S> {
    fn max_players(&mut self) -> Result<i32> {
        self.services.max_players(self.world)
    }
    fn player_count(&mut self) -> Result<i32> {
        self.services.player_count(self.world)
    }
    fn pulser(&mut self) -> Result<pulses::Runner> {
        Ok(self.world.scene.runner.clone())
    }
}
impl<A: scene::Assets, S: Services<A>> loader::Context for World<'_, '_, A, S> {
    fn entry_error(&mut self, entry: &Json, error: &loader::Error) -> loader::Result<()> {
        self.services.loader_error(self.world, entry, error)
    }
    fn initialized(&mut self, count: i32) -> loader::Result<()> {
        self.services.loaded(self.world, count)
    }
}
impl<A: scene::Assets, S: Services<A>> sync::Context for World<'_, '_, A, S> {
    fn last_scene(&mut self, player: &NodeHandle) -> sync::Result<Location> {
        self.canonical(player).map_err(serror)?;
        self.services
            .last_scene(self.world, player)
            .map_err(serror)?
            .ok_or(sync::Error::Null)
    }
    fn send(&mut self, player: &NodeHandle, buffer: &Buffer) -> sync::Result<()> {
        self.canonical(player).map_err(serror)?;
        let frame = buffer.read().clone();
        self.services
            .send_frames(self.world, player, &[frame])
            .map_err(serror)
    }
    fn attribute(&mut self, player: &NodeHandle, name: &str) -> sync::Result<bool> {
        self.canonical(player).map_err(serror)?;
        self.services
            .attribute(self.world, player, name)
            .map_err(serror)
    }
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> sync::Result<i32> {
        self.world
            .scene
            .context(self.services)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| sync::Error::Binding(e.to_string()))
    }
    fn layer(&mut self, object: &Scenery) -> sync::Result<Option<Layer>> {
        self.services.layer(self.world, object).map_err(serror)
    }
    fn foreign_flag(
        &mut self,
        id: usize,
        buffer: &Buffer,
        player: &NodeHandle,
    ) -> sync::Result<()> {
        self.services
            .foreign_flag(self.world, id, buffer, player)
            .map_err(serror)
    }
}
impl<A: scene::Assets, S: Services<A>> viewport::Context for World<'_, '_, A, S> {
    fn viewport(&mut self, player: &NodeHandle) -> viewport::Result<Viewport> {
        self.canonical(player).map_err(verror)?;
        self.services.viewport(self.world, player).map_err(verror)
    }
    fn last_scene(&mut self, player: &NodeHandle) -> viewport::Result<Option<Location>> {
        self.canonical(player).map_err(verror)?;
        self.services.last_scene(self.world, player).map_err(verror)
    }
    fn location(&mut self, player: &NodeHandle) -> viewport::Result<Location> {
        self.canonical(player).map_err(verror)?;
        self.world
            .actors
            .node_location(player.node)
            .map_err(|e| viewport::Error::Binding(e.to_string()))?
            .ok_or(viewport::Error::Null)
    }
    fn chunk(&mut self, location: Location) -> viewport::Result<HandleChunk> {
        self.world
            .scene
            .chunk(&location)
            .map_err(|e| viewport::Error::Binding(e.to_string()))
    }
    fn region(&mut self, location: Location) -> viewport::Result<region::Handle> {
        let [x, y, _] = location.coordinates();
        Ok(self.world.scene.region(((x >> 6) << 8) | (y >> 6)))
    }
    fn clear_chunk(&mut self, player: &NodeHandle, chunk: &HandleChunk) -> viewport::Result<()> {
        sync::clear(self, chunk, player).map_err(|e| viewport::Error::Binding(e.to_string()))
    }
    fn update_chunk(&mut self, player: &NodeHandle, chunk: &HandleChunk) -> viewport::Result<()> {
        sync::update(self, chunk, player).map_err(|e| viewport::Error::Binding(e.to_string()))
    }
    fn synchronize_chunk(
        &mut self,
        player: &NodeHandle,
        chunk: &HandleChunk,
    ) -> viewport::Result<()> {
        sync::synchronize(self, chunk, player).map_err(|e| viewport::Error::Binding(e.to_string()))
    }
    fn name(&mut self, player: &NodeHandle) -> viewport::Result<String> {
        ground::Context::username(self, player)
            .map_err(|e| viewport::Error::Binding(e.to_string()))?
            .ok_or(viewport::Error::Null)
    }
    fn decrement_view(&mut self, region: &region::Handle, name: String) -> viewport::Result<()> {
        self.services
            .decrement_view(self.world, region, name)
            .map_err(verror)
    }
    fn increment_view(&mut self, region: &region::Handle, name: String) -> viewport::Result<()> {
        self.services
            .increment_view(self.world, region, name)
            .map_err(verror)
    }
    fn is_active(&mut self, region: &region::Handle) -> viewport::Result<bool> {
        self.services
            .region_active(self.world, region)
            .map_err(verror)
    }
    fn check_inactive(&mut self, region: &region::Handle) -> viewport::Result<()> {
        self.services
            .check_inactive(self.world, region)
            .map_err(verror)
    }
    fn flag_active(&mut self, region: &region::Handle) -> viewport::Result<()> {
        self.services
            .flag_active(self.world, region)
            .map_err(verror)
    }
}
struct Checks<'a, 'b, 'c, A: scene::Assets, S: Services<A>>(&'a mut World<'b, 'c, A, S>);
impl<A: scene::Assets, S: Services<A>> pulses::Context<()> for Checks<'_, '_, '_, A, S> {
    fn node_active(&mut self, node: usize, _: &mut ()) -> pulses::Result<bool> {
        self.0.services.foreign_node_active(self.0.world, node)
    }
    fn pulse(&mut self, _: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn exception(&mut self, _: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn send_message(&mut self, _: usize, _: &str, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn log(&mut self, _: pulses::Level, _: String, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn count_pulse(&mut self, _: &str, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding("checks-only adapter".into()))
    }
}
