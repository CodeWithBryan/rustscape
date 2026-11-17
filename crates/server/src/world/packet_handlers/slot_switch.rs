//! `processSlotSwitch` and `PacketProcessor.switchItem`: moving items within
//! the inventory and the bank, and dragging bank items onto a tab.
use super::{Base, Container, Error, Result};
use crate::inventory::bank::array_index;
use rs09_protocol::game::actions::ClientAction;

/// The bank's main container child.
pub const BANK_ITEMS_CHILD: i32 = 73;
/// How deep the chatbox-closing recursion of `switchItem` may go here. The
/// original recurses until `hasChatbox` turns false (a binding that never
/// closes the chatbox overflows the JVM stack); this port stops instead.
pub const MAX_RECURSION: usize = 1024;

/// The two slot switch packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotSwitch {
    Multi {
        source_iface: i32,
        source_child: i32,
        source_slot: i32,
        dest_iface: i32,
        dest_child: i32,
        dest_slot: i32,
    },
    Single {
        iface: i32,
        child: i32,
        source_slot: i32,
        dest_slot: i32,
        is_insert: bool,
    },
}

impl SlotSwitch {
    pub fn from_action(action: &ClientAction) -> Option<Self> {
        Some(match *action {
            ClientAction::SlotSwitchMultiComponent {
                source_iface,
                source_child,
                source_slot,
                dest_iface,
                dest_child,
                dest_slot,
            } => Self::Multi {
                source_iface,
                source_child,
                source_slot,
                dest_iface,
                dest_child,
                dest_slot,
            },
            ClientAction::SlotSwitchSingleComponent {
                iface,
                child,
                source_slot,
                dest_slot,
                is_insert,
            } => Self::Single {
                iface,
                child,
                source_slot,
                dest_slot,
                is_insert,
            },
            _ => return None,
        })
    }
}

