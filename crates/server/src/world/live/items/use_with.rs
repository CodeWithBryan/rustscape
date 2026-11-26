//! `processUseWith` (`world::packet_handlers::use_with`) over the live world:
//! item on item, NPC, player, scenery and ground item, with the zone monitor,
//! the native `InteractionListeners` (`run_use`), PluginInteractionManager (no
//! plugins) and `UseWithHandler.HANDLERS` (no handler class is ported: the
//! list is empty, the original's answer with no content registered).
//! An unhandled item-on-item use runs `runUnhandledAction` ("Nothing
//! interesting happens." a tick later, as a world pulse). An unhandled use on
//! an NPC, a player or a scene object runs `UseWithHandler$1`: a
//! `MovementPulse` of the canonical movement toward the node (the scene
//! object's canonical node for scenery) whose arrival sends "Nothing
//! interesting happens." (super::super::talk's approaches). The movement to
//! a ground item, a UseWithHandler's own pulses and morphing scenery
//! (`getChild` through varbits/varps) are refused by name.
use super::{Ctx, Node, err, refused};
use crate::{
    definitions::item::Shared,
    pulses,
    world::{
        interaction_listeners::{self as listeners, Kind},
        packet_handlers::{
            Error, Result,
            use_with::{self, Body, Event, HandlerList, UseHandler},
        },
    },
};

/// A use-with pulse the player's pulse manager runs: a `MovementPulse`
/// toward the target whose arrival runs the body
/// ([`super::super::round::Pending`]).
pub type UsePulse = crate::world::movement::Movement;

