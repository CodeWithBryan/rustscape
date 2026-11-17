//! Ground items in the live world: `GroundItemManager` (create, destroy,
//! get, increase, `pulse`), the start-up's `GroundSpawn`s with their respawn
//! pulses on `GameWorld.Pulser`, `PacketProcessor.processGroundItemAction`
//! (`world::ground_runtime` over the canonical legacy scheduler) and
//! `PickupPlugin`/`PickupHandler.take` (`world::pickup`) at arrival.
//!
//! The libraries are unchanged; [`World`] is their owning world over the
//! live state: the canonical actors, the scene's chunks and dirty tracker,
//! the start-up's manager list, spawn registry and option-handler table, the
//! players' canonical inventories and attributes, and the per-node frame
//! queues the live world drains into each session. It is built either from
//! [`Live`] (the driver's ground pulse, drops, respawn pulses) or from the
//! walk host (arrival inside a movement pulse).
//!
//! Answers without a port are named ([`NotPorted`]) and listed in
//! docs/live-world.md: item plugins (`ItemDefinition.getItemPlugin`, none
//! ported: null), ground-item interaction listeners and
//! `PluginInteractionManager` (content: none answers), the drop-trade audit
//! log, guild-item dialogue and `PickUpEvent` dispatch.
use super::{
    host::Live,
    inner::Slot,
    not_ported::NotPorted,
    walk::{Map, Maps, WalkHost},
};
use crate::{
    definitions::{
        animation::CacheSource,
        item::{Handle as Definition, Registry, Shared},
    },
    inventory::{self, container::Container, item},
    player::{ironman, owned::Player},
    pulses,
    world::{
        animation::Animation,
        chunk::HandleChunk,
        chunk_sync::{self, Buffer, Items as ItemList},
        doors::{Handle as Scenery, Location},
        ground_action::{self, Listener},
        ground_item::{self as ground, Handle as Ground},
        ground_manager::{self as manager, Error, Manager, Packet, Result},
        ground_options, ground_runtime,
        ground_spawn::{self as spawn, Registry as Spawns},
        interaction::{Approach, OptionHandle},
        legacy_interaction as legacy, legacy_runtime, pickup, pickup_types,
        player_walk::{self, Node, NodeHandle, State},
        prepared_animation, repository,
        scenery_node::{self, Layer},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::{
    buffer::Writer,
    game::{Frame, updates},
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// `GroundItemManager.GROUND_ITEMS`, the `GroundSpawn` registry and
/// `ItemDefinition.OPTION_HANDLERS` from the start-up, joined to the live
/// world.
pub struct Grounds {
    pub manager: Manager,
    pub spawns: Spawns,
    pub options: ground_options::Registry,
    pub definitions: Arc<Registry>,
    pub types: pickup_types::Types,
    /// `GameWorld.Pulser` (the driver's runner), where respawns are submitted.
    pub runner: pulses::Runner,
    /// The uid and name of every player that logged in, by node: a ground
    /// item keeps its dropper's node after the player left, and the
    /// original reads the dropper's details from it.
    pub accounts: HashMap<Node, (i32, String)>,
}

impl Grounds {
    pub fn new(
        manager: Manager,
        spawns: Spawns,
        options: ground_options::Registry,
        definitions: Arc<Registry>,
        runner: pulses::Runner,
    ) -> Self {
        let types = pickup_types::Types::new(&definitions);
        Self {
            manager,
            spawns,
            options,
            definitions,
            types,
            runner,
            accounts: HashMap::new(),
        }
    }
    /// The native option handler bound to an option's handler id, if it is
    /// `PickupPlugin` (the one natively registered item option handler).
    pub fn program(&self, id: usize) -> Option<pickup::Program> {
        let handler = self.options.handler(id).ok()?;
        let program = handler.read().value::<pickup::Program>().copied();
        program
    }
}

/// The owning world of the ground-item libraries over disjoint parts of the
/// live world.
pub struct World<'a> {
    pub state: &'a mut State,
    pub scene: &'a mut Map,
    pub maps: &'a mut Maps,
    pub ground: &'a mut Grounds,
    pub players: &'a BTreeMap<Node, Slot>,
    pub frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    pub animations: &'a mut prepared_animation::World<CacheSource>,
    pub not_ported: &'a mut NotPorted,
    pub log: &'a mut Vec<String>,
}

fn binding(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn gbinding(e: impl std::fmt::Debug) -> ground::Error {
    ground::Error::Binding(format!("{e:?}"))
}
fn sbinding(e: impl std::fmt::Debug) -> chunk_sync::Error {
    chunk_sync::Error::Binding(format!("{e:?}"))
}
fn inventory_error(e: inventory::Error) -> Error {
    match e {
        inventory::Error::Null => Error::Null,
        inventory::Error::Type => Error::Type,
        inventory::Error::Bounds | inventory::Error::ListBounds => Error::Index,
        e => binding(e),
    }
}

/// `Player.sendMessage` / `PacketDispatch.sendMessage` as GameMessage (70).
pub fn message_frame(text: &str) -> Frame {
    let mut w = Writer::new();
    // A message the writer cannot encode is truncated by the original;
    // nothing here sends one.
    let _ = w.pjstr(text.as_bytes());
    Frame {
        opcode: 70,
        payload: w.into_vec(),
    }
}

impl World<'_> {
    /// Runs `f` over the live world's ground parts, then moves what they
    /// queued into the sessions and logs what they logged.
    pub fn with<T>(live: &mut Live, f: impl FnOnce(&mut World<'_>) -> T) -> T {
        let mut lines = Vec::new();
        let result = {
            let super::inner::Inner {
                state,
                frames,
                players,
                not_ported,
                ..
            } = &mut live.inner;
            let super::walk::Walk {
                scene,
                maps,
                animations,
                ground,
                ..
            } = &mut live.walk;
            let mut world = World {
                state,
                scene,
                maps,
                ground,
                players,
                frames,
                animations,
                not_ported,
                log: &mut lines,
            };
            f(&mut world)
        };
        for line in lines {
            live.inner.log_line(line);
        }
        for node in live.inner.frames.keys().copied().collect::<Vec<_>>() {
            if let Some(p) = live.player_of(node) {
                live.inner.drain(&p);
            }
        }
        result
    }
    /// The same over the walk host's parts (arrival inside a movement pulse).
    pub fn with_walk<T>(
        state: &mut State,
        host: &mut WalkHost<'_>,
        f: impl FnOnce(&mut World<'_>) -> T,
    ) -> T {
        let super::walk::Walk {
            scene,
            maps,
            animations,
            ground,
            ..
        } = &mut *host.walk;
        let mut world = World {
            state,
            scene,
            maps,
            ground,
            players: host.players,
            frames: &mut *host.frames,
            animations,
            not_ported: &mut *host.not_ported,
            log: &mut *host.log,
        };
        f(&mut world)
    }
    fn account(&self, player: &NodeHandle) -> Option<(i32, String)> {
        match self.players.get(&player.node) {
            Some(slot) => Some((slot.uid, slot.name.clone())),
            None => self.ground.accounts.get(&player.node).cloned(),
        }
    }
    fn player(&self, owner: &NodeHandle) -> Result<Player> {
        self.state.player_fields(owner).map_err(binding)
    }
    fn send(&mut self, node: Node, frame: Frame) {
        self.frames.entry(node).or_default().push(frame);
    }
}

impl ground::Context for World<'_> {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        // `new Item(id, amount, charge)`.
        Ok(Shared::new(item::Item::new(
            id,
            amount,
            charge,
            &self.ground.definitions,
        )))
    }
    fn default_interaction(&mut self, item: &Ground) -> ground::Result<()> {
        // `interactPlugin.setDefault()` over ItemDefinition.OPTION_HANDLERS
        // (the start-up's table: `take` is PickupPlugin).
        let options = self.ground.options.clone();
        options
            .initialize(
                self.state,
                &self.ground.spawns,
                &self.ground.definitions,
                item,
                |text| Ok(lowercase_root(text)),
            )
            .map_err(gbinding)
    }
    fn uid(&mut self, player: &NodeHandle) -> ground::Result<i32> {
        self.account(player)
            .map(|(uid, _)| uid)
            .ok_or(ground::Error::Null)
    }
    fn username(&mut self, player: &NodeHandle) -> ground::Result<Option<String>> {
        Ok(self.account(player).map(|(_, name)| name))
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.state.ticks)
    }
    fn virtual_private(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        // GroundSpawn.isPrivate() is false.
        Ok(self.ground.spawns.get(item).map(|_| false))
    }
    fn virtual_active(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        // GroundSpawn.isActive() is true.
        Ok(self.ground.spawns.get(item).map(|_| true))
    }
}

impl manager::Context for World<'_> {
    type Plugin = ();
    fn plugin(&mut self, _: &Definition) -> Result<Option<()>> {
        // ItemDefinition.getItemPlugin: item plugins are content, none is
        // ported or registered.
        self.not_ported
            .default_answer("ItemDefinition.getItemPlugin (no item plugins ported: null)");
        Ok(None)
    }
    fn plugin_remove(&mut self, _: (), _: Option<NodeHandle>, _: &Ground, _: i32) -> Result<()> {
        Err(Error::Binding(
            "not ported in the live world: ItemPlugin.remove (no item plugin is registered)".into(),
        ))
    }
    fn chunk(&mut self, location: Location) -> Result<HandleChunk> {
        self.scene.chunk(&location).map_err(binding)
    }
    fn flagged(&mut self, chunk: &HandleChunk) -> Result<()> {
        // RegionChunk.dirtyListener: ChunkUpdateTracker, reset at the end of
        // the tick by the driver.
        self.scene.dirty_chunks.on_flagged(chunk);
        Ok(())
    }
    fn player_location(&mut self, player: &NodeHandle) -> Result<Option<Location>> {
        self.state.node_location(player.node).map_err(binding)
    }
    fn playing(&mut self, player: &NodeHandle) -> Result<bool> {
        Ok(self.players.get(&player.node).is_some_and(|s| s.playing))
    }
    fn artificial(&mut self, _: &NodeHandle) -> Result<bool> {
        // Bots are not spawned in this world.
        Ok(false)
    }
    fn ai_items(&mut self, _: &NodeHandle) -> Result<Option<ItemList>> {
        Err(Error::Binding(
            "not ported in the live world: AIPlayer ground items (no artificial players)".into(),
        ))
    }
    fn packet(
        &mut self,
        kind: Packet,
        player: Option<NodeHandle>,
        item: &Ground,
        old_amount: i32,
    ) -> Result<()> {
        let player = player.ok_or(Error::Null)?;
        let frames = manager::direct_frames(self, kind, Some(player.clone()), item, old_amount)
            .map_err(|e| Error::Binding(e.to_string()))?;
        for frame in frames {
            self.send(player.node, frame);
        }
        Ok(())
    }
    fn auto_spawn(&mut self, item: &Ground) -> Result<bool> {
        Ok(self.ground.spawns.get(item).is_some() || ground::is_auto_spawn(item))
    }
    fn respawn(&mut self, item: &Ground) -> Result<()> {
        if let Some(spawn) = self.ground.spawns.get(item) {
            let registry = self.ground.spawns.clone();
            registry.respawn(self, &spawn)?;
        } else {
            ground::respawn(item);
        }
        Ok(())
    }
}

