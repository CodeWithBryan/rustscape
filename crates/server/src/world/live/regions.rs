//! Entity regions and zones in the live world: `RegionManager.move`
//! (`world::entity_regions`), region activation (`world::region_activity`),
//! `ZoneMonitor` with its music zones (`world::zones`) and
//! `MapChunkRenderer` (`world::map_chunk_render`) over the scene's regions
//! and chunks (`world::scene`, the same `RegionManager` the collision uses).
//!
//! [`super::host::Live`] is the zones host (`zones::Host` must be a type that
//! owns its state): a monitor per entity, the regions of the scene with the
//! start-up's region music and music zones, music through the player's
//! `MusicPlayer` ([`super::tabs`]). Entity-region queries replace the earlier
//! 15-tile scan for local players and give the NPC renderer its candidates.
//! Every call into a system without a port is a named answer
//! ([`super::not_ported`]) or a refusal.
use super::{P, host::Live, tabs::Tabs};
use crate::{
    pulses,
    startup::regions::{MusicZone as LoadedZone, Regions as Loaded, ZoneBorders},
    world::{
        chunk::HandleChunk,
        doors::Location,
        entity_regions::{self, Kind as EntityKind},
        landscape::{Chunks, Mapscape},
        map_chunk_render,
        player_walk::{Node, NodeHandle},
        region, region_activity,
        zones::{self, Borders, Monitor, MusicZone},
    },
};
use std::collections::{HashMap, HashSet};

/// The live world's region and zone state.
pub struct Regions {
    /// The start-up's region configurations (region music, music zones).
    pub loaded: Loaded,
    /// Regions whose activity fields were filled from `loaded`.
    configured: HashSet<i32>,
    /// One zone object per loaded MusicZone, shared by every region it covers.
    music: HashMap<usize, MusicZone>,
    /// `entity.getZoneMonitor()`, by node.
    pub monitors: HashMap<Node, Monitor<Live>>,
    /// `playerFlags.lastViewport`, by player node.
    pub viewports: HashMap<Node, map_chunk_render::Viewport>,
}

impl Regions {
    pub fn new(loaded: Loaded) -> Self {
        Self {
            loaded,
            configured: HashSet::new(),
            music: HashMap::new(),
            monitors: HashMap::new(),
            viewports: HashMap::new(),
        }
    }
}

fn borders(b: &ZoneBorders) -> Borders {
    let out = Borders::with_plane(
        b.south_west_x,
        b.south_west_y,
        b.north_east_x,
        b.north_east_y,
        b.plane,
    );
    for exception in b.exceptions.iter().flatten() {
        out.add_exception(borders(exception));
    }
    out
}

fn region_id(l: &Location) -> i32 {
    zones::region_id(l)
}

impl Live {
    /// `RegionManager.forId(id)`: the scene's region, with the region music
    /// and music zones the start-up's music loader gave it.
    pub fn region(&mut self, id: i32) -> region::Handle {
        let handle = self.walk.scene.region(id);
        if self.regions.configured.insert(id)
            && let Some(config) = self.regions.loaded.get(id)
        {
            let music = config.music;
            let zones: Vec<std::sync::Arc<LoadedZone>> = config.music_zones.clone();
            let activity = handle.read().activity.clone();
            activity.write().music = music;
            let list = activity.read().music_zones.clone();
            for zone in zones {
                let key = std::sync::Arc::as_ptr(&zone) as usize;
                let object = self
                    .regions
                    .music
                    .entry(key)
                    .or_insert_with(|| MusicZone::new(zone.music_id, borders(&zone.borders)))
                    .clone();
                list.add(object);
            }
        }
        handle
    }

    /// The zone monitor of an entity, made with it.
    pub fn monitor(&mut self, entity: &NodeHandle) -> Monitor<Live> {
        self.regions
            .monitors
            .entry(entity.node)
            .or_insert_with(|| Monitor::new(entity.clone()))
            .clone()
    }

    pub fn entity_kind(&self, node: Node) -> EntityKind {
        if self.inner.players.contains_key(&node) {
            EntityKind::Player
        } else if self.inner.npcs.rows.contains_key(&node) {
            EntityKind::Npc
        } else {
            EntityKind::Other
        }
    }

