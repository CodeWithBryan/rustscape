//! Doors in the live world: DoorManagingPlugin (`world::door_option`,
//! corpus `fixtures/doors`) on scenery's "open", "close", "shut" and
//! "go-through" options, DoorActionHandler (`world::doors`) and its two
//! anonymous pulses as world pulses of `GameWorld.Pulser`.
//!
//! `Opener` is both libraries' context over the live world: the door
//! configurations of the start-up, the cache scene (`RegionManager.getObject`,
//! `Scenery.transform`, `SceneryBuilder.replace`), the player's location,
//! lock, synth sounds, messages, animator, walking queue (`reset` and
//! `addPath`, which do not ask the lock), `Pathfinder.find(..).isMoveNear()`,
//! the teleport location of a trapdoor and the player's own "autowalk"
//! logout listener, which this world keeps by player and runs at logout
//! (`Live::door_logout_listener`).
//!
//! Not ported, and answered by name (docs/live-world-doors.md): the achievement
//! diary tasks of three doors are a named skip, and a door option the
//! original gives to other content (`world::door_claims`: a content
//! listener or a handler on the scenery's definition that this world does
//! not install) is refused instead of being opened as an ordinary door.
//! The binding has no JVM corpus.
use super::{
    super::{host::Live, walk::WalkHost},
    P,
};
use crate::{
    definitions::option_handler,
    pulses,
    world::{
        action_locks,
        animation::Animation,
        door_claims, door_option,
        doors::{self, DoorPulse, Entity, Error, Handle, Location, Result},
        movement, player_walk,
        scenery::{self as builder, ReplaceMode},
        scenery_node,
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;
use std::collections::BTreeMap;

/// The plugin, the running door pulses and the "autowalk" logout listeners.
#[derive(Default)]
pub struct DoorWorld {
    /// The plugin (`this` of `newInstance`).
    pub plugin: Option<option_handler::Handle>,
    /// The anonymous pulses of `handleAutowalkDoorWithLocation` and
    /// `autowalkFence`, by their pulse object.
    pub pulses: Vec<DoorPulse>,
    /// `player.logoutListeners["autowalk"]`: where the player stood.
    pub logout: BTreeMap<usize, Location>,
    /// Where a player who logged out inside a door pulse was put back: the
    /// pulse still holds the player and reads this location.
    pub left: BTreeMap<usize, Location>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `world::doors` and `world::door_option` over the live world.
pub struct Opener<'a> {
    pub live: &'a mut Live,
}
impl Opener<'_> {
    /// The player of an entity still in the world.
    fn present(&mut self, e: Entity) -> Result<Option<P>> {
        if !self.live.inner.players.contains_key(&e) {
            return Ok(None);
        }
        let handle = self.live.inner.state.retain_node(e).map_err(fail)?;
        Ok(Some(P(handle)))
    }
    fn replace_mode(ticks: Option<i32>, clip: bool) -> ReplaceMode {
        ticks.map_or(
            ReplaceMode::Swap {
                clip,
                permanent: false,
            },
            |ticks| ReplaceMode::Temporary { clip, ticks },
        )
    }
}

impl doors::Context for Opener<'_> {
    fn config(&mut self, id: i32) -> Result<Option<doors::Door>> {
        Ok(self.live.walk.scene.doors.get(id))
    }
    fn object(&mut self, location: Location, shape: i32) -> Result<Option<Handle>> {
        let walk = &mut self.live.walk;
        walk.scene
            .context(&mut walk.maps)
            .object(location, -1, shape)
            .map_err(|e| Error::Binding(e.0))
    }
    fn construct(&mut self, id: i32, l: Location, shape: i32, rotation: i32) -> Result<Handle> {
        let definitions = &mut self.live.walk.scene.definitions;
        scenery_node::construct(definitions, id, l, shape, rotation, false).map_err(fail)
    }
    fn replace(&mut self, a: &Handle, b: Handle, ticks: Option<i32>, clip: bool) -> Result<bool> {
        let walk = &mut self.live.walk;
        let mut world = walk.scene.context(&mut walk.maps);
        let mode = Self::replace_mode(ticks, clip);
        builder::replace(&mut world, Some(a), Some(&b), mode).map_err(fail)
    }
    fn is_player(&mut self, e: Entity) -> bool {
        // No NPC opens a door in this world: every entity of this binding
        // is the player of a scenery option.
        !self.live.inner.npcs.rows.contains_key(&e)
    }
    fn entity_location(&mut self, e: Entity) -> Result<Location> {
        if self.live.inner.players.contains_key(&e) {
            return self
                .live
                .inner
                .state
                .node_location(e)
                .map_err(fail)?
                .ok_or(Error::Null);
        }
        // The logged-out player of a running pulse: where its logout
        // listener put it.
        self.live
            .round
            .doors
            .left
            .get(&e)
            .cloned()
            .ok_or(Error::Null)
    }
    fn lock(&mut self, e: Entity, ticks: i32) -> Result<()> {
        let Some(p) = self.present(e)? else {
            return Ok(());
        };
        let state = &mut self.live.inner.state;
        let locks = state.locks(&p.0).map_err(fail)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(fail)
    }
    fn audio(&mut self, e: Entity, id: i32, delay: Option<i32>) -> Result<()> {
        if let Some(p) = self.present(e)? {
            let frame = updates::synth_sound(id, 1, delay.unwrap_or(0));
            self.live.inner.queue_native(&p, frame);
        }
        Ok(())
    }
    fn message(&mut self, e: Entity, text: &str) -> Result<()> {
        match self.present(e)? {
            Some(p) => self.live.inner.message(&p, text).map_err(fail),
            None => Ok(()),
        }
    }
    fn logout_listener(&mut self, e: Entity, location: Option<Location>) -> Result<()> {
        let listeners = &mut self.live.round.doors.logout;
        match location {
            Some(location) => listeners.insert(e, location),
            None => listeners.remove(&e),
        };
        Ok(())
    }
    fn reset_path(&mut self, e: Entity) -> Result<()> {
        if self.present(e)?.is_none() {
            return Ok(());
        }
        self.live
            .with_walk(|world| movement::Context::walk_reset(world, None, e, None))
            .map_err(fail)
    }
    fn add_path(&mut self, e: Entity, x: i32, y: i32) -> Result<()> {
        if self.present(e)?.is_none() {
            return Ok(());
        }
        self.live
            .with_walk(|world| world.queue_add(e, x, y))
            .map_err(fail)
    }
    fn path_move_near(&mut self, e: Entity, location: Location) -> Result<bool> {
        let path = self
            .live
            .with_walk(|world| world.find_path(e, location, movement::SMART, true))
            .map_err(fail)?;
        Ok(path.move_near)
    }
    fn diary(&mut self, _: Entity, _: doors::Diary, _: i32, _: i32) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("AchievementDiaryManager.finishTask (the diary tasks of doors 2112, 35549, 35551 and 2406)");
        Ok(())
    }
    fn submit(&mut self, pulse: DoorPulse) -> Result<()> {
        // GameWorld.getPulser().submit(new Pulse(1) {...}).
        self.live
            .driver
            .pulser()
            .submit(Some(pulse.pulse.clone()))
            .map_err(fail)?;
        self.live.round.doors.pulses.push(pulse);
        Ok(())
    }
}

