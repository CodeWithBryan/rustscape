//! Five small item-on-object listeners in the live world
//! (`world::item_on_object`, ported without a JVM corpus:
//! docs/item-on-object.md): SapCollectListener (the knife on a tree and the
//! sap bucket's "empty" option), PoisonFountainListener,
//! HairdresserCheeseListener, FishEasterEgg and GWDEntranceRopeListener,
//! registered in the native InteractionListeners at boot
//! (super::everyday's `install_content_use`).
//!
//! [`WithObject`] is the library's `Host` over the live world, through the
//! services [`Everyday`] already joins (the canonical inventory with its
//! listeners, the animator, messages) plus the action locks, the player's
//! attributes (a `/save:` key is saved with the player), the persisted
//! Falador diary and the varbits. The pulses are STANDARD pulses of the
//! player's canonical pulse manager, dispatched by identity
//! ([`WithObjectWorld`]): a walk stops them, as it does the original's.
use super::{Ctx, Node, P, everyday::Everyday};
use crate::{
    definitions::item::Shared,
    pulses,
    world::{
        action_locks,
        global_items::Anim,
        item_on_object::{self, Error, Kind, Pulse, Result},
        packet_handlers::Container as Held,
        player_managers::diary,
    },
};

/// The installed listeners' state.
#[derive(Default)]
pub struct WithObjectWorld {
    /// The pulses the lambdas ran in a player's pulse manager, with the
    /// player's node and the body.
    pub pulses: Vec<(pulses::Handle, usize, Pulse<Node>)>,
    /// The classes registered at boot, in scan order.
    pub registered: Vec<item_on_object::Class>,
}

fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

/// `DiaryType.FALADOR`'s ordinal.
const FALADOR: usize = 3;

/// `world::item_on_object`'s host over the live world.
pub struct WithObject<'a>(pub Everyday<'a>);

impl WithObject<'_> {
    fn player(&self, node: &Node) -> Result<P> {
        self.0.0.player_of(node).map_err(|_| Error::Cast)
    }
}

impl item_on_object::Host for WithObject<'_> {
    type Node = Node;

    fn kind(&self, node: &Node) -> Kind {
        match node {
            Node::Player(_) => Kind::Player,
            Node::Npc(_) => Kind::Npc,
            Node::Scenery(_) => Kind::Scenery,
            Node::Item(_) | Node::Ground(_) => Kind::Item,
        }
    }
    fn slot(&mut self, item: &Node) -> Result<i32> {
        Ok(Ctx::item(item).map_err(|_| Error::Cast)?.read().index)
    }
    fn in_inventory(&mut self, player: &Node, id: i32) -> Result<bool> {
        let inventory = self.0.0.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.contains(id, 1))
    }
    fn send_message(&mut self, player: &Node, text: &str) -> Result<()> {
        self.0.message(player, text)
    }
    fn remove_item_id(&mut self, player: &Node, id: i32) -> Result<bool> {
        let item = self.0.native(id, 1);
        self.0.remove_from_inventory(player, &item)
    }
    fn remove_item(&mut self, player: &Node, item: &Node) -> Result<bool> {
        // `removeItem<T>` of something that is neither an Item nor an Int.
        let item = Ctx::item(item).map_err(|_| Error::State)?;
        self.0.remove_from_inventory(player, &item)
    }
    fn add_item(&mut self, player: &Node, id: i32) -> Result<bool> {
        self.0.add_to_inventory(player, id, 1)
    }
    fn replace_slot(&mut self, player: &Node, slot: i32, id: i32) -> Result<()> {
        // `replaceSlot(player, slot, Item(id))`: no current item, so
        // `inventory.replace(item, slot)` (the listeners fire).
        let item = self.0.native(id, 1);
        self.0
            .0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.replace(Some(&item), slot, true, defs, ctx)
            })
            .map(|_| ())
            .map_err(bind)
    }
    fn animate(&mut self, entity: &Node, animation: i32) -> Result<()> {
        self.0.animate_entity(entity, Anim::new(animation))
    }
    fn attribute(&mut self, player: &Node, key: &str, default: bool) -> Result<bool> {
        let p = self.player(player)?;
        let attributes = self.0.0.0.inner.canonical(&p).player.attributes();
        // A value of another type is the generic read's ClassCastException.
        attributes.boolean(key, default).map_err(|_| Error::Cast)
    }
    fn set_attribute(&mut self, player: &Node, key: &str, value: bool) -> Result<()> {
        let p = self.player(player)?;
        let attributes = self.0.0.0.inner.canonical(&p).player.attributes();
        attributes.set_boolean(key, value);
        Ok(())
    }
    fn lock(&mut self, entity: &Node, ticks: i32) -> Result<()> {
        let p = self.player(entity)?;
        let state = &mut self.0.0.0.inner.state;
        let locks = state.locks(&p.0).map_err(bind)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(bind)
    }
    fn unlock(&mut self, entity: &Node) -> Result<()> {
        let p = self.player(entity)?;
        self.0.0.0.inner.state.locks(&p.0).map_err(bind)?.unlock();
        Ok(())
    }
    fn finish_falador_task(&mut self, player: &Node, level: i32, index: i32) -> Result<()> {
        let p = self.player(player)?;
        super::super::subclasses::with_diaries(self.0.0.0, &p, |h, owner| {
            diary::finish_task(h, owner, owner, Some(FALADOR), level, index)
        })
        .map_err(bind)
    }
    fn set_varbit(&mut self, player: &Node, id: i32, value: i32, save: bool) -> Result<()> {
        let p = self.player(player)?;
        crate::world::login::Host::set_varbit(self.0.0.0, &p, id, value, save).map_err(bind)
    }
    fn run_woodcutting_pulse(&mut self, player: &Node, scenery: &Node) -> Result<()> {
        let p = self.player(player)?;
        let Node::Scenery(tree) = scenery else {
            return Err(Error::Cast);
        };
        // super::woodcutting.
        self.0.0.0.run_woodcutting_pulse(&p, tree)
    }
    fn run_pulse(&mut self, pulse: Pulse<Node>, delay: i32) -> Result<()> {
        let p = self.player(pulse.player())?;
        // `Pulse(delay)`: no checked nodes.
        let handle = Shared::new(pulses::Pulse::new(
            delay,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            pulse.class().into(),
        ));
        let live = &mut *self.0.0.0;
        if live.run_standard_pulse(&p, handle.clone()).map_err(bind)? {
            let world = &mut live.round.with_object;
            world.pulses.retain(|(h, _, _)| h.read().running);
            world.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }
}

impl super::super::host::Live {
    /// A pulse one of the listeners ran in a pulse manager, by identity:
    /// `Some` with whether it finished.
    pub fn with_object_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.with_object.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let result = item_on_object::pulse(&mut WithObject(Everyday(Ctx(self))), &mut task);
        if matches!(result, Ok(false)) {
            self.round.with_object.pulses.push((handle, owner, task));
        }
        Some(result)
    }
}
