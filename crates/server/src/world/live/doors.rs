//! Doors and gates in the live world: DoorManagingPlugin (`world::door_option`,
//! registered for "open", "close", "shut" and "go-through" as the original's
//! `newInstance` does) with `DoorActionHandler` (`world::doors`) over the
//! live scene, through `world::door_runtime`.
//!
//! The libraries are unchanged; [`DoorWorld`] answers their services from the
//! live world: the canonical actors' locations, action locks, walking queues
//! and animators, the players' frame queues (sounds, messages), the scene's
//! replacement, collision and chunk flags (delivered to every viewer by the
//! chunk updates, `world::chunk_sync`), and the "autowalk" logout listener.
//! The scene's restore and autowalk pulses run on the canonical state's
//! runner (the rest of `GameWorld.Pulser`, as the movement pulses), whose
//! external pulses the walk host routes here.
//!
//! Two calls are made right after the library call returns, in their order,
//! because they need what the scene call holds: `walkingQueue.reset()` and
//! `addPath(x, y)` of the autowalk pulse (no read of the queue lies between
//! them and the end of the pulse body), and `finishTask` of the diaries
//! (needs the whole world: [`super::walk::Deferred::DoorDiary`]).
//!
//! Answers without a port are refusals by name, listed in
//! docs/live-world.md.
use super::{
    inner::Slot,
    walk::{Deferred, Map, Maps, WalkHost},
};
use crate::{
    definitions::animation::CacheSource,
    pulses,
    world::{
        action_locks,
        animation::Animation,
        door_option, door_runtime,
        doors::{self, Location},
        pathfinding,
        player_walk::{self, Event, Node, Query, State},
        prepared_animation,
        scene::{self, CacheAssets},
        walk_request::Interfaces,
    },
};
use rs09_protocol::{
    buffer::Writer,
    game::{Frame, updates},
};
use std::collections::BTreeMap;

/// A walking-queue call of a door body, made after the scene call returns.
pub(super) enum PathOp {
    Reset(Node),
    Add(Node, i32, i32),
}

/// `scene::Services` and `door_runtime::Bindings` over the parts of the live
/// world a door reaches (all but the scene, which the library holds).
pub(super) struct DoorWorld<'a> {
    pub state: &'a mut State,
    pub maps: &'a mut Maps,
    pub animations: &'a mut prepared_animation::World<CacheSource>,
    pub players: &'a BTreeMap<Node, Slot>,
    pub frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    pub deferred: &'a mut Vec<Deferred>,
    /// The "autowalk" logout listeners (`Player.addLogoutListener`): the
    /// location each restores at logout.
    pub logouts: &'a mut BTreeMap<Node, Location>,
    pub paths: Vec<PathOp>,
}

fn serror(e: impl std::fmt::Debug) -> scene::Error {
    scene::Error(format!("{e:?}"))
}
fn derror(e: impl std::fmt::Debug) -> doors::Error {
    doors::Error::Binding(format!("{e:?}"))
}

impl DoorWorld<'_> {
    fn send(&mut self, n: Node, frame: Frame) {
        self.frames.entry(n).or_default().push(frame);
    }
    fn slot(&self, n: Node) -> scene::Result<&Slot> {
        self.players
            .get(&n)
            .ok_or_else(|| scene::Error(format!("node {n} is not a live player")))
    }
}