impl door_option::Context for Opener<'_> {
    /// The player whose properties were asked.
    type Properties = Entity;
    fn register(&mut self, _: &str, _: usize) -> Result<()> {
        Err(Error::Binding(
            "registered by Live::install_doors over the scenery option registry".into(),
        ))
    }
    fn player_location(&mut self, player: Option<usize>) -> Result<Option<Location>> {
        let player = player.ok_or(Error::Null)?;
        self.live.inner.state.node_location(player).map_err(fail)
    }
    fn lowercase(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn audio(&mut self, player: Option<usize>, id: i32) -> Result<()> {
        doors::Context::audio(self, player.ok_or(Error::Null)?, id, None)
    }
    fn animate(&mut self, player: Option<usize>, id: i32, _: bool) -> Result<()> {
        let p = self
            .present(player.ok_or(Error::Null)?)?
            .ok_or(Error::Null)?;
        self.live
            .entity_visualize(&p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn transform(&mut self, object: &Handle, id: i32) -> Result<Handle> {
        let definitions = &mut self.live.walk.scene.definitions;
        scenery_node::transform(definitions, object, id, None, None, None).map_err(fail)
    }
    fn replace(&mut self, object: &Handle, new: Handle, restore: Option<i32>) -> Result<()> {
        doors::Context::replace(self, object, new, restore, true).map(|_| ())
    }
    fn teleport_permitted(&mut self, destination: &Location) -> Result<bool> {
        let walk = &mut self.live.walk;
        walk.scene
            .context(&mut walk.maps)
            .teleport_permitted(Some(destination))
            .map_err(|e| Error::Binding(e.0))
    }
    fn message(&mut self, player: Option<usize>, text: &str) -> Result<()> {
        doors::Context::message(self, player.ok_or(Error::Null)?, text)
    }
    fn properties(&mut self, player: Option<usize>) -> Result<Option<Entity>> {
        Ok(player)
    }
    fn set_teleport(&mut self, player: &Entity, destination: Location) -> Result<()> {
        // `player.getProperties().setTeleportLocation(destination)`.
        let actor = self.live.inner.state.actor(*player).map_err(fail)?;
        actor.queue.state.write().entity.teleport_location = Some(destination);
        Ok(())
    }
    fn door(&mut self, player: Option<usize>, object: &Handle) -> Result<()> {
        doors::handle(self, player.ok_or(Error::Null)?, object)
    }
    fn door_destination(&mut self, player: Option<usize>, object: &Handle) -> Result<Location> {
        doors::destination(self, player.ok_or(Error::Null)?, object)
    }
}

/// `DoorActionHandler.getDestination` inside a walking call (the movement
/// pulse's destination): the mover's location and the path finder only.
pub struct Side<'a, 'b> {
    pub state: &'a mut player_walk::State,
    pub host: &'a mut WalkHost<'b>,
}
macro_rules! no_side {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(fn $name(&mut self, $($arg: $ty),*) -> Result<$ret> {
            Err(Error::Binding(concat!(
                "DoorActionHandler.getDestination does not reach ",
                stringify!($name)
            ).into()))
        })*
    };
}
impl doors::Context for Side<'_, '_> {
    fn is_player(&mut self, e: Entity) -> bool {
        self.host.players.contains_key(&e)
    }
    fn entity_location(&mut self, e: Entity) -> Result<Location> {
        self.state
            .node_location(e)
            .map_err(fail)?
            .ok_or(Error::Null)
    }
    fn path_move_near(&mut self, e: Entity, location: Location) -> Result<bool> {
        let path = self
            .state
            .context(&mut *self.host)
            .find_path(e, location, movement::SMART, true)
            .map_err(fail)?;
        Ok(path.move_near)
    }
    no_side! {
        config(_id: i32) -> Option<doors::Door>;
        object(_l: Location, _shape: i32) -> Option<Handle>;
        construct(_id: i32, _l: Location, _shape: i32, _rotation: i32) -> Handle;
        replace(_a: &Handle, _b: Handle, _ticks: Option<i32>, _clip: bool) -> bool;
        lock(_e: Entity, _ticks: i32) -> ();
        audio(_e: Entity, _id: i32, _delay: Option<i32>) -> ();
        message(_e: Entity, _text: &str) -> ();
        logout_listener(_e: Entity, _l: Option<Location>) -> ();
        reset_path(_e: Entity) -> ();
        add_path(_e: Entity, _x: i32, _y: i32) -> ();
        diary(_e: Entity, _d: doors::Diary, _level: i32, _task: i32) -> ();
        submit(_p: DoorPulse) -> ();
    }
}

impl Live {
    /// `DoorManagingPlugin.newInstance(null)`, if the class scan names it.
    pub fn install_doors(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if !scanned.contains(&door_option::CLASS) {
            return Ok(());
        }
        let plugin = self
            .walk
            .scenery
            .register_doors()
            .map_err(|e| format!("DoorManagingPlugin: {e:?}"))?;
        self.round.doors.plugin = Some(plugin);
        Ok(())
    }

    /// `DoorManagingPlugin.handle(player, node, option)`.
    pub fn door_handle(&mut self, player: &P, door: &Handle, option: &str) -> Result<bool> {
        let id = door.read().id;
        if let Some(owner) = door_claims::claim(id, option) {
            return Err(Error::Binding(format!(
                "not ported in the live world: the \"{option}\" option of scenery {id} belongs to {owner}"
            )));
        }
        let mut opener = Opener { live: self };
        let handled =
            door_option::handle(&mut opener, Some(player.node()), Some(door), Some(option));
        self.inner.drain(player);
        handled
    }

    /// A door pulse by identity: `Some` with whether the runner drops it.
    pub fn door_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &self.round.doors.pulses;
        let at = pulses.iter().position(|p| p.pulse.same(pulse))?;
        let task = pulses[at].clone();
        let entity = match &*task.action.read() {
            doors::Action::Autowalk { entity, .. } | doors::Action::Fence { entity, .. } => *entity,
        };
        let done = doors::pulse_body(&mut Opener { live: self }, &task);
        if !matches!(done, Ok(false)) {
            self.round.doors.pulses.retain(|p| !p.pulse.same(pulse));
            self.round.doors.left.remove(&entity);
            pulses::stop_base(pulse);
        }
        if self.inner.players.contains_key(&entity)
            && let Ok(handle) = self.inner.state.retain_node(entity)
        {
            self.inner.drain(&P(handle));
        }
        Some(done)
    }

    /// The player's "autowalk" logout listener: `player.setLocation(loc)`.
    pub fn door_logout_listener(&mut self, p: &P) -> std::result::Result<(), String> {
        let node = p.node();
        let Some(location) = self.round.doors.logout.remove(&node) else {
            return Ok(());
        };
        let slot = self
            .inner
            .players
            .get_mut(&node)
            .ok_or("a logout listener of a player not in the world")?;
        super::super::walk::place(&self.inner.state, node, slot, &location)
            .map_err(|e| format!("{e:?}"))?;
        // A pulse of this player still runs: it reads this location.
        let running =
            self.round
                .doors
                .pulses
                .iter()
                .any(|p| match &*p.action.read() {
                    doors::Action::Autowalk { entity, .. }
                    | doors::Action::Fence { entity, .. } => *entity == node,
                });
        if running {
            self.round.doors.left.insert(node, location);
        }
        Ok(())
    }
}
