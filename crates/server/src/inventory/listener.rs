//! Complete InventoryListener control flow over native containers/items.
//! Player service receivers and packet delivery belong to the owning world.
use super::{
    Error, Result,
    container::{Container, Event, Items},
    item::Handle,
};
use crate::world::player_walk::NodeHandle;
use rs09_protocol::game::{
    Frame,
    updates::{self, WireItem},
};

/// Retains the original array and captures event slots before packet delivery.
/// These are live item references, never a deferred value snapshot.
pub struct Packet {
    pub player: Option<NodeHandle>,
    pub items: Items,
    pub slots: Option<Vec<usize>>,
}
impl Packet {
    /// Original ContainerPacket for InventoryListener's149:0/container93 input.
    /// Partial updates compare identity with the owning world's NULL_ITEM.
    pub fn frame(&self, null_item: &Handle) -> Result<Frame> {
        let items = self.items.read();
        let wire = |item: &Handle| {
            let item = item.read();
            WireItem {
                id: item.id(),
                amount: item.amount(),
            }
        };
        let result = if let Some(slots) = &self.slots {
            let slots = slots
                .iter()
                .map(|&slot| {
                    let item = items.get(slot).ok_or(Error::Bounds)?;
                    Ok((
                        slot as i32,
                        item.as_ref().filter(|item| !item.same(null_item)).map(wire),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            updates::inventory_partial(149 << 16, 93, &slots)
        } else {
            let items: Vec<_> = items.iter().map(|item| item.as_ref().map(wire)).collect();
            updates::inventory_full(149 << 16, 93, &items)
        };
        result.map_err(|error| Error::Binding(error.to_string()))
    }
}

/// Required live receiver lookups remain distinct from their method calls.
/// Callbacks may mutate/reenter the passed container. No guard spans a callback.
pub trait Context {
    type Settings;
    type Familiar;
    type Appearance;
    fn send(&mut self, packet: Packet, container: &mut Container) -> Result<()>;
    fn settings(
        &mut self,
        player: &NodeHandle,
        container: &mut Container,
    ) -> Result<Option<Self::Settings>>;
    fn update_weight(&mut self, settings: Self::Settings, container: &mut Container) -> Result<()>;
    fn familiar(
        &mut self,
        player: &NodeHandle,
        container: &mut Container,
    ) -> Result<Option<Self::Familiar>>;
    fn has_pouch(&mut self, familiar: Self::Familiar, container: &mut Container) -> Result<bool>;
    fn set_has_pouch(
        &mut self,
        familiar: Self::Familiar,
        value: bool,
        container: &mut Container,
    ) -> Result<()>;
    fn appearance(
        &mut self,
        player: &NodeHandle,
        container: &mut Container,
    ) -> Result<Option<Self::Appearance>>;
    fn sync(&mut self, appearance: Self::Appearance, container: &mut Container) -> Result<()>;
}

pub struct Listener {
    player: Option<NodeHandle>,
}
impl Listener {
    pub fn owner(&self) -> Option<NodeHandle> {
        self.player.clone()
    }
    pub fn new(player: Option<NodeHandle>) -> Self {
        Self { player }
    }
    pub fn update(&self, ctx: &mut impl Context, container: &mut Container) -> Result<()> {
        let player = self.player.as_ref().ok_or(Error::Null)?;
        let settings = ctx.settings(player, container)?.ok_or(Error::Null)?;
        ctx.update_weight(settings, container)?;
        let familiar = ctx.familiar(player, container)?.ok_or(Error::Null)?;
        let had_pouch = ctx.has_pouch(familiar, container)?;
        let pouch = container
            .items()
            .read()
            .iter()
            .flatten()
            .any(|item| super::pouch_ids::contains(item.read().id()));
        let familiar = ctx.familiar(player, container)?.ok_or(Error::Null)?;
        ctx.set_has_pouch(familiar, pouch, container)?;
        if had_pouch != pouch {
            let appearance = ctx.appearance(player, container)?.ok_or(Error::Null)?;
            ctx.sync(appearance, container)?;
        }
        Ok(())
    }
    pub fn refresh(&self, ctx: &mut impl Context, container: &mut Container) -> Result<()> {
        ctx.send(
            Packet {
                player: self.player.clone(),
                items: container.items(),
                slots: None,
            },
            container,
        )?;
        self.update(ctx, container)
    }
    pub fn update_event(
        &self,
        ctx: &mut impl Context,
        container: &mut Container,
        event: Option<&Event>,
    ) -> Result<()> {
        let event = event.ok_or(Error::Null)?;
        let items = event.read().items.clone();
        let slots = event.read().slots();
        ctx.send(
            Packet {
                player: self.player.clone(),
                items,
                slots: Some(slots),
            },
            container,
        )?;
        self.update(ctx, container)
    }
}