impl scene::Services<CacheAssets> for DoorWorld<'_> {
    fn is_player(&self, n: Node) -> bool {
        self.maps.players.contains(&n)
    }
    fn location(&mut self, _: &mut Map, n: Node) -> scene::Result<Location> {
        self.state
            .node_location(n)
            .map_err(serror)?
            .ok_or_else(|| scene::Error("null entity location".into()))
    }
    fn lock(&mut self, _: &mut Map, n: Node, ticks: i32) -> scene::Result<()> {
        // Entity.lock(ticks): the canonical actor's action locks.
        let owner = self.state.retain_node(n).map_err(serror)?;
        let locks = self.state.locks(&owner).map_err(serror)?;
        locks
            .lock(ticks, self.state, &mut action_locks::Clock)
            .map_err(serror)
    }
    fn audio(&mut self, _: &mut Map, n: Node, id: i32, delay: Option<i32>) -> scene::Result<()> {
        // ContentAPI.playAudio(player, id[, delay]): one loop.
        self.slot(n)?;
        self.send(n, updates::synth_sound(id, 1, delay.unwrap_or(0)));
        Ok(())
    }
    fn message(&mut self, _: &mut Map, n: Node, text: &str) -> scene::Result<()> {
        // Player.sendMessage, with the chat filter of PacketRepository's
        // GameMessage (as super::inner::Inner::message).
        let filtered = self
            .slot(n)?
            .canonical
            .player
            .attributes()
            .get("chat_filter")
            .is_some();
        if filtered && !text.contains("<col=CC6600>") && !text.contains("<col=FFFF00>") {
            return Ok(());
        }
        let mut w = Writer::new();
        w.pjstr(text.as_bytes()).map_err(serror)?;
        self.send(
            n,
            Frame {
                opcode: 70,
                payload: w.into_vec(),
            },
        );
        Ok(())
    }
    fn logout_listener(
        &mut self,
        _: &mut Map,
        n: Node,
        location: Option<Location>,
    ) -> scene::Result<()> {
        // addLogoutListener("autowalk") { it.location = location } and
        // removeLogoutListener("autowalk"): run by Player.clear's
        // logoutListeners (super::host).
        match location {
            Some(l) => {
                self.logouts.insert(n, l);
            }
            None => {
                self.logouts.remove(&n);
            }
        }
        Ok(())
    }
    fn reset_path(&mut self, _: &mut Map, n: Node) -> scene::Result<()> {
        self.paths.push(PathOp::Reset(n));
        Ok(())
    }
    fn add_path(&mut self, _: &mut Map, n: Node, x: i32, y: i32) -> scene::Result<()> {
        self.paths.push(PathOp::Add(n, x, y));
        Ok(())
    }
    fn move_near(&mut self, scene: &mut Map, n: Node, l: Location) -> scene::Result<bool> {
        // Pathfinder.find(entity, location).isMoveNear with the world's
        // Smart finder over the scene's collision.
        self.state
            .context(&mut Routing {
                scene,
                maps: self.maps,
            })
            .find_path(n, l, pathfinding::dispatch::SMART, true)
            .map(|p| p.move_near)
            .map_err(serror)
    }
    fn diary(
        &mut self,
        _: &mut Map,
        n: Node,
        diary: doors::Diary,
        level: i32,
        task: i32,
    ) -> scene::Result<()> {
        // AchievementDiaryManager.finishTask: with the whole world, right
        // after this call (super::host).
        let owner = self.state.retain_node(n).map_err(serror)?;
        self.deferred
            .push(Deferred::DoorDiary(owner, diary, level, task));
        Ok(())
    }
    fn ground_create(&mut self, _: &mut Map, _: Option<usize>) -> scene::Result<()> {
        Err(scene::Error(
            "not ported in the live world: ground items of an expiring scenery (scene service ground_create)".into(),
        ))
    }
    fn pulse(&mut self, _: &mut Map, _: &pulses::Handle) -> scene::Result<bool> {
        Err(scene::Error(
            "not ported in the live world: a scene pulse this scene did not submit".into(),
        ))
    }
    fn stop(&mut self, _: &mut Map, _: &pulses::Handle) -> scene::Result<()> {
        Err(scene::Error(
            "not ported in the live world: stopping a pulse this scene did not submit".into(),
        ))
    }
    fn now_millis(&mut self) -> i64 {
        super::now_millis()
    }
}