    /// `RegionManager.move(entity, from, to)`.
    pub fn region_move(
        &mut self,
        node: &NodeHandle,
        from: Option<Location>,
        to: Location,
    ) -> entity_regions::Result<()> {
        // The scene's collision regions around the destination.
        self.walk
            .activate(&to)
            .map_err(entity_regions::Error::Binding)?;
        entity_regions::move_entity(self, node, from, to)
    }

    /// `Entity.clear`'s chunk and tolerance part.
    pub fn region_clear(&mut self, node: &NodeHandle) -> entity_regions::Result<()> {
        let Some(location) = self.inner.state.node_location(node.node).map_err(binding)? else {
            return Ok(());
        };
        match self.entity_kind(node.node) {
            EntityKind::Player => {
                let chunk = entity_regions::Context::chunk(self, location.clone())?;
                entity_regions::remove_player(self, &chunk, Some(node.clone()))?;
                let region = self.region(region_id(&location));
                let name = self.inner.slot(&P(node.clone())).name.clone();
                let map = region.read().tolerances.clone();
                map.write().remove(&name);
            }
            EntityKind::Npc => {
                let chunk = entity_regions::Context::chunk(self, location)?;
                entity_regions::remove_npc(&chunk, Some(node.clone()));
            }
            EntityKind::Other => {}
        }
        Ok(())
    }

    /// `RegionManager.getLocalPlayers/getLocalNpcs(location, distance)`.
    pub fn local(
        &mut self,
        location: Location,
        distance: i32,
        select: entity_regions::Select,
    ) -> entity_regions::Result<Vec<NodeHandle>> {
        entity_regions::local(self, location, distance, select)
    }
}

