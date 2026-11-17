//! GrandExchangeInterface.java: the ComponentPlugin of the collection box
//! (109), the item search (389), the item sets (644/645) and the guide
//! prices (642). `handle` submits a one-tick pulse whose body does the work.
use super::{
    Error, Host, InterfacePlugin, Result,
    components::{
        EXCHANGE_ITEMSETS_645, EXCHANGE_SETS_SIDE_644, GUIDE_PRICE_642, OBJDIALOG_389,
        STOCKCOLLECT_109,
    },
    item_sets, records,
    sounds::GE_TRADE_OK_4044,
    stock_market,
    ui::{self, Items, Ui},
};
use crate::{
    definitions::item::Shared,
    inventory::item::{Handle as Item, Item as ItemValue},
    world::{
        component_objects::Plugin,
        interfaces::{self, Handle as Component, Registry},
        player_walk::NodeHandle,
    },
};

/// The components `newInstance` registers the plugin for, in order.
pub const COMPONENTS: [i32; 5] = [
    STOCKCOLLECT_109,
    OBJDIALOG_389,
    EXCHANGE_SETS_SIDE_644,
    EXCHANGE_ITEMSETS_645,
    GUIDE_PRICE_642,
];

/// `newInstance(arg)`: `ComponentDefinition.put(id, this)` for each
/// component. Returns the plugin object.
pub fn new_instance(registry: &mut Registry) -> Plugin {
    let plugin = Plugin::new(InterfacePlugin);
    for id in COMPONENTS {
        registry.for_id(id).write().plugin = Some(plugin.clone());
    }
    plugin
}

/// The arguments `handle` captures in its pulse.
#[derive(Clone)]
pub struct ButtonPulse {
    pub player: NodeHandle,
    pub component: Component,
    pub opcode: i32,
    pub button: i32,
    pub slot: i32,
    pub item: i32,
}

/// `handle(player, component, opcode, button, slot, itemId)`: always true.
pub fn handle<H: Host + ?Sized>(h: &mut H, pulse: ButtonPulse) -> Result<bool> {
    h.submit_pulse(super::Pulse::Interface(pulse))?;
    Ok(true)
}

/// The pulse body (true: the pulse is finished).
pub fn run_pulse<H: Host + ?Sized>(h: &mut H, b: &ButtonPulse) -> Result<bool> {
    let id = b.component.read().id;
    match id {
        EXCHANGE_SETS_SIDE_644 | EXCHANGE_ITEMSETS_645 => {
            handle_item_set(h, b)?;
        }
        OBJDIALOG_389 => {
            handle_search_interface(h, &b.player, b.button)?;
        }
        STOCKCOLLECT_109 => {
            handle_collection_box(h, &b.player, b.opcode, b.button, b.slot)?;
        }
        GUIDE_PRICE_642 => {
            handle_guide_price(h, &b.player, b.opcode, b.button)?;
        }
        _ => {}
    }
    Ok(true)
}

/// `handleSearchInterface`: button 10 closes the chatbox.
pub fn handle_search_interface<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    button: i32,
) -> Result<bool> {
    if button == 10 {
        let m = h.interface_manager(p)?;
        interfaces::close_chatbox(&mut Ui::new(h, p), &m)?;
        return Ok(true);
    }
    Ok(false)
}

/// `handleCollectionBox`: the three top and three bottom boxes; the slot's
/// half is the withdraw slot.
pub fn handle_collection_box<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    opcode: i32,
    button: i32,
    slot: i32,
) -> Result<bool> {
    let index = match button {
        18 | 23 | 28 => (button - 18) >> 2,
        36 | 44 | 52 => 3 + ((button - 36) >> 3),
        _ => -1,
    };
    let instance = records::get_instance(h, Some(p))?;
    if index > -1 {
        let record = instance.records()[index as usize];
        if let Some(o) = records::get_offer_record(h, record)? {
            stock_market::withdraw(h, p, &o, slot >> 1, opcode)?;
        }
    }
    Ok(true)
}

fn new_item<H: Host + ?Sized>(h: &mut H, id: i32, amount: i32) -> Result<Item> {
    let definitions = h.definitions()?;
    Ok(Shared::new(ItemValue::new(id, amount, 1000, &definitions)))
}

