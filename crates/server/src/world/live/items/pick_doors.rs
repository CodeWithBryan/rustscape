//! Lock-picked doors in the live world (`world::thieving_door`, corpus
//! `fixtures/thieving-doors`): PickableDoorHandler on the "pick-lock" and
//! "open" options of its 28 door definitions.
//!
//! `Picker` is the library's `Host` over the live world, for the player and
//! the scenery of the option: both locations, the scenery's direction, the
//! messages (ContentAPI's split into 86-unit lines for the one line that
//! uses it), `RandomFunction.RANDOM`, the dynamic level, the manual hit,
//! the canonical inventory, the experience and ContentAPI
//! `handleAutowalkDoor`, which is DoorActionHandler's end location and
//! walk-through over the door binding (super::doors).
//!
//! `getDestination` is run in the walk host (live/walk.rs), where the
//! movement pulse asks it. The binding has no JVM corpus.
use super::{super::host::Live, Ctx, Node, P, doors::Opener, everyday::Everyday};
use crate::{
    definitions::option_handler,
    world::{
        combat_pulse::{self as cp, impact},
        combat_swing, doors,
        packet_handlers::Container as Held,
        thieving_door::{self as picks, Error, Host, Result},
    },
};
use rs09_game_core::direction::Direction;

/// `Skills.THIEVING`.
const THIEVING: i32 = 17;

/// The PickableDoorHandler object on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct PickDoorPlugin;
impl option_handler::NativeOptionHandler for PickDoorPlugin {}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `world::thieving_door`'s host over the live world.
struct Picker<'a> {
    live: &'a mut Live,
    p: P,
    node: doors::Handle,
}
impl Picker<'_> {
    fn player(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
}

impl Host for Picker<'_> {
    fn object_location(&mut self) -> Result<[i32; 3]> {
        Ok(self.node.read().location.coordinates())
    }
    fn object_direction(&mut self) -> Result<Direction> {
        Ok(self.node.read().direction)
    }
    fn player_location(&mut self) -> Result<[i32; 3]> {
        let at = self
            .live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?
            .ok_or(Error::Null)?;
        Ok(at.coordinates())
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        let node = self.player();
        Everyday(Ctx(self.live))
            .message(&node, message)
            .map_err(fail)
    }
    fn handle_autowalk_door(&mut self) -> Result<()> {
        // ContentAPI handleAutowalkDoor: DoorActionHandler.getEndLocation,
        // then handleAutowalkDoorWithLocation; the answer is not read.
        let player = self.p.node();
        let mut opener = Opener { live: self.live };
        let end = doors::end_location(&mut opener, player, &self.node).map_err(fail)?;
        doors::autowalk(&mut opener, player, &self.node, end)
            .map(|_| ())
            .map_err(fail)
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        if max <= 0 {
            return Ok(0);
        }
        // `RANDOM.nextInt(bound)` of a positive bound always answers.
        let drawn = self.live.inner.generators.shared_next_int(max);
        drawn.ok_or_else(|| fail("RandomFunction.RANDOM.nextInt"))
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, THIEVING).map_err(fail)
    }
    fn manual_hit(&mut self, hit: i32) -> Result<()> {
        let kind = Some(cp::HitsplatType::Normal);
        impact::manual_hit(self.live, &self.p, Some(&self.p), hit, kind)
            .map(|_| ())
            .map_err(fail)
    }
    fn has_lock_pick(&mut self) -> Result<bool> {
        let node = self.player();
        let inventory = Ctx(self.live)
            .container(&node, Held::Inventory)
            .map_err(fail)?;
        Ok(inventory.amount_id(picks::LOCK_PICK, &self.live.inner.items) >= 1)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        cp::Host::add_experience(self.live, &self.p, THIEVING, xp, true).map_err(fail)
    }
    fn has_action(&mut self, action: &str) -> Result<bool> {
        Ok(has_action(&self.node, action))
    }
    fn door_destination(&mut self) -> Result<[i32; 3]> {
        Err(Error::Binding(
            "PickableDoorHandler.getDestination is run in the walk host".into(),
        ))
    }
}

/// `object.getDefinition().hasAction(action)`.
pub fn has_action(door: &doors::Handle, action: &str) -> bool {
    let definition = door.read().definition.clone();
    definition.is_some_and(|d| d.read().has_action(Some(action)))
}

impl Live {
    /// `PickableDoorHandler.newInstance(null)`, if the class scan names it.
    pub fn install_pick_doors(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if !scanned.contains(&picks::CLASS) {
            return Ok(());
        }
        let plugin = option_handler::Handler::new(picks::CLASS, PickDoorPlugin);
        for id in picks::DOORS {
            let definition = self.walk.scene.definitions.for_id(id);
            let handlers = definition.read().handlers.clone();
            for key in picks::KEYS {
                handlers.write().insert(
                    key.to_owned(),
                    crate::definitions::property::Property::OptionHandler(plugin.clone()),
                );
            }
        }
        self.round.pick_doors = Some(plugin);
        Ok(())
    }

    /// `PickableDoorHandler.handle(player, node, option)`.
    pub fn pick_door_handle(
        &mut self,
        player: &P,
        door: &doors::Handle,
        option: &str,
    ) -> Result<bool> {
        let mut picker = Picker {
            live: self,
            p: player.clone(),
            node: door.clone(),
        };
        let handled = picks::handle(&mut picker, option);
        self.inner.drain(player);
        handled
    }
}
