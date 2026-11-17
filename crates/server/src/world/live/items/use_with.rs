//! `processUseWith` (`world::packet_handlers::use_with`) over the live world:
//! item on item, NPC, player and ground item, with the zone monitor, the
//! native `InteractionListeners` (`run_use`), PluginInteractionManager (no
//! plugins) and `UseWithHandler.HANDLERS` (no handler class is ported: the
//! list is empty, the original's answer with no content registered).
//! An unhandled item-on-item use runs `runUnhandledAction` ("Nothing
//! interesting happens." a tick later, as a world pulse). Use-with on scenery
//! and the movement pulses to NPCs, players and ground items are refused by
//! name (the use-with MovementPulse is not joined to the canonical movement).
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

/// A use-with pulse: `runUnhandledAction`'s.
#[derive(Clone)]
pub enum UsePulse {
    Unhandled(Node),
}

impl use_with::Context for Ctx<'_> {
    type Pulse = UsePulse;
    fn object(&mut self, _: i32, _: i32, _: i32, _: i32, _: i32) -> Result<Option<Node>> {
        refused("use-with on scenery (scenery is not a node of the item handlers here)")
    }
    fn scenery_child(&mut self, _: &Node, _: &Node) -> Result<Option<Node>> {
        refused("use-with on scenery (scenery is not a node of the item handlers here)")
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
            .push((handle, UsePulse::Unhandled(player.clone())));
        Ok(())
    }
    fn movement(
        &mut self,
        _: &Node,
        _: &Node,
        _: Option<&UseHandler>,
        _: Body<Node>,
    ) -> Result<UsePulse> {
        refused("UseWithHandler's MovementPulse (use-with on NPCs, players and ground items)")
    }
    fn delayed(&mut self, _: &Event<Node>, _: Body<Node>) -> Result<UsePulse> {
        refused("UseWithHandler's item-on-item Pulse (no content use-with handler is ported)")
    }
    fn run_pulse(&mut self, _: &Node, pulse: UsePulse) -> Result<()> {
        match pulse {
            UsePulse::Unhandled(_) => refused("a use-with pulse on the entity pulse manager"),
        }
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
        let (_, body) = self.item_actions.use_pulses.remove(at);
        Some(match body {
            UsePulse::Unhandled(player) => {
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
                result.map(|()| true)
            }
        })
    }
}