/// The container and bank calls of the two methods (each one a method of
/// `inventory::container` or `inventory::bank`).
pub trait Context: Base {
    /// `bank.isInsertItems()`.
    fn insert_items(&mut self, player: &Self::Node) -> Result<bool>;
    /// `container.freeSlot()`.
    fn free_slot(&mut self, player: &Self::Node, container: Container) -> Result<i32>;
    /// `bank.tabStartSlot[tab]`.
    fn tab_start_slot(&mut self, player: &Self::Node, tab: i32) -> Result<i32>;
    /// `bank.getItemsInTab(tab)`.
    fn items_in_tab(&mut self, player: &Self::Node, tab: i32) -> Result<i32>;
    /// `container.remove(item)`.
    fn remove(
        &mut self,
        player: &Self::Node,
        container: Container,
        item: &Self::Node,
    ) -> Result<bool>;
    /// `container.add(item)`.
    fn add(&mut self, player: &Self::Node, container: Container, item: &Self::Node)
    -> Result<bool>;
    /// `bank.getTabByItemSlot(slot)`.
    fn tab_by_item_slot(&mut self, player: &Self::Node, slot: i32) -> Result<i32>;
    /// `container.insert(from, to[, update])` (the two-argument form updates).
    fn insert(
        &mut self,
        player: &Self::Node,
        container: Container,
        from: i32,
        to: i32,
        update: bool,
    ) -> Result<()>;
    /// `bank.increaseTabStartSlots(tab)`.
    fn increase_tab_start_slots(&mut self, player: &Self::Node, tab: i32) -> Result<()>;
    /// `bank.decreaseTabStartSlots(tab)`.
    fn decrease_tab_start_slots(&mut self, player: &Self::Node, tab: i32) -> Result<()>;
    /// `bank.setTabConfigurations()`.
    fn set_tab_configurations(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.interfaceManager.hasChatbox()`.
    fn has_chatbox(&mut self, player: &Self::Node) -> Result<bool>;
    /// `getAttribute(player, key, default)` for a `Boolean`.
    fn attribute_bool(&mut self, player: &Self::Node, key: &str, default: bool) -> Result<bool>;
    /// `player.interfaceManager.closeChatbox()`.
    fn close_chatbox(&mut self, player: &Self::Node) -> Result<()>;
    /// `container.refresh()`.
    fn refresh(&mut self, player: &Self::Node, container: Container) -> Result<()>;
}

/// `processSlotSwitch`.
pub fn process<C: Context>(ctx: &mut C, player: &C::Node, request: SlotSwitch) -> Result<()> {
    match request {
        SlotSwitch::Multi {
            source_slot,
            dest_iface,
            dest_child,
            dest_slot,
            ..
        } => {
            if dest_iface != super::BANK_V2_MAIN_762 {
                return switch_item(
                    ctx,
                    source_slot,
                    dest_slot,
                    Some(Container::Inventory),
                    false,
                    player,
                );
            }
            if dest_child == BANK_ITEMS_CHILD {
                let insert = ctx.insert_items(player)?;
                return switch_item(
                    ctx,
                    source_slot,
                    dest_slot,
                    Some(Container::Bank),
                    insert,
                    player,
                );
            }
            let tab = array_index(dest_child);
            if tab <= -1 {
                return Ok(());
            }
            let second = if tab == 10 {
                ctx.free_slot(player, Container::Bank)?
            } else {
                let start = ctx.tab_start_slot(player, tab)?;
                start.wrapping_add(ctx.items_in_tab(player, tab)?)
            };
            // `val inSlot: Item = bank.get(slot)`: Kotlin checks it for null.
            let in_slot = ctx
                .get(player, Container::Bank, source_slot)?
                .ok_or(Error::Null)?;
            if second == -1 && ctx.remove(player, Container::Bank, &in_slot)? {
                ctx.add(player, Container::Bank, &in_slot)?;
                return Ok(());
            }
            let child_id = ctx.tab_by_item_slot(player, source_slot)?;
            if second > source_slot {
                ctx.insert(
                    player,
                    Container::Bank,
                    source_slot,
                    second.wrapping_sub(1),
                    true,
                )?;
            } else if source_slot > second {
                ctx.insert(player, Container::Bank, source_slot, second, true)?;
            }
            ctx.increase_tab_start_slots(player, tab)?;
            ctx.decrease_tab_start_slots(player, child_id)?;
            ctx.set_tab_configurations(player)
        }
        SlotSwitch::Single {
            iface,
            source_slot,
            dest_slot,
            is_insert,
            ..
        } => {
            let container = if iface == super::BANK_V2_MAIN_762 {
                Container::Bank
            } else {
                Container::Inventory
            };
            switch_item(
                ctx,
                source_slot,
                dest_slot,
                Some(container),
                is_insert,
                player,
            )
        }
    }
}

/// `switchItem(slot, secondSlot, container, insert, player)`.
pub fn switch_item<C: Context>(
    ctx: &mut C,
    slot: i32,
    second: i32,
    container: Option<Container>,
    insert: bool,
    player: &C::Node,
) -> Result<()> {
    switch(ctx, slot, second, container, insert, player, 0)
}

fn switch<C: Context>(
    ctx: &mut C,
    slot: i32,
    second_slot: i32,
    container: Option<Container>,
    insert: bool,
    player: &C::Node,
    depth: usize,
) -> Result<()> {
    let Some(container) = container else {
        return Ok(());
    };
    if slot < 0 || slot >= ctx.capacity(player, container)? || second_slot < 0 {
        return Ok(());
    }
    if second_slot >= ctx.capacity(player, container)? {
        return Ok(());
    }
    let item = ctx.get(player, container, slot)?;
    let second = ctx.get(player, container, second_slot)?;
    if ctx.has_chatbox(player)? && !ctx.attribute_bool(player, "close_c_", false)? {
        ctx.close_chatbox(player)?;
        if depth >= MAX_RECURSION {
            return Err(Error::Binding(Some(
                "switchItem: the chatbox never closed".into(),
            )));
        }
        switch(
            ctx,
            second_slot,
            slot,
            Some(container),
            insert,
            player,
            depth + 1,
        )?;
        return ctx.refresh(player, container);
    }
    let Some(item) = item else {
        return Ok(());
    };
    if !insert {
        ctx.replace(player, container, second.as_ref(), slot, false)?;
        ctx.replace(player, container, Some(&item), second_slot, false)?;
        ctx.set_index(&item, second_slot)?;
        if let Some(second) = &second {
            ctx.set_index(second, slot)?;
        }
    } else {
        ctx.insert(player, container, slot, second_slot, false)?;
    }
    ctx.refresh(player, container)
}