impl door_runtime::Bindings<CacheAssets> for DoorWorld<'_> {
    /// `player.getProperties()`: the player node, whose walking queue holds
    /// the teleport location.
    type Properties = Node;
    fn lowercase(&self, s: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(s)
    }
    fn player_location(&mut self, p: Option<Node>) -> doors::Result<Option<Location>> {
        self.state
            .node_location(p.ok_or(doors::Error::Null)?)
            .map_err(derror)
    }
    fn animate(&mut self, p: Option<Node>, id: i32, forced: bool) -> doors::Result<()> {
        let p = p.ok_or(doors::Error::Null)?;
        let animator = self
            .players
            .get(&p)
            .ok_or(doors::Error::Null)?
            .canonical
            .animator
            .clone();
        let animation = Some(Animation::new(id));
        if forced {
            animator
                .force_animation(self.animations, animation)
                .map_err(derror)?;
        } else {
            animator
                .animate(self.animations, animation, None)
                .map_err(derror)?;
        }
        Ok(())
    }
    fn properties(&mut self, p: Option<Node>) -> doors::Result<Option<Node>> {
        let p = p.ok_or(doors::Error::Null)?;
        self.state.actor(p).map_err(derror)?;
        Ok(Some(p))
    }
    fn set_teleport(&mut self, p: &Node, destination: Location) -> doors::Result<()> {
        // Properties.setTeleportLocation: the walking queue moves the player
        // at its next update.
        self.state
            .actor(*p)
            .map_err(derror)?
            .queue
            .state
            .write()
            .entity
            .teleport_location = Some(destination);
        Ok(())
    }
}

