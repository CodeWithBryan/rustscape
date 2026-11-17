//! Walking and `Player.tick`'s entity part on the canonical actor state.
//!
//! The walk packets reach `world::walk_request` through
//! [`player_walk::World::process`]; the movement pulse it starts runs in the
//! canonical state's runner; `Entity.tick` is
//! [`player_walk::World::entity_before_masks`] (scripts, the walking queue,
//! timers) and the masks are the canonical player's
//! [`crate::world::prepared_masks::PreparedMasks`]. Collision is the cache
//! scene ([`crate::world::scene`]), loaded region by region as players
//! reach them.
//!
//! Answers here that are not a native library are named
//! ([`super::not_ported`]) and listed in docs/live-world.md.
use super::{inner::Slot, not_ported::NotPorted};
use crate::{
    pulses,
    world::{
        action_locks,
        doors::{self, Location},
        interfaces,
        player_walk::{self, Event, Node, NodeHandle, Query, State},
        prepared_masks::FaceTarget,
        scene::{self, CacheAssets, Scene},
        script_runtime,
        walk_request::Interfaces,
    },
};
use rs09_protocol::{
    buffer::Writer,
    game::{Frame, updates},
};
use std::collections::{BTreeMap, BTreeSet};

pub type Map = Scene<CacheAssets>;

/// The cache scene and the regions loaded in it.
pub struct Walk {
    pub scene: Map,
    /// Animation definitions and the animators' tick.
    pub animations:
        crate::world::prepared_animation::World<crate::definitions::animation::CacheSource>,
    pub maps: Maps,
    pub active: BTreeSet<i32>,
    /// Ground items ([`super::ground_items`]): with the scene they live in.
    pub ground: super::ground_items::Grounds,
    /// Scenery's InteractPlugin tables and option handler arena
    /// (`world::scenery_options`), with the global handlers' defaults.
    pub scenery: crate::world::scenery_options::Registry,
    /// A scenery action is being dispatched (its handler ids are the
    /// scenery arena's).
    pub scenery_dispatch: bool,
    /// `InteractionListeners` (the item packets' registry), for the scenery
    /// listener lookup.
    pub interactions: Option<crate::world::interaction_listeners::Registry>,
}

/// `scene::Services` for collision queries. The scenery services (doors,
/// audio, scripts of scenery pulses) are not bound in the live world: they
/// fail by name if reached.
#[derive(Default)]
pub struct Maps {
    pub players: BTreeSet<Node>,
}
macro_rules! unbound {
    ($name:ident($($arg:ident:$ty:ty),* $(,)?) -> $ret:ty) => {
        fn $name(&mut self, $($arg: $ty),*) -> scene::Result<$ret> {
            Err(scene::Error(concat!("not ported in the live world: scene service ", stringify!($name)).into()))
        }
    };
}
impl scene::Services<CacheAssets> for Maps {
    fn is_player(&self, n: usize) -> bool {
        self.players.contains(&n)
    }
    unbound!(location(_scene: &mut Map, _entity: usize) -> Location);
    unbound!(lock(_scene: &mut Map, _entity: usize, _ticks: i32) -> ());
    unbound!(audio(_scene: &mut Map, _entity: usize, _id: i32, _delay: Option<i32>) -> ());
    unbound!(message(_scene: &mut Map, _entity: usize, _text: &str) -> ());
    unbound!(logout_listener(_scene: &mut Map, _entity: usize, _location: Option<Location>) -> ());
    unbound!(reset_path(_scene: &mut Map, _entity: usize) -> ());
    unbound!(add_path(_scene: &mut Map, _entity: usize, _x: i32, _y: i32) -> ());
    unbound!(move_near(_scene: &mut Map, _entity: usize, _location: Location) -> bool);
    unbound!(
        diary(
            _scene: &mut Map,
            _entity: usize,
            _diary: doors::Diary,
            _level: i32,
            _task: i32,
        ) -> ()
    );
    unbound!(ground_create(_scene: &mut Map, _item: Option<usize>) -> ());
    unbound!(pulse(_scene: &mut Map, _pulse: &pulses::Handle) -> bool);
    unbound!(stop(_scene: &mut Map, _pulse: &pulses::Handle) -> ());
    fn now_millis(&mut self) -> i64 {
        super::now_millis()
    }
}