impl use_with::Context for Ctx<'_> {
    type Pulse = UsePulse;
    fn object(&mut self, x: i32, y: i32, z: i32, id: i32, shape: i32) -> Result<Option<Node>> {
        // RegionManager.getObject(z, x, y, id): the cache scene's object
        // (the same lookup as processSceneryAction's).
        let walk = &mut self.0.walk;
        walk.scene
            .context(&mut walk.maps)
            .object(crate::world::doors::Location::new(x, y, z), id, shape)
            .map(|found| found.map(Node::Scenery))
            .map_err(|e| err(e.0))
    }
    fn scenery_child(&mut self, scenery: &Node, player: &Node) -> Result<Option<Node>> {
        // Scenery.getChild(player): varbit- and varp-configured objects
        // (morphs) are refused by name, as for scenery options.
        let Node::Scenery(object) = scenery else {
            return Err(Error::Cast);
        };
        let p = self.player_of(player)?;
        let walk = &mut self.0.walk;
        crate::world::scenery_node::child(
            &mut walk.scene.definitions,
            &mut super::super::scenery::NoMorphs,
            object,
            Some(p.node()),
        )
        .map(|child| Some(Node::Scenery(child)))
        .map_err(err)
    }
    fn zone_use_with(&mut self, player: &Node, _: &Node, _: &Node) -> Result<bool> {
        let p = self.player_of(player)?;
        let monitor = self.0.monitor(&p.0);
        if monitor.zones().is_empty() {
            // ZoneMonitor.useWith with no zone: false.
            return Ok(false);
        }
        refused(
            "ZoneMonitor.useWith inside a region zone (an inventory item is not a canonical node)",
        )
    }
    fn listeners_use(
        &mut self,
        used: &Node,
        with: &Node,
        kind: Kind,
        player: &Node,
    ) -> Result<bool> {
        let registry = self.0.item_actions.listeners.clone();
        registry
            .run_use(self, used.clone(), with.clone(), kind, player.clone())
            .map_err(|e| match e {
                listeners::Error::Null => Error::Null,
                e => err(e),
            })
    }
    fn plugin_use(&mut self, _: &Node, _: &Event<Node>) -> Result<bool> {
        self.0.inner.not_ported.default_answer(
            "PluginInteractionManager for use-with (content plugins not ported: false)",
        );
        Ok(false)
    }
    fn handlers(&mut self, _: i32) -> Result<Option<HandlerList>> {
        // UseWithHandler.HANDLERS: filled by content UseWithHandler classes
        // (none ported): no list for any key.
        self.0.inner.not_ported.default_answer(
            "UseWithHandler.HANDLERS (no content use-with handler registered: null)",
        );
        Ok(None)
    }
    fn close_interface(&mut self, player: &Node) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.current = Some(p.clone());
        let manager = self.0.inner.canonical(&p).player.manager();
        crate::world::interfaces::close(&mut self.0.inner, &manager).map_err(err)?;
        self.0.inner.drain(&p);
        Ok(())
    }
    fn unhandled(&mut self, player: &Node) -> Result<()> {
        // PulseManager.runUnhandledAction(player, STANDARD): a Pulse(1)
        // whose body sends "Nothing interesting happens."; here a world
        // pulse (the canonical PulseManager does not take content bodies).
        let p = self.player_of(player)?;
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(vec![Some(p.node())])),
            pulses::Class::Other,
            "core.game.system.task.PulseManager$runUnhandledAction$1".into(),
        ));
        self.0
            .driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(err)?;
        self.0
            .item_actions
            .use_pulses
            .push((handle, player.clone()));
        Ok(())
    }
    fn movement(
        &mut self,
        player: &Node,
        destination: &Node,
        handler: Option<&UseHandler>,
        body: Body<Node>,
    ) -> Result<UsePulse> {
        if handler.is_some() {
            // UseWithHandler.getDestination: no handler class is ported, so
            // HANDLERS never answers a list for this pulse to carry.
            return refused("a UseWithHandler's MovementPulse (no handler class is ported)");
        }
        // `new MovementPulse(player, event.getUsedWith())`: no destination
        // flag, so the node's own getDestination; its arrival runs `body`.
        let Node::Player(p) = player else {
            return Err(Error::Cast);
        };
        if matches!(destination, Node::Item(_) | Node::Ground(_)) {
            return refused("UseWithHandler's MovementPulse to an item");
        }
        self.0
            .approach_movement(
                p.clone(),
                destination,
                None,
                None,
                super::super::round::Pending::UseWith(body),
            )
            .map_err(err)
    }
    fn delayed(&mut self, _: &Event<Node>, _: Body<Node>) -> Result<UsePulse> {
        refused("UseWithHandler's item-on-item Pulse (no content use-with handler is ported)")
    }
    fn run_pulse(&mut self, _: &Node, pulse: UsePulse) -> Result<()> {
        // player.getPulseManager().run(pulse, STANDARD).
        self.0.run_listener_movement(pulse).map_err(err)
    }
    fn dispatch_use(&mut self, _: &Node, _: i32, _: i32) -> Result<()> {
        self.0
            .inner
            .not_ported
            .skip("UseWithEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn use_handle(&mut self, _: &UseHandler, _: &Event<Node>) -> Result<bool> {
        refused("UseWithHandler.handle (no content use-with handler is ported)")
    }
    fn face_location(&mut self, _: &Node, _: Option<[i32; 3]>) -> Result<Option<[i32; 3]>> {
        refused("Node.getFaceLocation in use-with")
    }
    fn face(&mut self, _: &Node, _: Option<[i32; 3]>) -> Result<()> {
        refused("faceLocation in use-with")
    }
}

impl super::super::host::Live {
    /// `processUseWith`.
    pub fn use_with(&mut self, player: &super::super::P, request: use_with::UseWith) -> Result<()> {
        let node = Node::Player(player.0.clone());
        use_with::process(&mut Ctx(self), &node, request)
    }
    /// `processSlotSwitch`.
    pub fn slot_switch(
        &mut self,
        player: &super::super::P,
        request: crate::world::packet_handlers::slot_switch::SlotSwitch,
    ) -> Result<()> {
        let node = Node::Player(player.0.clone());
        crate::world::packet_handlers::slot_switch::process(&mut Ctx(self), &node, request)
    }
    /// A use-with pulse's body, if this world submitted it.
    pub fn use_pulse(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let at = self
            .item_actions
            .use_pulses
            .iter()
            .position(|(h, _)| h.same(pulse))?;
        let (_, player) = self.item_actions.use_pulses.remove(at);
        let ctx = Ctx(self);
        let result = match ctx.player_of(&player) {
            Ok(p) => ctx
                .0
                .inner
                .message(&p, crate::world::legacy_interaction::NOTHING)
                .map_err(err),
            // The player left: the pulse's check stops it.
            Err(_) => Ok(()),
        };
        Some(result.map(|()| true))
    }
}