/// `handleItemSet`: Examine (124), Exchange (196: a set for its parts or
/// the parts for a set) and Components (155: the CS2 mapping 1089 text).
fn handle_item_set<H: Host + ?Sized>(h: &mut H, b: &ButtonPulse) -> Result<()> {
    if b.button != 16 && b.button != 0 {
        return Ok(());
    }
    let p = &b.player;
    let inventory_side = b.component.read().id == EXCHANGE_SETS_SIDE_644;
    let limit = if inventory_side {
        28
    } else {
        item_sets::COUNT as i32
    };
    if b.slot < 0 || b.slot >= limit {
        return Ok(());
    }
    let (item, set) = if inventory_side {
        let Some(item) = h.inventory(p)?.get(b.slot) else {
            return Ok(());
        };
        let Some(set) = item_sets::for_id(item.read().id()) else {
            return Ok(());
        };
        (item, set)
    } else {
        let set = b.slot as usize;
        let item = new_item(h, item_sets::item_id(set), 1)?;
        (item, set)
    };
    match b.opcode {
        124 => {
            let id = item.read().id();
            let examine = h.examine(id)?;
            h.message(p, &examine)?;
        }
        196 => {
            let definitions = h.definitions()?;
            if inventory_side {
                let free = h.inventory(p)?.free_slots();
                if free < item_sets::components(set).len() as i32 - 1 {
                    h.message(
                        p,
                        "You don't have enough inventory space for the component parts.",
                    )?;
                    return Ok(());
                }
                let mut inventory = h.inventory(p)?;
                if !inventory.remove(
                    Some(&item),
                    None,
                    false,
                    &definitions,
                    &mut Items { host: h },
                )? {
                    return Ok(());
                }
                for id in item_sets::components(set) {
                    let part = new_item(h, id, 1)?;
                    let mut inventory = h.inventory(p)?;
                    inventory.add(Some(&part), true, -1, &definitions, &mut Items { host: h })?;
                }
                let mut inventory = h.inventory(p)?;
                inventory.refresh(None, &definitions, &mut Items { host: h })?;
                h.message(
                    p,
                    "You successfully traded your set for its component items!",
                )?;
            } else {
                let parts = item_sets::components(set);
                if !h.inventory(p)?.contain_items(&parts) {
                    h.message(p, "You don't have the parts that make up this set.")?;
                    return Ok(());
                }
                for id in parts {
                    let part = new_item(h, id, 1)?;
                    let mut inventory = h.inventory(p)?;
                    inventory.remove(
                        Some(&part),
                        None,
                        false,
                        &definitions,
                        &mut Items { host: h },
                    )?;
                }
                let mut inventory = h.inventory(p)?;
                inventory.add(Some(&item), true, -1, &definitions, &mut Items { host: h })?;
                let mut inventory = h.inventory(p)?;
                inventory.refresh(None, &definitions, &mut Items { host: h })?;
                h.message(p, "You successfully traded your item components for a set!")?;
            }
            h.audio(p, GE_TRADE_OK_4044)?;
            let key = h.container_key(p, 93)?;
            let inventory = h.inventory(p)?;
            let items = ui::wire_items(&inventory);
            ui::send_items(h, p, -1, -2, key, &items)?;
        }
        155 => {
            if let Some(Some(text)) = h.cs2_mapping(1089, item_sets::item_id(set))? {
                h.message(p, &text)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `handleGuidePrice`: option 155 examines the guide item of the button.
fn handle_guide_price<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    opcode: i32,
    button: i32,
) -> Result<()> {
    if opcode != 155 {
        return Ok(());
    }
    let Some(kind) = h.guide_price(p)? else {
        return Ok(());
    };
    let mut subtract = 0;
    if (15..=23).contains(&button) {
        subtract = 15;
    }
    if (43..=57).contains(&button) {
        subtract = 43;
    }
    if (89..=103).contains(&button) {
        subtract = 89;
    }
    if (135..=144).contains(&button) {
        subtract = 135;
    }
    if (167..=182).contains(&button) {
        subtract = 167;
    }
    let items = kind.items();
    let index = usize::try_from(button.wrapping_sub(subtract))
        .ok()
        .filter(|i| *i < items.len())
        .ok_or(Error::Bounds)?;
    let examine = h.examine(items[index].0)?;
    h.message(p, &examine)
}