impl spawn::Context for World<'_> {
    fn max_players(&mut self) -> Result<i32> {
        // ServerConstants.MAX_PLAYERS.
        Ok(repository::MAX_PLAYERS)
    }
    fn player_count(&mut self) -> Result<i32> {
        // Repository.getPlayers().size().
        Ok(self.players.values().filter(|s| s.active).count() as i32)
    }
    fn pulser(&mut self) -> Result<pulses::Runner> {
        Ok(self.ground.runner.clone())
    }
}

impl chunk_sync::Context for World<'_> {
    fn last_scene(&mut self, player: &NodeHandle) -> chunk_sync::Result<Location> {
        self.players
            .get(&player.node)
            .and_then(|s| s.canonical.render.read().last_scene.clone())
            .ok_or(chunk_sync::Error::Null)
    }
    fn send(&mut self, player: &NodeHandle, buffer: &Buffer) -> chunk_sync::Result<()> {
        let frame = buffer.read().clone();
        World::send(self, player.node, frame);
        Ok(())
    }
    fn attribute(&mut self, player: &NodeHandle, name: &str) -> chunk_sync::Result<bool> {
        self.player(player)
            .map_err(sbinding)?
            .attributes()
            .boolean(name, false)
            .map_err(sbinding)
    }
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> chunk_sync::Result<i32> {
        self.scene
            .context(self.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| chunk_sync::Error::Binding(e.0))
    }
    fn layer(&mut self, object: &Scenery) -> chunk_sync::Result<Option<Layer>> {
        let mut lines = Vec::new();
        let layer =
            scenery_node::layer(&mut super::ground::Layers(&mut lines), object).map_err(sbinding);
        self.log.extend(lines);
        layer
    }
    fn foreign_flag(&mut self, _: usize, _: &Buffer, _: &NodeHandle) -> chunk_sync::Result<()> {
        Err(chunk_sync::Error::Binding(
            "not ported in the live world: foreign chunk flags".into(),
        ))
    }
}

