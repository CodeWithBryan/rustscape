//! `processSlotSwitch` (`world::packet_handlers::slot_switch`) over the
//! canonical containers: inventory moves and inserts. The bank's tab calls
//! are refused by name (the bank interface is not installed in the live
//! world).
use super::{Ctx, Node, err, refused};
use crate::world::{
    interfaces,
    packet_handlers::{Container, Error, Result, slot_switch},
};

impl slot_switch::Context for Ctx<'_> {
    fn insert_items(&mut self, _: &Node) -> Result<bool> {
        refused("bank.isInsertItems (the bank is not installed)")
    }
    fn free_slot(&mut self, player: &Node, container: Container) -> Result<i32> {
        Ok(self.container(player, container)?.free_slot())
    }
    fn tab_start_slot(&mut self, _: &Node, _: i32) -> Result<i32> {
        refused("bank.tabStartSlot (the bank is not installed)")
    }
    fn items_in_tab(&mut self, _: &Node, _: i32) -> Result<i32> {
        refused("bank.getItemsInTab (the bank is not installed)")
    }
    fn remove(&mut self, player: &Node, container: Container, item: &Node) -> Result<bool> {
        let item = Self::item(item)?;
        self.with_container(player, container, |c, defs, ctx| {
            c.remove(Some(&item), None, true, defs, ctx)
        })
    }
    fn add(&mut self, player: &Node, container: Container, item: &Node) -> Result<bool> {
        let item = Self::item(item)?;
        self.with_container(player, container, |c, defs, ctx| {
            c.add(Some(&item), true, -1, defs, ctx)
        })
    }
    fn tab_by_item_slot(&mut self, _: &Node, _: i32) -> Result<i32> {
        refused("bank.getTabByItemSlot (the bank is not installed)")
    }
    fn insert(
        &mut self,
        player: &Node,
        container: Container,
        from: i32,
        to: i32,
        update: bool,
    ) -> Result<()> {
        self.with_container(player, container, |c, defs, ctx| {
            c.insert(from, to, update, defs, ctx)
        })
    }
    fn increase_tab_start_slots(&mut self, _: &Node, _: i32) -> Result<()> {
        refused("bank.increaseTabStartSlots (the bank is not installed)")
    }
    fn decrease_tab_start_slots(&mut self, _: &Node, _: i32) -> Result<()> {
        refused("bank.decreaseTabStartSlots (the bank is not installed)")
    }
    fn set_tab_configurations(&mut self, _: &Node) -> Result<()> {
        refused("bank.setTabConfigurations (the bank is not installed)")
    }
    fn has_chatbox(&mut self, player: &Node) -> Result<bool> {
        let p = self.player_of(player)?;
        let manager = self.0.inner.canonical(&p).player.manager();
        Ok(interfaces::has_chatbox(&manager))
    }
    fn attribute_bool(&mut self, player: &Node, key: &str, default: bool) -> Result<bool> {
        self.canonical(player)?
            .attributes()
            .boolean(key, default)
            .map_err(|_| Error::Cast)
    }
    fn close_chatbox(&mut self, player: &Node) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.current = Some(p.clone());
        let manager = self.0.inner.canonical(&p).player.manager();
        interfaces::close_chatbox(&mut self.0.inner, &manager).map_err(err)?;
        self.0.inner.drain(&p);
        Ok(())
    }
    fn refresh(&mut self, player: &Node, container: Container) -> Result<()> {
        self.with_container(player, container, |c, defs, ctx| c.refresh(None, defs, ctx))
    }
}
