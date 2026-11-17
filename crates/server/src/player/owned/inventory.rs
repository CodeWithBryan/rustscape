use super::Player;
use crate::{
    definitions::item::{Registry, Shared},
    inventory::{
        self as inv, bank,
        container::{Container, Event},
        container_listener::Handle as ListenerHandle,
        item::Handle,
        listener::Listener,
        player_binding::{self as binding, Links},
    },
    world::{
        player_render,
        player_walk::{Node, State},
    },
};
use rs09_protocol::game::{Frame, updates};
use std::{collections::BTreeMap, sync::Arc};

pub struct Services<'a> {
    state: &'a mut State,
    frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    player: Player,
    clock: fn() -> i64,
}
impl<'a> Services<'a> {
    pub(super) fn new(
        state: &'a mut State,
        frames: &'a mut BTreeMap<Node, Vec<Frame>>,
        player: Player,
        clock: fn() -> i64,
    ) -> Self {
        Self {
            state,
            frames,
            player,
            clock,
        }
    }
    fn receiver(&self, listener: &ListenerHandle) -> inv::Result<(Arc<Listener>, Player)> {
        let receiver = listener
            .value::<Listener>()
            .ok_or_else(|| inv::Error::Binding("unbound native container listener".into()))?;
        let owner = receiver.owner().ok_or(inv::Error::Null)?;
        let player = self
            .state
            .player_fields(&owner)
            .map_err(|e| inv::Error::Binding(e.to_string()))?;
        Ok((receiver, player))
    }
}
impl binding::Services for Services<'_> {
    type Familiar = Shared<bool>;
    type Appearance = player_render::Handle;
    fn send(&mut self, _: &mut Links<'_>, _: &mut Container, frame: Frame) -> inv::Result<()> {
        self.frames
            .entry(self.player.owner().node)
            .or_default()
            .push(frame);
        Ok(())
    }
    fn text(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        text: String,
        interface: i32,
        child: i32,
    ) -> inv::Result<()> {
        let counter = self.player.manager().packet_count(1);
        let frame =
            updates::interface_text(interface.wrapping_shl(16) | child, text.as_bytes(), counter)
                .map_err(|e| inv::Error::Binding(e.to_string()))?;
        self.send(links, inventory, frame)
    }
    fn familiar(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
    ) -> inv::Result<Option<Self::Familiar>> {
        Ok(Some(self.player.pouch()))
    }
    fn has_pouch(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        familiar: Self::Familiar,
    ) -> inv::Result<bool> {
        Ok(*familiar.read())
    }
    fn set_has_pouch(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        familiar: Self::Familiar,
        value: bool,
    ) -> inv::Result<()> {
        *familiar.write() = value;
        Ok(())
    }
    fn appearance(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
    ) -> inv::Result<Option<Self::Appearance>> {
        Ok(Some(self.player.masks().owner()))
    }
    fn sync(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        appearance: Self::Appearance,
    ) -> inv::Result<()> {
        let masks = self.player.masks();
        if !appearance.same(&masks.owner()) {
            return Err(inv::Error::Binding(
                "captured appearance is not this canonical player".into(),
            ));
        }
        // The supported fresh-player branch has no active familiar/pet.
        masks.body().write().using_summoning = *self.player.pouch().read();
        masks
            .sync_appearance(self.clock)
            .map_err(|e| inv::Error::Binding(e.to_string()))?;
        Ok(())
    }
    fn ground(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        _: Option<Handle>,
        _: &Registry,
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "inventory overflow needs the owning-world drop service".into(),
        ))
    }
    fn update_foreign(
        &mut self,
        _: &mut Links<'_>,
        listener: ListenerHandle,
        inventory: &mut Container,
        event: Event,
        _: &Registry,
    ) -> inv::Result<()> {
        if let Some(receiver) = listener.value::<bank::Listener>() {
            self.state
                .player_fields(&receiver.owner().ok_or(inv::Error::Null)?)
                .map_err(|e| inv::Error::Binding(e.to_string()))?;
            return receiver.update(inventory, Some(&event), self);
        }
        let (receiver, player) = self.receiver(&listener)?;
        player.apply(self.state, self.frames, self.clock, |_, _, context| {
            receiver.update_event(context, inventory, Some(&event))
        })
    }
    fn refresh_foreign(
        &mut self,
        _: &mut Links<'_>,
        listener: ListenerHandle,
        inventory: &mut Container,
        _: &Registry,
    ) -> inv::Result<()> {
        if let Some(receiver) = listener.value::<bank::Listener>() {
            self.state
                .player_fields(&receiver.owner().ok_or(inv::Error::Null)?)
                .map_err(|e| inv::Error::Binding(e.to_string()))?;
            return receiver.refresh(inventory, self);
        }
        let (receiver, player) = self.receiver(&listener)?;
        player.apply(self.state, self.frames, self.clock, |_, _, context| {
            receiver.refresh(context, inventory)
        })
    }
}
impl bank::ListenerContext for Services<'_> {
    fn listener_bank(
        &mut self,
        owner: &crate::world::player_walk::NodeHandle,
    ) -> inv::Result<bank::Bank> {
        self.state
            .player_fields(owner)
            .map(|p| p.bank())
            .map_err(|e| inv::Error::Binding(e.to_string()))
    }
    fn listener_packet(&mut self, packet: bank::Packet) -> inv::Result<()> {
        let owner = packet.player.as_ref().ok_or(inv::Error::Null)?;
        let player = self
            .state
            .player_fields(owner)
            .map_err(|e| inv::Error::Binding(e.to_string()))?;
        if !self
            .state
            .actor(owner.node)
            .map_err(|e| inv::Error::Binding(e.to_string()))?
            .connected
        {
            return Err(inv::Error::Binding(
                "bank listener recipient is disconnected".into(),
            ));
        }
        let frame = packet.frame(&player.core().null_item())?;
        self.frames.entry(owner.node).or_default().push(frame);
        Ok(())
    }
    fn listener_varbit(
        &mut self,
        owner: &crate::world::player_walk::NodeHandle,
        id: i32,
        value: i32,
        save: bool,
    ) -> inv::Result<()> {
        let player = self
            .state
            .player_fields(owner)
            .map_err(|e| inv::Error::Binding(e.to_string()))?;
        player
            .apply_world_variables(self.state, self.frames, |ctx| {
                crate::player::variables::set_varbit(ctx, Some(owner), id, value, save)
            })
            .map_err(|e| inv::Error::Binding(format!("{e:?}")))
    }
    fn listener_varc(
        &mut self,
        owner: &crate::world::player_walk::NodeHandle,
        id: i32,
        value: i32,
    ) -> inv::Result<()> {
        let player = self
            .state
            .player_fields(owner)
            .map_err(|e| inv::Error::Binding(e.to_string()))?;
        player
            .apply_world_variables(self.state, self.frames, |ctx| {
                crate::player::variables::set_varc(ctx, Some(owner), id, value)
            })
            .map_err(|e| inv::Error::Binding(format!("{e:?}")))
    }
}