/// The canonical inventory receiver evaluated before each operation.
pub struct Inventory {
    player: Player,
    container: Container,
}

struct Ironman<'a, 'b>(&'a mut World<'b>);
impl ironman::Context for Ironman<'_, '_> {
    fn message(&mut self, p: Option<&NodeHandle>, text: &str) -> ironman::Result<()> {
        let p = p.ok_or(ironman::Error::Null)?;
        self.0.send(p.node, message_frame(text));
        Ok(())
    }
}

impl pickup::Context for World<'_> {
    type Inventory = Inventory;
    fn inventory(&mut self, p: &NodeHandle) -> Result<Inventory> {
        let player = self.player(p)?;
        let container = player.inventory();
        Ok(Inventory { player, container })
    }
    fn has_space(&mut self, i: Inventory, item: &item::Handle) -> Result<bool> {
        i.container
            .has_space_for(Some(item))
            .map_err(inventory_error)
    }
    fn add(&mut self, i: Inventory, item: &item::Handle) -> Result<bool> {
        let mut container = i.container;
        i.player
            .apply(
                self.state,
                self.frames,
                super::now_millis,
                |_, defs, ctx| container.add_two(Some(item), true, defs, ctx),
            )
            .map_err(inventory_error)
    }
    fn drop_deadline(&mut self, p: &NodeHandle, key: &str) -> Result<i32> {
        self.player(p)?
            .attributes()
            .integer(key, 0)
            .map_err(|e| match e {
                crate::player::attributes::Error::Null => Error::Null,
                crate::player::attributes::Error::Type => Error::Type,
                e => binding(e),
            })
    }
    fn is_spawn(&mut self, h: &Ground) -> Result<bool> {
        Ok(self.ground.spawns.get(h).is_some())
    }
    fn ironman_restriction(&mut self, p: &NodeHandle) -> Result<bool> {
        let manager = self.player(p)?.ironman();
        manager
            .check_restriction(&mut Ironman(self))
            .map_err(|e| match e {
                ironman::Error::Null => Error::Null,
                e => binding(e),
            })
    }
    fn god_cape(&mut self, h: &Ground) -> Result<bool> {
        Ok(self.ground.types.cape(&ground::as_item(h)).is_some())
    }
    fn has_god_cape(&mut self, p: &NodeHandle) -> Result<bool> {
        let types = self.ground.types.clone();
        let player = self.player(p)?;
        types.has_cape(|item| player.has_item(Some(item)).map_err(inventory_error))
    }
    fn rune_pouch(&mut self, h: &Ground) -> Result<bool> {
        Ok(self.ground.types.pouch(&ground::as_item(h)).is_some())
    }
    fn has_item(&mut self, p: &NodeHandle, h: &Ground) -> Result<bool> {
        self.player(p)?
            .has_item(Some(&ground::as_item(h)))
            .map_err(inventory_error)
    }
    fn plugin_pickup(&mut self, _: (), _: &NodeHandle, _: &Ground, _: i32) -> Result<bool> {
        Err(Error::Binding(
            "not ported in the live world: ItemPlugin.canPickUp (no item plugin is registered)"
                .into(),
        ))
    }
    fn packet_message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        self.send(p.node, message_frame(text));
        Ok(())
    }
    fn message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        self.send(p.node, message_frame(text));
        Ok(())
    }
    fn messages(&mut self, p: &NodeHandle, text: &[&str]) -> Result<()> {
        for line in text {
            self.send(p.node, message_frame(line));
        }
        Ok(())
    }
    fn guild_dialogue(&mut self, _: &NodeHandle) -> Result<()> {
        Err(Error::Binding(
            "not ported in the live world: the guild items' dialogue (content)".into(),
        ))
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        Ok(self
            .ground
            .definitions
            .for_id(id)
            .read()
            .name
            .clone()
            .unwrap_or_else(|| "null".into()))
    }
    fn player_name(&mut self, p: &NodeHandle) -> Result<Option<String>> {
        Ok(self.account(p).map(|(_, name)| name))
    }
    fn drop_trade_log(&mut self, _: Option<NodeHandle>, _: &str) -> Result<()> {
        self.not_ported
            .skip("PlayerMonitor drop-trade log (player monitor not ported)");
        Ok(())
    }
    fn teleport_permitted(&mut self, l: Option<Location>) -> Result<bool> {
        self.scene
            .context(self.maps)
            .teleport_permitted(l.as_ref())
            .map_err(|e| Error::Binding(e.0))
    }
    fn animate(&mut self, p: &NodeHandle, id: i32) -> Result<()> {
        let animator = self
            .players
            .get(&p.node)
            .map(|s| s.canonical.animator.clone())
            .ok_or(Error::Null)?;
        animator
            .animate(self.animations, Some(Animation::new(id)), None)
            .map_err(binding)?;
        Ok(())
    }
    fn audio(&mut self, p: &NodeHandle, id: i32) -> Result<()> {
        // playAudio(player, id): the sound effect packet.
        self.send(p.node, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn dispatch(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        self.not_ported
            .skip("PickUpEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn pickup_attribute(&mut self, p: &NodeHandle) -> Result<bool> {
        Ok(self.player(p)?.attributes().contains_key("pickup"))
    }
    fn remove_pickup_attribute(&mut self, p: &NodeHandle) -> Result<()> {
        self.player(p)?.attributes().remove("pickup");
        Ok(())
    }
}

// ---------------------------------------------------- the live world's calls

impl Live {
    /// `GroundItemManager.pulse()`, from the driver's update sequence.
    pub fn ground_pulse(&mut self) -> Result<()> {
        World::with(self, |w| {
            let manager = w.ground.manager.clone();
            manager::pulse(w, &manager)
        })
    }
    /// `GroundItemManager.create(item, location, player)`.
    pub fn ground_create(
        &mut self,
        item: item::Handle,
        location: Option<Location>,
        player: Option<NodeHandle>,
    ) -> Result<Ground> {
        World::with(self, |w| {
            let manager = w.ground.manager.clone();
            manager::create_at(w, &manager, Some(item), location, player)
        })
    }
    /// A `GroundSpawn` respawn pulse's body, if this world submitted it.
    pub fn ground_respawn_pulse(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        if !self.walk.ground.spawns.owns_pulse(pulse) {
            return None;
        }
        Some(World::with(self, |w| {
            let registry = w.ground.spawns.clone();
            let manager = w.ground.manager.clone();
            registry.pulse(w, &manager, pulse)
        }))
    }
    /// `PacketProcessor.processGroundItemAction`.
    pub fn ground_action(
        &mut self,
        node: Node,
        action: &rs09_protocol::game::ClientAction,
    ) -> Result<()> {
        self.with_walk_host(|state, host| {
            let player = state.retain_node(node).map_err(binding)?;
            let request = ground_action::Request::from_action(player, action).ok_or(Error::Null)?;
            ground_runtime::process(state, host, &request)
        })
    }
}

/// The start-up's ground items in the scene's chunks: the start-up ran the
/// loader against its own chunk store; each item joins the scene chunk of
/// its location in the manager's order (each chunk's list keeps its order).
pub fn join(live: &mut Live) -> std::result::Result<usize, String> {
    let items = live.walk.ground.manager.get_items().read().snapshot();
    let mut joined = 0;
    for item in items.into_iter().flatten() {
        let Some(location) = item.read().location.clone() else {
            continue;
        };
        let chunk = crate::world::entity_regions::Context::chunk(live, location)
            .map_err(|e| format!("{e:?}"))?;
        chunk_sync::get_items(&chunk).write().push(Some(item));
        joined += 1;
    }
    Ok(joined)
}

// ---------------------------------------------------- the walk host's bindings

fn lerror(e: impl std::fmt::Debug) -> legacy::Error {
    legacy::Error::Binding(Some(format!("{e:?}")))
}

impl WalkHost<'_> {
    fn pickup_program(&self, id: usize) -> legacy::Result<pickup::Program> {
        self.walk.ground.program(id).ok_or_else(|| {
            legacy::Error::Binding(Some(format!(
                "not ported in the live world: option handler {id} (only PickupPlugin is installed)"
            )))
        })
    }
}

impl legacy_runtime::Bindings for WalkHost<'_> {
    fn interaction_locked(&mut self, state: &mut State, p: Option<Node>) -> legacy::Result<bool> {
        let owner = state
            .retain_node(p.ok_or(legacy::Error::Null)?)
            .map_err(lerror)?;
        state
            .locks(&owner)
            .map_err(lerror)?
            .is_kind_locked(
                crate::world::action_locks::Kind::Interaction,
                state,
                &mut crate::world::action_locks::Clock,
            )
            .map_err(lerror)
    }
    fn handler_walk(&mut self, _: &mut State, id: usize) -> legacy::Result<bool> {
        // PickupPlugin is an OptionHandler: isWalk() is true.
        self.pickup_program(id).map(|_| true)
    }
    fn handler_walk_for(
        &mut self,
        _: &mut State,
        id: usize,
        _: Option<Node>,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        self.pickup_program(id).map(|_| false)
    }
    fn handler_delayed(
        &mut self,
        _: &mut State,
        id: usize,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        self.pickup_program(id).map(|_| true)
    }
    fn handler_name(&mut self, _: &mut State, id: usize) -> legacy::Result<String> {
        self.pickup_program(id).map(|_| "PickupPlugin".into())
    }
    fn handler_handle(
        &mut self,
        state: &mut State,
        id: usize,
        p: Option<Node>,
        target: Option<Node>,
        _: &str,
    ) -> legacy::Result<bool> {
        let program = self.pickup_program(id)?;
        let player = state
            .retain_node(p.ok_or(legacy::Error::Null)?)
            .map_err(lerror)?;
        let item = state
            .ground(target.ok_or(legacy::Error::Null)?)
            .map_err(lerror)?;
        World::with_walk(state, self, |w| {
            let manager = w.ground.manager.clone();
            program.handle(w, &manager, Some(&player), Some(&item))
        })
        .map_err(lerror)
    }
    fn lowercase(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn zone(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
        _: Option<&OptionHandle>,
    ) -> legacy::Result<bool> {
        // ZoneMonitor.interact: no installed region zone handles a ground
        // item option (none overrides interact for one).
        self.not_ported.default_answer(
            "ZoneMonitor.interact for ground items (no installed zone handles one: false)",
        );
        Ok(false)
    }
    fn combat_stop(&mut self, _: &mut State, _: Option<Node>) -> legacy::Result<()> {
        self.not_ported
            .skip("CombatPulse.stop before an interaction (combat pulse not installed)");
        Ok(())
    }
    fn face_player(
        &mut self,
        _: &mut State,
        p: Option<Node>,
        l: Option<Location>,
    ) -> legacy::Result<()> {
        let p = p.ok_or(legacy::Error::Null)?;
        let masks = self
            .players
            .get(&p)
            .map(|s| s.canonical.player.masks())
            .ok_or(legacy::Error::Null)?;
        masks.face_location(l).map_err(lerror)?;
        Ok(())
    }
    fn dispatch(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
        _: &str,
    ) -> legacy::Result<()> {
        self.not_ported
            .skip("InteractionEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn exception(&mut self, _: &mut State, e: &legacy::Error) -> legacy::Result<()> {
        self.log.push(format!("interaction exception: {e:?}"));
        Ok(())
    }
    fn log(&mut self, _: &mut State, class: &str, message: &str) -> legacy::Result<()> {
        self.log.push(format!("{class} {message}"));
        Ok(())
    }
}

impl ground_runtime::Bindings for WalkHost<'_> {
    fn ground_lookup(
        &mut self,
        state: &mut State,
        id: i32,
        location: Option<Location>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Ground>> {
        World::with_walk(state, self, |w| manager::get(w, id, location, player))
    }
    fn ground_interaction(
        &mut self,
        state: &mut State,
        item: &Ground,
    ) -> Result<ground_options::Captured> {
        self.walk
            .ground
            .options
            .capture(state, &self.walk.ground.spawns, item)
            .map_err(binding)
    }
    fn ground_plugin(
        &mut self,
        _: &mut State,
        _: &NodeHandle,
        _: &Ground,
        _: &OptionHandle,
    ) -> Result<bool> {
        self.not_ported.default_answer(
            "PluginInteractionManager for ground items (content plugins not ported: false)",
        );
        Ok(false)
    }
    fn ground_listeners(
        &mut self,
        _: &mut State,
        _: i32,
        _: Listener,
        _: Option<String>,
        _: &NodeHandle,
        _: &Ground,
    ) -> Result<bool> {
        self.not_ported.default_answer(
            "InteractionListeners for ground items (content listeners not installed: false)",
        );
        Ok(false)
    }
}

/// `player_walk::Services::interaction_arrival` of the walk host.
pub fn arrival(
    state: &mut State,
    host: &mut WalkHost<'_>,
    approach: &Approach,
) -> player_walk::Result<bool> {
    ground_runtime::arrival(state, host, approach)
}