/// The region id of a location (`Location.getRegionId`).
pub fn region_id(l: &Location) -> i32 {
    let [x, y, _] = l.coordinates();
    ((x >> 6) << 8) | (y >> 6)
}

impl Walk {
    /// Loads the region of a location, and the regions around it a viewport
    /// reaches (the original loads regions as the viewport touches them).
    pub fn activate(&mut self, l: &Location) -> Result<(), String> {
        let [x, y, _] = l.coordinates();
        for dx in [-64, 0, 64] {
            for dy in [-64, 0, 64] {
                let (rx, ry) = (x + dx, y + dy);
                if !(0..16384).contains(&rx) || !(0..16384).contains(&ry) {
                    continue;
                }
                let id = ((rx >> 6) << 8) | (ry >> 6);
                if self.active.contains(&id) {
                    continue;
                }
                self.scene
                    .context(&mut self.maps)
                    .load(id, None)
                    .map_err(|e| e.0)?;
                self.active.insert(id);
            }
        }
        Ok(())
    }
}

/// What the walk services ask of the rest of the world after the call
/// returns: these need the interface context, which the state's call holds.
pub enum Deferred {
    CloseInterfaces(Node),
    CloseChatbox(Node),
    RunToggled(Node, bool),
    /// `RegionManager.move(entity, from, to)` from the walking queue.
    RegionMove(NodeHandle, Option<Location>, Location),
    /// `Repository.removeRenderableNPC(npc)` from `onRegionInactivity`.
    RemoveRenderable(NodeHandle),
    /// A global handler's OptionHandler reached by an interaction
    /// (`InteractPlugin.handle`), run with the whole world.
    Handler(super::global::HandlerCall),
    /// `AggressiveHandler.selectTarget()` of an NPC's tick, run with the
    /// whole world right after the tick's kernel returns.
    SelectTarget(Node),
    /// `ImpactHandler.setDisabledTicks(ticks)` from an NPC's tick.
    ImpactDisabled(Node, i32),
    /// `CombatPulse.stop()` from a pulse manager or an NPC's tick.
    CombatStop(Node),
}

/// `player_walk::Services` and `script_runtime::Bindings` over disjoint parts
/// of the live world.
pub struct WalkHost<'a> {
    pub walk: &'a mut Walk,
    pub frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    pub players: &'a BTreeMap<Node, Slot>,
    pub not_ported: &'a mut NotPorted,
    pub deferred: &'a mut Vec<Deferred>,
    pub log: &'a mut Vec<String>,
    /// The start-up's NPC store (rows, AI, behaviours).
    pub npcs: &'a mut crate::startup::npcs::Npcs,
    /// The world's generators (`RandomFunction.RANDOM` is `shared`).
    pub random: &'a mut crate::startup::generators::WorldGenerators,
    /// The NPC renderer's views (facing an NPC), when the caller holds them.
    pub npc_views: Option<&'a std::collections::HashMap<Node, crate::world::npc_render::Handle>>,
    /// The combat state (combat pulses, attributes) the NPC tick queries.
    pub combat: &'a mut super::combat::CombatWorld,
}

