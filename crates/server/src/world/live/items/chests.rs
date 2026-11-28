//! Thieving chests in the live world (`world::thieving_chest`, corpus
//! `fixtures/thieving-chests`): ThievableChestPlugin on the "open" and
//! "search for traps" options of the six chest definitions, and the
//! anonymous trap pulse as a world pulse of `GameWorld.Pulser`.
//!
//! `Looter` is the library's `Host` over the live world, for the player and
//! the scenery of the option: the world tick, the dynamic level and the
//! lifepoints, the canonical inventory (what does not fit is dropped for
//! the player), the lock, the animator, the face mask, the impact handler
//! (the manual hit and its disabled ticks), `Scenery.transform` and
//! `SceneryBuilder.replace` over the cache scene, and the constants'
//! respawn ticks, which this world keeps once for all players as the enum
//! does.
//!
//! `player.sendMessage(message, ticks)` is a pulse of that delay which then
//! sends the message; it is run here as a second kind of world pulse. The
//! binding has no JVM corpus.
use super::{super::host::Live, Ctx, Node, P, everyday::Everyday};
use crate::{
    definitions::{item::Shared, option_handler},
    pulses,
    world::{
        action_locks,
        animation::Animation,
        combat_pulse::{self as cp, impact},
        combat_swing, doors,
        packet_handlers::Container as Held,
        scenery::{self as builder, ReplaceMode},
        thieving_chest::{self as chests, Error, Host, Respawns, Result, TrapPulse},
    },
};

/// `Skills.THIEVING`.
const THIEVING: i32 = 17;

/// The ThievableChestPlugin object on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct ChestPlugin;
impl option_handler::NativeOptionHandler for ChestPlugin {}

/// What a world pulse of this binding does when it is due.
pub enum Task {
    /// The trap pulse, with the chest it was started on.
    Trap(doors::Handle, TrapPulse),
    /// `PacketDispatch.sendMessage(message, ticks)`.
    Message(String),
}

/// The plugin, the enum's respawn ticks and the running pulses.
#[derive(Default)]
pub struct ChestWorld {
    /// The plugin (`this` of `newInstance`).
    pub plugin: Option<option_handler::Handle>,
    /// Every constant's `currentRespawn`.
    pub respawns: Respawns,
    /// The pulses, by their pulse object, with the player's node.
    pub pulses: Vec<(pulses::Handle, usize, Task)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `world::thieving_chest`'s host over the live world.
struct Looter<'a> {
    live: &'a mut Live,
    p: P,
    node: doors::Handle,
}
impl Looter<'_> {
    fn player(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    /// `GameWorld.getPulser().submit(new Pulse(delay, player) {...})`.
    fn submit_task(&mut self, delay: i32, class: &str, task: Task) -> Result<()> {
        let handle = Shared::new(pulses::Pulse::new(
            delay,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            class.into(),
        ));
        self.live
            .driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(fail)?;
        let owner = self.p.node();
        self.live.round.chests.pulses.push((handle, owner, task));
        Ok(())
    }
}