fn binding(e: impl std::fmt::Debug) -> entity_regions::Error {
    entity_regions::Error::Binding(format!("{e:?}"))
}
fn zerror(e: impl std::fmt::Debug) -> zones::Error {
    zones::Error::Binding(format!("{e:?}"))
}
fn refused<T>(what: &str) -> zones::Result<T> {
    Err(zones::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

// ---------------------------------------------------- RegionManager.move

impl entity_regions::Context for Live {
    fn chunk(&mut self, location: Location) -> entity_regions::Result<HandleChunk> {
        // RegionManager.getRegionChunk: the region first (with its
        // configuration), then its chunk.
        self.region(region_id(&location));
        self.walk
            .scene
            .chunk(&location)
            .map_err(|e| entity_regions::Error::Binding(e.0))
    }
    fn region(&mut self, location: Location) -> entity_regions::Result<region::Handle> {
        Ok(Live::region(self, region_id(&location)))
    }
    fn kind(&mut self, node: &NodeHandle) -> entity_regions::Result<EntityKind> {
        Ok(self.entity_kind(node.node))
    }
    fn location(&mut self, node: &NodeHandle) -> entity_regions::Result<Option<Location>> {
        self.inner.state.node_location(node.node).map_err(binding)
    }
    fn as_player(&mut self, node: &NodeHandle) -> entity_regions::Result<NodeHandle> {
        if self.inner.players.contains_key(&node.node) {
            Ok(node.clone())
        } else {
            Err(entity_regions::Error::Binding(
                "ClassCastException: not a player".into(),
            ))
        }
    }
    fn name(&mut self, node: &NodeHandle) -> entity_regions::Result<String> {
        self.inner
            .players
            .get(&node.node)
            .map(|s| s.name.clone())
            .ok_or(entity_regions::Error::Null)
    }
    fn flag_active(&mut self, region: region::Handle) -> entity_regions::Result<()> {
        region_activity::flag_active(self, &region).map_err(binding)
    }
    fn check_inactive(&mut self, region: region::Handle) -> entity_regions::Result<()> {
        region_activity::check_inactive(self, &region)
            .map(|_| ())
            .map_err(binding)
    }
    fn footprint(&mut self, node: &NodeHandle) -> entity_regions::Result<Option<Location>> {
        let actor = self.inner.state.actor(node.node).map_err(binding)?;
        Ok(actor.queue.foot_print())
    }
    fn zone_update(
        &mut self,
        node: &NodeHandle,
        location: Option<Location>,
    ) -> entity_regions::Result<()> {
        let monitor = self.monitor(node);
        monitor
            .update_location(self, location.as_ref())
            .map(|_| ())
            .map_err(binding)
    }
    fn unknown_entity(&mut self, node: &NodeHandle) -> entity_regions::Result<()> {
        Err(entity_regions::Error::Binding(format!(
            "RegionManager.move of node {}: neither a player nor an NPC",
            node.node
        )))
    }
    fn now_millis(&mut self) -> entity_regions::Result<i64> {
        Ok(super::now_millis())
    }
}

// ---------------------------------------------------- Region.load, activity

macro_rules! scene {
    ($self:ident) => {
        $self.walk.scene.context(&mut $self.walk.maps)
    };
}

impl region::Context for Live {
    fn archive_id(&mut self, name: &str) -> region::Result<i32> {
        region::Context::archive_id(&mut scene!(self), name)
    }
    fn mapscape_bytes(&mut self, group: i32) -> region::Result<Option<Vec<u8>>> {
        region::Context::mapscape_bytes(&mut scene!(self), group)
    }
    fn xtea(&mut self, region: i32) -> region::Result<[u32; 4]> {
        region::Context::xtea(&mut scene!(self), region)
    }
    fn landscape_bytes(&mut self, group: i32, key: [u32; 4]) -> region::Result<Option<Vec<u8>>> {
        region::Context::landscape_bytes(&mut scene!(self), group, key)
    }
    fn parse_mapscape(
        &mut self,
        region: &region::Handle,
        map: &Mapscape,
        bytes: &[u8],
    ) -> region::Result<()> {
        region::Context::parse_mapscape(&mut scene!(self), region, map, bytes)
    }
    fn parse_landscape(
        &mut self,
        region: &region::Handle,
        chunks: Option<Chunks>,
        map: &Mapscape,
        bytes: &[u8],
        build: bool,
    ) -> region::Result<()> {
        region::Context::parse_landscape(&mut scene!(self), region, chunks, map, bytes, build)
    }
    fn clip_mapscape(
        &mut self,
        region: &region::Handle,
        chunks: Option<Chunks>,
        map: &Mapscape,
    ) -> region::Result<()> {
        region::Context::clip_mapscape(&mut scene!(self), region, chunks, map)
    }
    fn object(
        &mut self,
        location: Location,
        id: i32,
        shape: i32,
    ) -> region::Result<Option<crate::world::doors::Handle>> {
        region::Context::object(&mut scene!(self), location, id, shape)
    }
    fn remove(&mut self, scenery: &crate::world::doors::Handle) -> region::Result<()> {
        region::Context::remove(&mut scene!(self), scenery)
    }
    fn add(&mut self, scenery: &crate::world::doors::Handle) -> region::Result<()> {
        region::Context::add(&mut scene!(self), scenery)
    }
    fn exception(
        &mut self,
        error: &region::Error,
        landscape_region: Option<i32>,
    ) -> region::Result<()> {
        region::Context::exception(&mut scene!(self), error, landscape_region)
    }
}

impl region_activity::Host for Live {
    fn submit_pulse(&mut self, _: &pulses::Handle) -> zones::Result<()> {
        // The region's 50-tick activity pulse unloads an unviewed region
        // (Region.flagInactive, NPC.onRegionInactivity, chunk clears): not
        // installed. A region stays active once a player activated it.
        self.inner
            .not_ported
            .skip("Region activity pulse (region unloading not installed: regions stay active)");
        Ok(())
    }
    fn npc_active(&mut self, npc: &NodeHandle) -> zones::Result<bool> {
        Ok(self
            .inner
            .npcs
            .rows
            .get(&npc.node)
            .is_some_and(|row| row.active))
    }
    fn add_renderable_npc(&mut self, npc: &NodeHandle) -> zones::Result<()> {
        let repository = self.repository.clone();
        repository.add_renderable_npc(self, &P(npc.clone()));
        Ok(())
    }
    fn npc_region_inactivity(&mut self, _: &NodeHandle) -> zones::Result<()> {
        refused("NPC.onRegionInactivity (region unloading not installed)")
    }
    fn log_error(&mut self, message: &str) -> zones::Result<()> {
        self.inner
            .log_line(format!("CommunicationInfo ERR {message}"));
        Ok(())
    }
    fn chunk_region(&mut self, region_id: i32) -> zones::Result<region::Handle> {
        Ok(self.region(region_id))
    }
}

// ---------------------------------------------------- ZoneMonitor

impl zones::Host for Live {
    type Target = NodeHandle;
    type Opaque = ();
    fn kind(&mut self, entity: &NodeHandle) -> zones::Result<zones::EntityKind> {
        Ok(match self.entity_kind(entity.node) {
            EntityKind::Player => zones::EntityKind::Player,
            // No familiar is ever spawned in this world.
            EntityKind::Npc => zones::EntityKind::Npc,
            EntityKind::Other => zones::EntityKind::Other,
        })
    }
    fn monitor(&mut self, entity: &NodeHandle) -> zones::Result<Monitor<Self>> {
        Ok(Live::monitor(self, entity))
    }
    fn region(&mut self, region_id: i32) -> zones::Result<region::Handle> {
        Ok(Live::region(self, region_id))
    }
    fn target_entity(&mut self, target: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        Ok(match self.entity_kind(target.node) {
            EntityKind::Other => None,
            _ => Some(target.clone()),
        })
    }
    fn location(&mut self, entity: &NodeHandle) -> zones::Result<Option<Location>> {
        self.inner.state.node_location(entity.node).map_err(zerror)
    }
    fn is_artificial(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        Ok(false)
    }
    fn entity_update_location(
        &mut self,
        _: &NodeHandle,
        _: Option<&Location>,
    ) -> zones::Result<()> {
        // Entity.updateLocation(last) is empty; Player and NPC do not
        // override it (content NPC classes that do are not ported).
        Ok(())
    }
    fn set_invisible(&mut self, npc: &NodeHandle, invisible: bool) -> zones::Result<()> {
        let ai = self
            .inner
            .npcs
            .ai
            .get_mut(&npc.node)
            .ok_or(zones::Error::Null)?;
        ai.invisible = invisible;
        if let Some(ai) = self.inner.combat.ai.get_mut(&npc.node) {
            ai.invisible = invisible;
        }
        Ok(())
    }
    fn entity_continue_attack(
        &mut self,
        entity: &NodeHandle,
        target: &NodeHandle,
        _: Option<&()>,
        message: bool,
    ) -> zones::Result<bool> {
        // entity.continueAttack(target, style, message) by runtime class;
        // the style is the one of the isAttackable call in progress.
        let style = self.inner.combat.zone_style;
        let (e, t) = (P(entity.clone()), P(target.clone()));
        let result = if self.inner.is_npc_node(entity.node) {
            crate::world::npc_behaviour::combat::npc_continue_attack(self, &e, &t, style, message)
        } else if self.inner.is_player_node(entity.node) {
            crate::world::npc_behaviour::combat::player_continue_attack(
                self, &e, &t, style, message,
            )
        } else {
            Ok(true)
        };
        result.map_err(|e| zones::Error::Binding(format!("{e:?}")))
    }
    fn skull_wilderness(&mut self, player: &NodeHandle) -> zones::Result<bool> {
        // skullManager.isWilderness(), the player's SkullManager.
        crate::world::player_managers::skull::answers::is_wilderness(self, player).map_err(zerror)
    }
    fn skull_level(&mut self, player: &NodeHandle) -> zones::Result<i32> {
        crate::world::player_managers::skull::answers::level(self, player).map_err(zerror)
    }
    fn packet_message(&mut self, player: &NodeHandle, message: &str) -> zones::Result<()> {
        self.inner
            .message(&P(player.clone()), message)
            .map_err(zerror)
    }
    fn player_message(&mut self, player: &NodeHandle, message: &str) -> zones::Result<()> {
        self.inner
            .message(&P(player.clone()), message)
            .map_err(zerror)
    }
    fn has_timer(&mut self, entity: &NodeHandle, identifier: &str) -> zones::Result<bool> {
        // hasTimerActive(entity, identifier) over the player's timers
        // (super::timers); NPC timers are not installed.
        self.has_timer_of(entity.node, identifier).map_err(zerror)
    }
    fn teleport_locked(&mut self, entity: &NodeHandle) -> zones::Result<bool> {
        let locks = self.inner.state.locks(entity).map_err(zerror)?;
        locks
            .is_kind_locked(
                crate::world::action_locks::Kind::Teleport,
                &mut self.inner.state,
                &mut crate::world::action_locks::Clock,
            )
            .map_err(zerror)
    }
    fn target_item_id(&mut self, _: Option<&NodeHandle>) -> zones::Result<i32> {
        refused("teleport jewellery targets (teleports are not installed)")
    }
    fn now_millis(&mut self) -> zones::Result<i64> {
        Ok(super::now_millis())
    }
    fn multi_zone(&mut self, entity: &NodeHandle) -> zones::Result<bool> {
        // properties.isMultiZone().
        crate::world::npc_behaviour::Host::is_multi_zone(self, &P(entity.clone()))
            .map_err(|e| zones::Error::Binding(format!("{e:?}")))
    }
    fn entity_ignore_multi(
        &mut self,
        entity: &NodeHandle,
        victim: &NodeHandle,
    ) -> zones::Result<bool> {
        // Entity.isIgnoreMultiBoundaries(victim).
        crate::world::npc_behaviour::combat::is_ignore_multi_boundaries(
            self,
            &P(entity.clone()),
            &P(victim.clone()),
        )
        .map_err(|e| zones::Error::Binding(format!("{e:?}")))
    }
    fn familiar_owner(&mut self, _: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        refused("familiars")
    }
    fn combat_attacker(&mut self, entity: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        // getAttribute("combat-attacker") (super::combat).
        Ok(match self.inner.attribute(entity.node, "combat-attacker") {
            Some(super::combat::Attr::Entity(p)) => Some(p.0),
            _ => None,
        })
    }
    fn combat_time(&mut self, entity: &NodeHandle) -> zones::Result<Option<i64>> {
        let Some(slot) = self.inner.players.get(&entity.node) else {
            // An NPC's combat-time attribute (super::combat).
            return match self.inner.attribute(entity.node, "combat-time") {
                None => Ok(None),
                Some(super::combat::Attr::Long(v)) => Ok(Some(v)),
                Some(_) => Err(zones::Error::ClassCast),
            };
        };
        match slot.canonical.player.attributes().get("combat-time") {
            None => Ok(None),
            Some(crate::player::attributes::AttributeValue::Long(v)) => Ok(Some(v)),
            Some(_) => Err(zones::Error::ClassCast),
        }
    }
    fn active(&mut self, entity: &NodeHandle) -> zones::Result<bool> {
        Ok(match self.inner.players.get(&entity.node) {
            Some(slot) => slot.active,
            None => self
                .inner
                .npcs
                .rows
                .get(&entity.node)
                .is_some_and(|row| row.active),
        })
    }
    fn id(&mut self, entity: &NodeHandle) -> zones::Result<i32> {
        Ok(self
            .inner
            .npcs
            .rows
            .get(&entity.node)
            .map_or(-1, |row| row.id))
    }
    fn music_playing(&mut self, player: &NodeHandle) -> zones::Result<bool> {
        let p = P(player.clone());
        let playback = &self.inner.canonical(&p).music;
        Ok(playback.current_music_id > 0 && playback.playing)
    }
    fn music_play_default(&mut self, player: &NodeHandle) -> zones::Result<()> {
        Tabs::new(&mut self.inner, P(player.clone()))
            .music(|tabs, music| music.play_default(tabs))
            .map_err(zerror)
    }
    fn music_unlock(
        &mut self,
        player: &NodeHandle,
        id: i32,
        play: Option<bool>,
    ) -> zones::Result<()> {
        Tabs::new(&mut self.inner, P(player.clone()))
            .music(|tabs, music| music.unlock(tabs, id, play.unwrap_or(true)))
            .map_err(zerror)
    }
    fn random_next_int(&mut self, bound: i32) -> zones::Result<i32> {
        self.inner
            .generators
            .shared_next_int(bound)
            .ok_or(zones::Error::IllegalArgument)
    }
    fn fresh_random_next_int(&mut self, _: i32) -> zones::Result<i32> {
        refused("new Random() in zones (no zone that draws one is installed)")
    }
    fn teleport_permitted(&mut self, location: &Location) -> zones::Result<bool> {
        scene!(self)
            .teleport_permitted(Some(location))
            .map_err(|e| zones::Error::Binding(e.0))
    }
}

// ---------------------------------------------------- MapChunkRenderer

impl Live {
    /// `MapChunkRenderer.render(player)`.
    pub fn render_map_chunks_of(&mut self, p: &P) -> map_chunk_render::Result<()> {
        let owner = p.0.clone();
        map_chunk_render::render(self, &owner)
    }
}

fn merror(e: impl std::fmt::Debug) -> map_chunk_render::Error {
    map_chunk_render::Error::Binding(format!("{e:?}"))
}

impl map_chunk_render::Context for Live {
    fn viewport(
        &mut self,
        player: &NodeHandle,
    ) -> map_chunk_render::Result<map_chunk_render::Viewport> {
        Ok(self
            .regions
            .viewports
            .entry(player.node)
            .or_insert_with(map_chunk_render::empty_viewport)
            .clone())
    }
    fn last_scene(&mut self, player: &NodeHandle) -> map_chunk_render::Result<Option<Location>> {
        let p = P(player.clone());
        Ok(self.inner.canonical(&p).render.read().last_scene.clone())
    }
    fn location(&mut self, player: &NodeHandle) -> map_chunk_render::Result<Location> {
        self.inner
            .state
            .node_location(player.node)
            .map_err(merror)?
            .ok_or(map_chunk_render::Error::Null)
    }
    fn chunk(&mut self, location: Location) -> map_chunk_render::Result<HandleChunk> {
        entity_regions::Context::chunk(self, location).map_err(merror)
    }
    fn region(&mut self, location: Location) -> map_chunk_render::Result<region::Handle> {
        Ok(Live::region(self, region_id(&location)))
    }
    fn clear_chunk(
        &mut self,
        player: &NodeHandle,
        chunk: &HandleChunk,
    ) -> map_chunk_render::Result<()> {
        super::ground::chunk_clear(self, player, chunk).map_err(merror)
    }
    fn update_chunk(
        &mut self,
        player: &NodeHandle,
        chunk: &HandleChunk,
    ) -> map_chunk_render::Result<()> {
        super::ground::chunk_update(self, player, chunk).map_err(merror)
    }
    fn synchronize_chunk(
        &mut self,
        player: &NodeHandle,
        chunk: &HandleChunk,
    ) -> map_chunk_render::Result<()> {
        super::ground::chunk_synchronize(self, player, chunk).map_err(merror)
    }
    fn name(&mut self, player: &NodeHandle) -> map_chunk_render::Result<String> {
        self.inner
            .players
            .get(&player.node)
            .map(|s| s.name.clone())
            .ok_or(map_chunk_render::Error::Null)
    }
    fn decrement_view(
        &mut self,
        region: &region::Handle,
        name: String,
    ) -> map_chunk_render::Result<()> {
        region_activity::decrement_view_amount(region, &name);
        Ok(())
    }
    fn increment_view(
        &mut self,
        region: &region::Handle,
        name: String,
    ) -> map_chunk_render::Result<()> {
        region_activity::increment_view_amount(region, &name);
        Ok(())
    }
    fn is_active(&mut self, region: &region::Handle) -> map_chunk_render::Result<bool> {
        Ok(region_activity::is_active(region))
    }
    fn check_inactive(&mut self, region: &region::Handle) -> map_chunk_render::Result<()> {
        region_activity::check_inactive(self, region)
            .map(|_| ())
            .map_err(merror)
    }
    fn flag_active(&mut self, region: &region::Handle) -> map_chunk_render::Result<()> {
        region_activity::flag_active(self, region).map_err(merror)
    }
}