impl WalkHost<'_> {
    fn send(&mut self, n: Node, frame: Frame) {
        self.frames.entry(n).or_default().push(frame);
    }
    fn masks(&self, n: Node) -> player_walk::Result<crate::world::prepared_masks::PreparedMasks> {
        self.players
            .get(&n)
            .map(|s| s.canonical.player.masks())
            .ok_or(player_walk::Error::Binding(format!(
                "masks of node {n}: not a live player (NPC masks are not joined)"
            )))
    }
    fn manager(&self, n: Node) -> player_walk::Result<interfaces::Manager> {
        self.players
            .get(&n)
            .map(|s| s.canonical.player.manager())
            .ok_or(player_walk::Error::Null)
    }
}

fn binding(e: impl std::fmt::Display) -> player_walk::Error {
    player_walk::Error::Binding(e.to_string())
}
fn binding_debug(e: impl std::fmt::Debug) -> player_walk::Error {
    player_walk::Error::Binding(format!("{e:?}"))
}

impl player_walk::Services for WalkHost<'_> {
    fn external_stop(
        &mut self,
        _: &mut State,
        pulse: &crate::pulses::Handle,
    ) -> crate::pulses::Result<()> {
        // A combat pulse current in a pulse manager: its base stops now,
        // CombatPulse.stop() runs right after this walk call returns.
        let (class, node) = {
            let p = pulse.read();
            (p.class.clone(), p.node(0).ok().flatten())
        };
        match (class, node) {
            (crate::pulses::Class::Combat, Some(node)) => {
                crate::pulses::stop_base(pulse);
                self.deferred.push(Deferred::CombatStop(node));
                Ok(())
            }
            _ => Err(crate::pulses::Error::Binding(
                "unregistered external stop".into(),
            )),
        }
    }
    fn external_inactive(
        &mut self,
        _: &mut State,
        pulse: &crate::pulses::Handle,
    ) -> crate::pulses::Result<bool> {
        let (class, node) = {
            let p = pulse.read();
            (p.class.clone(), p.node(0).ok().flatten())
        };
        match (class, node) {
            (crate::pulses::Class::Combat, Some(node)) => Ok(!self
                .players
                .get(&node)
                .map(|s| s.active)
                .or_else(|| self.npcs.rows.get(&node).map(|r| r.active))
                .unwrap_or(false)),
            _ => Err(crate::pulses::Error::Binding(
                "unregistered external checks".into(),
            )),
        }
    }
    fn query(&mut self, state: &mut State, query: Query<'_>) -> player_walk::Result<bool> {
        Ok(match query {
            Query::Locked(n) => {
                let owner = state.retain_node(n)?;
                state
                    .locks(&owner)?
                    .is_kind_locked(
                        action_locks::Kind::Movement,
                        state,
                        &mut action_locks::Clock,
                    )
                    .map_err(binding)?
            }
            Query::Timer(n, name) => {
                // timers.getTimer(name) != null over the player's timer
                // manager (super::timers). An NPC's manager holds only the
                // start-up's auto timers (AntiMacro and HolidayRandoms remove
                // themselves, GrandExchangeTimer polls): none is asked for.
                let Some(slot) = self.players.get(&n) else {
                    self.not_ported.default_answer(
                        "NPC timer queries (NPC timers not installed: none registered)",
                    );
                    return Ok(false);
                };
                slot.canonical
                    .player
                    .with_profile(|profile| {
                        crate::timers::content::has_identifier(&profile.timers, name)
                    })
                    .map_err(binding_debug)?
                    .map_err(binding_debug)?
            }
            Query::Marathon(_) => {
                self.not_ported
                    .default_answer("Marathon perk query (perks not ported: false)");
                false
            }
            Query::CloseDialogue(_) => {
                // DialogueInterpreter.close with no dialogue open answers true;
                // dialogue content is not ported, so none is open.
                self.not_ported
                    .default_answer("DialogueInterpreter.close (no dialogue content: true)");
                true
            }
            Query::ZoneMove(_, _, _) => {
                // ZoneMonitor.move: true unless a zone the entity is in
                // refuses; none of the installed zones (ZoneBuilder.startup's
                // six) overrides move.
                self.not_ported.default_answer(
                    "ZoneMonitor.move (no installed region zone overrides move: true)",
                );
                true
            }
            Query::CloseInterface(n) => {
                self.deferred.push(Deferred::CloseInterfaces(n));
                true
            }
        })
    }
    fn event(&mut self, state: &mut State, event: Event<'_>) -> player_walk::Result<()> {
        match event {
            Event::Minimap(n) => self.send(
                n,
                Frame {
                    opcode: 153,
                    payload: vec![],
                },
            ),
            Event::Energy(n, _) => {
                let energy = state.actor(n)?.queue.entity().energy;
                if let Some(slot) = self.players.get(&n) {
                    slot.canonical.player.settings().fields.write().run_energy = energy;
                }
                self.send(n, updates::run_energy(energy));
            }
            Event::RunToggled(n, value) => self.deferred.push(Deferred::RunToggled(n, value)),
            Event::Message(n, text) => {
                let mut w = Writer::new();
                w.pjstr(text.as_bytes()).map_err(binding)?;
                self.send(
                    n,
                    Frame {
                        opcode: 70,
                        payload: w.into_vec(),
                    },
                );
            }
            Event::RegionMove(n, from, Some(l)) => {
                // RegionManager.move needs the whole world (zones, the
                // repository): it runs right after this walking call returns.
                self.walk
                    .activate(&l)
                    .map_err(player_walk::Error::Binding)?;
                let owner = state.retain_node(n)?;
                self.deferred.push(Deferred::RegionMove(owner, from, l));
            }
            Event::RegionMove(_, _, None) => return Err(player_walk::Error::Null),
            Event::Exception(text) => self.log.push(format!("walk exception: {text}")),
            Event::Log(level, text) => self.log.push(format!("walk {level:?}: {text}")),
            Event::Debug(_, text) => self.log.push(format!("walk debug: {text}")),
            Event::Scripts(n, phase) => {
                let owner = state.retain_node(n)?;
                script_runtime::phase(state, self, &owner, phase)
                    .map_err(|e| player_walk::Error::Binding(format!("{e:?}")))?;
            }
            Event::TickEvent(_, _) => self
                .not_ported
                .skip("Entity tick event hooks (entity events not ported)"),
            Event::Skills(n) => {
                // Skills.pulse(): returns at once (restoration is the
                // SkillRestore timer's).
                if let Some(slot) = self.players.get(&n) {
                    slot.canonical
                        .player
                        .with_profile(|profile| profile.skills.pulse())
                        .map_err(binding_debug)?;
                }
            }
            Event::Timers(n) => {
                // A player's timers.processTimers() runs right after this
                // call returns (Live::player_tick, super::timers); NPC
                // timers are not installed.
                if !self.players.contains_key(&n) {
                    self.not_ported
                        .skip("timers.processTimers for NPCs (NPC timers not installed)");
                }
            }
            Event::PulseLength(_, _)
            | Event::CountPulse(_)
            | Event::BotTotal(_)
            | Event::OtherTotal(_) => {}
            Event::Masks(n) => {
                if self.players.contains_key(&n) {
                    self.masks(n)?.prepare().map_err(binding)?;
                }
            }
            Event::FaceLocationNone(n) => {
                if self.players.contains_key(&n) {
                    self.masks(n)?.face_location(None).map_err(binding)?;
                }
            }
            Event::Face(n, None) => {
                if self.players.contains_key(&n) {
                    self.masks(n)?.clear_face_entity().map_err(binding)?;
                }
            }
            Event::Face(n, Some(target)) => {
                let masks = self.masks(n)?;
                let face = match self.players.get(&target) {
                    Some(slot) => {
                        let render = slot.canonical.render.clone();
                        FaceTarget::Player(state.retain_node(target)?, render)
                    }
                    None => {
                        let view = self
                            .npc_views
                            .and_then(|views| views.get(&target))
                            .cloned()
                            .ok_or(player_walk::Error::Binding(
                                "facing an NPC without an NPC render view".into(),
                            ))?;
                        FaceTarget::Npc(state.retain_node(target)?, view)
                    }
                };
                masks.face_entity(Some(face)).map_err(binding)?;
            }
            Event::CloseChatbox(n) => self.deferred.push(Deferred::CloseChatbox(n)),
        }
        Ok(())
    }
    fn interfaces(&self, _: &State, n: Node) -> player_walk::Result<Interfaces> {
        let m = self.manager(n)?;
        let chatbox = interfaces::has_chatbox(&m);
        Ok(Interfaces {
            opened: interfaces::is_opened(&m),
            opened_walkable: Some(interfaces::is_walkable(&m).map_err(binding)?),
            chatbox,
            // A chatbox other than 137 is opened only by dialogue content,
            // which is not ported.
            chatbox_walkable: None,
        })
    }
    fn region(&mut self, _: &mut State, l: &Location) -> player_walk::Result<Option<bool>> {
        let id = region_id(l);
        self.walk.scene.region(id);
        Ok(Some(self.walk.active.contains(&id)))
    }
    fn clipping(&mut self, _: &mut State, z: i32, x: i32, y: i32) -> player_walk::Result<i32> {
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn water_clipping(
        &mut self,
        _: &mut State,
        z: i32,
        x: i32,
        y: i32,
    ) -> player_walk::Result<i32> {
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .water_clipping(Some(&Location::new(x, y, z)))
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn npc_finder(&mut self, _: &mut State, node: Node) -> player_walk::Result<Option<usize>> {
        // behavior.getPathfinderOverride(npc), through npc_behaviour.
        use crate::world::npc_behaviour::{self as nb, Finder};
        let behaviour = nb::behaviour(&mut *self.npcs, &node).map_err(binding_debug)?;
        let finder = behaviour
            .pathfinder_override(&mut *self.npcs, &node)
            .map_err(binding_debug)?;
        Ok(finder.map(|f| match f {
            Finder::Smart => crate::world::movement::SMART,
            Finder::Dumb => crate::world::movement::DUMB,
        }))
    }
    fn npc_supplier(
        &mut self,
        _: &mut State,
        node: Node,
    ) -> player_walk::Result<Option<crate::world::pathfinding::dispatch::Supplier>> {
        // behavior.getClippingSupplier(npc), through npc_behaviour.
        use crate::world::npc_behaviour::{self as nb, ClipSupplier};
        let behaviour = nb::behaviour(&mut *self.npcs, &node).map_err(binding_debug)?;
        let supplier = behaviour
            .clipping_supplier(&mut *self.npcs, &node)
            .map_err(binding_debug)?;
        match supplier {
            None => Ok(None),
            Some(ClipSupplier::Standard) => {
                Ok(Some(crate::world::pathfinding::dispatch::Supplier::World))
            }
            Some(ClipSupplier::Other(_)) => Err(player_walk::Error::Binding(
                "not ported in the live world: a content clipping supplier".into(),
            )),
        }
    }
    fn npc_arrival(&mut self, state: &mut State, node: Node) -> player_walk::Result<bool> {
        // The return-to-spawn pulse's arrival body (npc_walk::arrive).
        crate::world::npc_walk::arrive(state, self, node).map_err(binding_debug)
    }
    fn now_millis(&mut self) -> i64 {
        super::now_millis()
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
    fn interaction_arrival(
        &mut self,
        state: &mut State,
        approach: &crate::world::interaction::Approach,
    ) -> player_walk::Result<bool> {
        // InteractPlugin's movement pulse arrived: the legacy arrival body
        // (the only installed option handler is PickupPlugin).
        super::ground_items::arrival(state, self, approach)
    }
    fn delayed_interaction(
        &mut self,
        state: &mut State,
        task: &crate::world::legacy_interaction::Task,
    ) -> player_walk::Result<bool> {
        crate::world::legacy_runtime::arrival(state, self, task)
            .map_err(|e| player_walk::Error::Binding(e.to_string()))
    }
    fn destination(
        &mut self,
        state: &mut State,
        _: Option<Node>,
        target: Option<Node>,
        service: crate::world::movement::DestinationService,
    ) -> player_walk::Result<Option<Location>> {
        // OptionHandler.getDestination of a scenery option: the global
        // handlers' (LadderManagingPlugin computes the climbing side over the
        // scene, super::destination).
        if let (crate::world::movement::DestinationService::Option(id), Some(target)) =
            (&service, target)
        {
            if let Ok(object) = state.scenery(target) {
                let plugin = self
                    .scenery_handler(*id)
                    .map_err(|e| player_walk::Error::Binding(format!("{e:?}")))?;
                let mut geometry = super::destination::Geometry {
                    walk: &mut *self.walk,
                    failures: Vec::new(),
                };
                let at = crate::world::global_handlers::registry::handler_destination(
                    &mut geometry,
                    plugin,
                    Some(&super::destination::SceneNode(object)),
                )
                .map_err(|e| player_walk::Error::Binding(format!("{e:?}")))?;
                if !geometry.failures.is_empty() {
                    return Err(player_walk::Error::Binding(format!(
                        "getDestination reached {:?}",
                        geometry.failures
                    )));
                }
                return Ok(at.map(|[x, y, z]| Location::new(x, y, z)));
            }
        }
        // OptionHandler.getDestination: PickupPlugin's is null (the default
        // destination); no other handler is installed.
        match service {
            crate::world::movement::DestinationService::Option(id)
                if self.walk.ground.program(id).is_some() =>
            {
                Ok(None)
            }
            _ => Err(player_walk::Error::Binding(
                "not ported in the live world: content destinations".into(),
            )),
        }
    }
    fn teleport_permitted(
        &mut self,
        _: &mut State,
        location: Option<Location>,
    ) -> player_walk::Result<bool> {
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .teleport_permitted(location.as_ref())
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
}

fn script_error(what: &str) -> crate::scripts::Error {
    crate::scripts::Error::Binding(format!("not ported in the live world: {what}"))
}

impl script_runtime::Bindings for WalkHost<'_> {
    fn modal_open(&mut self, _: &mut State, owner: &NodeHandle) -> crate::scripts::Result<bool> {
        let m = self
            .manager(owner.node)
            .map_err(|e| crate::scripts::Error::Binding(format!("{e:?}")))?;
        Ok(interfaces::is_opened(&m))
    }
    fn interface_open(
        &mut self,
        _: &mut State,
        owner: &NodeHandle,
    ) -> crate::scripts::Result<bool> {
        let m = self
            .manager(owner.node)
            .map_err(|e| crate::scripts::Error::Binding(format!("{e:?}")))?;
        Ok(interfaces::is_opened(&m))
    }
    fn chatbox_open(&mut self, _: &mut State, owner: &NodeHandle) -> crate::scripts::Result<bool> {
        let m = self
            .manager(owner.node)
            .map_err(|e| crate::scripts::Error::Binding(format!("{e:?}")))?;
        Ok(interfaces::has_chatbox(&m))
    }
    fn close_interfaces(
        &mut self,
        _: &mut State,
        owner: &NodeHandle,
    ) -> crate::scripts::Result<()> {
        self.deferred.push(Deferred::CloseInterfaces(owner.node));
        Ok(())
    }
    fn reset_animator(&mut self, _: &mut State, owner: &NodeHandle) -> crate::scripts::Result<()> {
        let animator = self
            .players
            .get(&owner.node)
            .map(|s| s.canonical.animator.clone())
            .ok_or_else(|| script_error("an NPC animator"))?;
        animator
            .reset(&mut self.walk.animations)
            .map_err(|e| crate::scripts::Error::Binding(format!("{e:?}")))
    }
    fn node_description(
        &mut self,
        _: &mut State,
        _: Option<NodeHandle>,
    ) -> crate::scripts::Result<String> {
        Err(script_error("node descriptions"))
    }
    fn face_location(
        &mut self,
        _: &mut State,
        _: NodeHandle,
        _: Option<Location>,
    ) -> crate::scripts::Result<Option<Location>> {
        Err(script_error("face locations"))
    }
    fn center_location(
        &mut self,
        _: &mut State,
        _: NodeHandle,
    ) -> crate::scripts::Result<Option<Location>> {
        Err(script_error("center locations"))
    }
    fn line_of_sight(
        &mut self,
        _: &mut State,
        _: &NodeHandle,
        _: &Location,
    ) -> crate::scripts::Result<bool> {
        Err(script_error("line of sight"))
    }
    fn face(&mut self, _: &mut State, _: &NodeHandle, _: Location) -> crate::scripts::Result<()> {
        Err(script_error("facing"))
    }
    fn execution_equals(
        &mut self,
        _: &mut State,
        _: Option<crate::world::interaction_listeners::Function>,
        _: Option<crate::world::interaction_listeners::Function>,
    ) -> crate::scripts::Result<bool> {
        Err(script_error("content scripts"))
    }
    fn invoke(
        &mut self,
        _: &mut State,
        _: &NodeHandle,
        _: Option<crate::world::interaction_listeners::Function>,
        _: crate::scripts::Arguments<NodeHandle>,
    ) -> crate::scripts::Result<bool> {
        Err(script_error("content scripts"))
    }
    fn exception_text(
        &mut self,
        _: &mut State,
        error: &crate::scripts::Error,
    ) -> crate::scripts::Result<String> {
        Ok(format!("{error:?}\n"))
    }
    fn log(
        &mut self,
        _: &mut State,
        owner: &str,
        level: crate::scripts::Level,
        text: String,
    ) -> crate::scripts::Result<()> {
        self.log.push(format!("scripts {owner} {level:?} {text}"));
        Ok(())
    }
    fn lock_entity(&mut self, _: &mut State, _: &NodeHandle, _: i32) -> crate::scripts::Result<()> {
        Err(script_error("entity locks from scripts"))
    }
}

/// Copies the walking queue's entity into the render view and the slot:
/// location, scene flag, teleport, direction and this tick's steps.
pub fn project(state: &State, node: Node, slot: &mut Slot) -> player_walk::Result<()> {
    let queue = &state.actor(node)?.queue;
    let entity = queue.entity();
    let mut render = slot.canonical.render.write();
    render.location = entity.location.clone();
    render.update_scene = render.update_scene || entity.update_scene;
    render.teleporting = entity.teleporting;
    render.direction = entity.direction.map_or(6, |d| d.code());
    render.walk_dir = queue.walk_dir();
    render.run_dir = queue.run_dir();
    if let Some(location) = entity.location {
        slot.location = location;
    }
    Ok(())
}

/// Moves a player's walking entity, render view and slot to a location
/// (login's parsed location; `Entity.location` is the actor's).
pub fn place(
    state: &State,
    node: Node,
    slot: &mut Slot,
    location: &Location,
) -> player_walk::Result<()> {
    let actor = state.actor(node)?;
    actor.queue.state.write().entity.location = Some(location.clone());
    slot.canonical.render.write().location = Some(location.clone());
    slot.location = location.clone();
    Ok(())
}

/// `Player.updateSceneGraph`'s flag on both the render view and the
/// walking entity.
pub fn set_scene_flag(
    state: &State,
    node: Node,
    slot: &Slot,
    value: bool,
) -> player_walk::Result<()> {
    state.actor(node)?.queue.state.write().entity.update_scene = value;
    slot.canonical.render.write().update_scene = value;
    Ok(())
}