impl Host for Looter<'_> {
    fn respawns(&mut self) -> &mut Respawns {
        &mut self.live.round.chests.respawns
    }
    fn world_ticks(&mut self) -> Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn message_later(&mut self, message: &str, ticks: i32) -> Result<()> {
        // PacketDispatch.sendMessage(message, ticks): a Pulse(ticks, player)
        // that sends it.
        let class = "core.net.packet.PacketDispatch$1";
        self.submit_task(ticks, class, Task::Message(message.to_owned()))
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&self.p.0).map_err(fail)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(fail)
    }
    fn lifepoints(&mut self) -> Result<i32> {
        combat_swing::Host::lifepoints(self.live, &self.p).map_err(fail)
    }
    fn manual_hit(&mut self, hit: i32) -> Result<()> {
        let kind = Some(cp::HitsplatType::Normal);
        impact::manual_hit(self.live, &self.p, Some(&self.p), hit, kind)
            .map(|_| ())
            .map_err(fail)
    }
    fn face_chest(&mut self) -> Result<()> {
        let at = self.node.read().location.clone();
        let masks = self.live.inner.canonical(&self.p).player.masks();
        masks.face_location(Some(at)).map(|_| ()).map_err(fail)
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, THIEVING).map_err(fail)
    }
    fn animator_animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.animator_animate(id)
    }
    fn free_slots(&mut self) -> Result<i32> {
        let node = self.player();
        let inventory = Ctx(self.live)
            .container(&node, Held::Inventory)
            .map_err(fail)?;
        Ok(inventory.free_slots())
    }
    fn set_disabled_ticks(&mut self, ticks: i32) -> Result<()> {
        impact::set_disabled_ticks(self.live, &self.p, ticks).map_err(fail)
    }
    fn submit(&mut self, pulse: TrapPulse) -> Result<()> {
        let task = Task::Trap(self.node.clone(), pulse);
        self.submit_task(1, chests::PULSE_CLASS, task)
    }
    fn add_or_drop(&mut self, id: i32, amount: i32) -> Result<()> {
        // Container.add(item, player): `add(item, true, -1)`, else
        // `GroundItemManager.create(item, player)`.
        let node = self.player();
        let mut everyday = Everyday(Ctx(self.live));
        if everyday.add_to_inventory(&node, id, amount).map_err(fail)? {
            return Ok(());
        }
        let item = everyday.native(id, amount);
        let at = self
            .live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?;
        self.live
            .ground_create(item, at, Some(self.p.0.clone()))
            .map(|_| ())
            .map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        cp::Host::add_experience(self.live, &self.p, THIEVING, xp, true).map_err(fail)
    }
    fn node_active(&mut self) -> Result<bool> {
        Ok(self.node.read().active)
    }
    fn replace(&mut self, id: i32, ticks: i32) -> Result<()> {
        // `object.transform(id)`: the id at the object's location, with
        // its type and rotation.
        let (location, shape, rotation) = {
            let node = self.node.read();
            (node.location.clone(), node.shape, node.rotation)
        };
        let walk = &mut self.live.walk;
        let empty = crate::world::scenery_node::construct(
            &mut walk.scene.definitions,
            id,
            location,
            shape,
            rotation,
            false,
        )
        .map_err(fail)?;
        let mut world = walk.scene.context(&mut walk.maps);
        let mode = ReplaceMode::Temporary { clip: true, ticks };
        builder::replace(&mut world, Some(&self.node), Some(&empty), mode)
            .map(|_| ())
            .map_err(fail)
    }
}

impl Live {
    /// `ThievableChestPlugin.newInstance(null)`, if the class scan names it.
    pub fn install_chests(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if !scanned.contains(&chests::CLASS) {
            return Ok(());
        }
        let plugin = option_handler::Handler::new(chests::CLASS, ChestPlugin);
        for chest in chests::CHESTS {
            for id in chest.object_ids {
                let definition = self.walk.scene.definitions.for_id(*id);
                let handlers = definition.read().handlers.clone();
                for key in chests::KEYS {
                    handlers.write().insert(
                        key.to_owned(),
                        crate::definitions::property::Property::OptionHandler(plugin.clone()),
                    );
                }
            }
        }
        self.round.chests.plugin = Some(plugin);
        Ok(())
    }

    /// `ThievableChestPlugin.handle(player, node, option)`.
    pub fn chest_handle(
        &mut self,
        player: &P,
        chest: &doors::Handle,
        option: &str,
    ) -> Result<bool> {
        let id = chest.read().id;
        let mut looter = Looter {
            live: self,
            p: player.clone(),
            node: chest.clone(),
        };
        let handled = chests::handle(&mut looter, id, option);
        self.inner.drain(player);
        handled
    }

    /// A pulse of this binding by identity: `Some` with whether the runner
    /// drops it.
    pub fn chest_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.chests.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (handle, owner, task) = pulses.remove(at);
        // `Pulse.hasInactiveNode()`: the player left or is not active.
        if !self.inner.players.contains_key(&owner) || !self.inner.node_active(owner) {
            pulses::stop_base(&handle);
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(fail(e))),
        };
        let result = match task {
            Task::Message(text) => self.inner.message(&p, &text).map(|_| true).map_err(fail),
            Task::Trap(node, mut trap) => {
                let mut looter = Looter {
                    live: self,
                    p: p.clone(),
                    node: node.clone(),
                };
                let done = trap.pulse(&mut looter);
                if matches!(done, Ok(false)) {
                    self.round
                        .chests
                        .pulses
                        .push((handle.clone(), owner, Task::Trap(node, trap)));
                }
                done
            }
        };
        self.inner.drain(&p);
        if !matches!(result, Ok(false)) {
            pulses::stop_base(&handle);
        }
        Some(result)
    }
}