/// The search `DoorActionHandler.getDestination` makes: only the scene's
/// collision; anything else it asked of the actors would be refused.
struct Routing<'a> {
    scene: &'a mut Map,
    maps: &'a mut Maps,
}
impl player_walk::Services for Routing<'_> {
    fn query(&mut self, _: &mut State, _: Query<'_>) -> player_walk::Result<bool> {
        Err(player_walk::Error::Unsupported(
            "door search queried actor services",
        ))
    }
    fn event(&mut self, _: &mut State, _: Event<'_>) -> player_walk::Result<()> {
        Err(player_walk::Error::Unsupported(
            "door search emitted an actor event",
        ))
    }
    fn interfaces(&self, _: &State, _: Node) -> player_walk::Result<Interfaces> {
        Err(player_walk::Error::Unsupported(
            "door search queried interfaces",
        ))
    }
    fn region(&mut self, _: &mut State, _: &Location) -> player_walk::Result<Option<bool>> {
        Err(player_walk::Error::Unsupported(
            "door search queried region activity",
        ))
    }
    fn clipping(&mut self, _: &mut State, z: i32, x: i32, y: i32) -> player_walk::Result<i32> {
        self.scene
            .context(self.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn teleport_permitted(
        &mut self,
        _: &mut State,
        l: Option<Location>,
    ) -> player_walk::Result<bool> {
        self.scene
            .context(self.maps)
            .teleport_permitted(l.as_ref())
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn npc_supplier(
        &mut self,
        _: &mut State,
        _: Node,
    ) -> player_walk::Result<Option<pathfinding::dispatch::Supplier>> {
        Ok(Some(pathfinding::dispatch::Supplier::World))
    }
    fn now_millis(&mut self) -> i64 {
        super::now_millis()
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
}

impl WalkHost<'_> {
    /// DoorManagingPlugin's program, when the scenery arena's handler `id`
    /// is it.
    pub fn door_program(&self, id: usize) -> Option<door_option::Program> {
        let handler = self.walk.scenery.handler(id).ok()?;
        handler.read().value::<door_option::Program>().copied()
    }

    /// A door library call with the scene and the world's door services,
    /// then the walking-queue calls it made.
    pub(super) fn with_doors<T>(
        &mut self,
        state: &mut State,
        f: impl FnOnce(&mut door_runtime::Context<'_, CacheAssets, DoorWorld<'_>>) -> doors::Result<T>,
    ) -> doors::Result<T> {
        let (result, paths) = {
            let super::walk::Walk {
                scene,
                maps,
                animations,
                door_logouts,
                ..
            } = &mut *self.walk;
            let mut services = DoorWorld {
                state: &mut *state,
                maps,
                animations,
                players: self.players,
                frames: &mut *self.frames,
                deferred: &mut *self.deferred,
                logouts: door_logouts,
                paths: Vec::new(),
            };
            let result = f(&mut door_runtime::Context {
                scene,
                services: &mut services,
            });
            (result, std::mem::take(&mut services.paths))
        };
        self.apply_paths(state, paths).map_err(derror)?;
        result
    }

    /// A scene pulse body (a scenery restore or a door's autowalk pulse).
    pub(super) fn scene_pulse(
        &mut self,
        state: &mut State,
        pulse: &pulses::Handle,
    ) -> pulses::Result<bool> {
        self.with_doors(state, |ctx| {
            ctx.scene
                .context(&mut *ctx.services)
                .pulse_body(pulse)
                .map_err(|e| doors::Error::Binding(format!("{e:?}")))
        })
        .map_err(|e| pulses::Error::Binding(format!("{e:?}")))
    }

    /// `walkingQueue.reset()` and `addPath(x, y)`, in the order they were
    /// asked for.
    fn apply_paths(&mut self, state: &mut State, paths: Vec<PathOp>) -> player_walk::Result<()> {
        for op in paths {
            let mut world = state.context(self);
            match op {
                PathOp::Reset(n) => {
                    crate::world::movement::Context::walk_reset(&mut world, None, n, None)
                        .map_err(|e| player_walk::Error::Binding(e.to_string()))?
                }
                PathOp::Add(n, x, y) => world.queue_add_path(n, x, y)?,
            }
        }
        Ok(())
    }
}

impl super::host::Live {
    /// `DoorManagingPlugin.newInstance`: its handler for "open", "close",
    /// "shut" and "go-through" in scenery's option handler defaults
    /// (`Option.setHandler`), at its place in the class scan.
    pub fn install_doors(&mut self) -> Result<(), String> {
        self.walk
            .scenery
            .register_doors()
            .map(|_| ())
            .map_err(|e| format!("DoorManagingPlugin: {e:?}"))
    }

    /// The "autowalk" logout listener of a player who logs out in the middle
    /// of a door's walk through: `player.location = location`. `true` when
    /// the player had one.
    pub(super) fn door_logout(&mut self, p: &super::P) -> bool {
        let node = p.node();
        let Some(location) = self.walk.door_logouts.remove(&node) else {
            return false;
        };
        if let Ok(actor) = self.inner.state.actor(node) {
            actor.queue.state.write().entity.location = Some(location.clone());
        }
        if let Some(slot) = self.inner.players.get_mut(&node) {
            slot.location = location;
        }
        true
    }

    /// `player.achievementDiaryManager.finishTask(player, type, level, task)`
    /// from a door's autowalk pulse.
    pub(super) fn door_diary(
        &mut self,
        owner: &crate::world::player_walk::NodeHandle,
        diary: doors::Diary,
        level: i32,
        task: i32,
    ) -> crate::world::player_managers::Result<()> {
        use crate::world::player_managers::diary;
        let p = super::P(owner.clone());
        if !self.inner.players.contains_key(&p.node()) {
            return Ok(());
        }
        let constant = match diary {
            doors::Diary::Falador => "FALADOR",
            doors::Diary::Lumbridge => "LUMBRIDGE",
        };
        let kind = crate::world::player_managers::diary_data::TYPES
            .iter()
            .position(|t| t.constant == constant);
        super::subclasses::with_diaries(self, &p, |h, owner| {
            diary::finish_task(h, owner, owner, kind, level, task)
        })
    }
}
