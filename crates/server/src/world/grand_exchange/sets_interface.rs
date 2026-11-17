//! ExchangeItemSets.kt (the item sets interface 645 with its inventory side
//! 644 and the inventory listener that redraws both) and the members of
//! InterfaceContainer.java it uses (`generateItems`, `generate`,
//! `increment`, the script arguments).
use super::{
    Host, Result, SetsClose,
    components::{EXCHANGE_ITEMSETS_645, EXCHANGE_SETS_SIDE_644},
    item_sets,
    ui::{self, Ui},
};
use crate::{
    inventory::container_listener::{Handle as Listener, NativeListener},
    world::{
        interface_listeners::{Callback, Registry},
        interfaces,
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::updates::{ScriptArg, WireItem};

/// ExchangeItemSets' private `InventoryListener(player)`.
pub struct SetsListener {
    pub player: NodeHandle,
}
impl NativeListener for SetsListener {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        vec![self.player.clone()]
    }
}

/// `defineInterfaceListeners()`: `onClose(645)`.
pub fn define_interface_listeners(registry: &Registry) -> Result<()> {
    registry.add_close(EXCHANGE_ITEMSETS_645, Callback::open(SetsClose))?;
    Ok(())
}

/// The three options of both item grids.
pub const OPTIONS: [&str; 3] = ["Examine", "Exchange", "Components"];

/// `onClose(645)`: the listener leaves the inventory, the side closes and
/// both attributes go.
pub fn sets_close<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    let listener = h.sets_listener(p)?;
    let inventory = h.inventory(p)?;
    inventory.listener_list().remove_first(listener);
    let m = h.interface_manager(p)?;
    interfaces::close_single_tab(&mut Ui::new(h, p), &m)?;
    h.remove_container_key(p)?;
    h.remove_sets_listener(p)?;
    Ok(true)
}

/// `ExchangeItemSets.openFor(player)`.
pub fn open_for<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<()> {
    let m = h.interface_manager(p)?;
    let main = h.ui_component(p, EXCHANGE_ITEMSETS_645)?;
    interfaces::open(&mut Ui::new(h, p), &m, Some(main))?;
    let m = h.interface_manager(p)?;
    let side = h.ui_component(p, EXCHANGE_SETS_SIDE_644)?;
    interfaces::open_single_tab(&mut Ui::new(h, p), &m, &side)?;
    // The constructor's `init` draws both grids before the attribute is set.
    create_containers(h, p)?;
    let listener = Listener::new(SetsListener { player: p.clone() });
    h.set_sets_listener(p, listener.clone())?;
    h.inventory(p)?.listener_list().add(Some(listener));
    Ok(())
}

/// `update(c, event)`.
pub fn listener_update<H: Host + ?Sized>(h: &mut H, listener: &SetsListener) -> Result<()> {
    create_containers(h, &listener.player)
}
/// `refresh(c)`.
pub fn listener_refresh<H: Host + ?Sized>(h: &mut H, listener: &SetsListener) -> Result<()> {
    create_containers(h, &listener.player)
}

/// `createContainers(player)`: the inventory grid (its key stored as
/// "container-key") and the sets grid.
fn create_containers<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<()> {
    let inventory = h.inventory(p)?;
    let items = ui::wire_items(&inventory);
    let key = generate_items(h, p, &items, &OPTIONS, EXCHANGE_SETS_SIDE_644, 0, 7, 4)?;
    h.set_container_key(p, key)?;
    let sets: Vec<Option<WireItem>> = item_sets::item_array()
        .into_iter()
        .map(|id| Some(WireItem { id, amount: 1 }))
        .collect();
    generate_items(h, p, &sets, &OPTIONS, EXCHANGE_ITEMSETS_645, 16, 15, 10)?;
    Ok(())
}

/// `InterfaceContainer.increment()`: the world's key, wrapping from 6999
/// back to 600.
pub fn increment<H: Host + ?Sized>(h: &mut H) -> Result<i32> {
    let exchange = h.exchange()?;
    let mut t = exchange.tables();
    if t.container_index == 6999 {
        t.container_index = 600;
    }
    let key = t.container_index;
    t.container_index = key.wrapping_add(1);
    Ok(key)
}

/// `generateItems(player, items, options, interface, child, x, y)`: the key
/// is taken first; `generate` sends with it and returns the next key, which
/// is what the caller receives.
#[allow(clippy::too_many_arguments)]
pub fn generate_items<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    items: &[Option<WireItem>],
    options: &[&str],
    interface: i32,
    child: i32,
    x: i32,
    y: i32,
) -> Result<i32> {
    let key = increment(h)?;
    let mut types = String::from("IviiiI");
    types.push_str(&"s".repeat(options.len()));
    let mut objects: Vec<ScriptArg> = options
        .iter()
        .map(|o| ScriptArg::Text(o.as_bytes().to_vec()))
        .collect();
    objects.extend([
        ScriptArg::Int(-1),
        ScriptArg::Int(0),
        ScriptArg::Int(x),
        ScriptArg::Int(y),
        ScriptArg::Int(key),
        ScriptArg::Int(interface.wrapping_shl(16) | child),
    ]);
    ui::send_run_script(h, p, 150, &types, &objects)?;
    ui::send_iface_settings(h, p, ALL_OPTIONS, child, interface, 0, items.len() as i32)?;
    ui::send_items(h, p, -1, -2, key, items)?;
    increment(h)
}

/// `IfaceSettingsBuilder().enableAllOptions().build()`.
pub const ALL_OPTIONS: i32 = 2046;
