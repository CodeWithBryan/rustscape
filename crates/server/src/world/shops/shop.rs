//! Shop.kt: ShopItem, ShopListener and every Shop member, in the original
//! call order. Field reads the original repeats are repeated. No shop or
//! table lock is held while a host, container or interface call runs.
use super::{
    Error, Host, PERSONALIZED_SHOPS, Result, ShopEvent, View, fixed_price, java_hash, lowercase,
    node_captures,
    ui::{Items, Ui},
};
use crate::{
    definitions::{
        item::{Handle as Definition, Registry, Shared},
        property::Property,
    },
    inventory::{
        container::{Container, ContainerType, SortType},
        container_listener::{Handle as Listener, NativeListener},
        item::{self as items, Handle as Item, Item as ItemValue},
    },
    world::{
        component_objects::{Captures, NativeObject},
        drop_tables::LogLevel,
        input_prompt::Response,
        interfaces,
        player_walk::NodeHandle,
        repository::hash_map::JavaHashMap,
    },
};
use rs09_protocol::game::updates::{self, ScriptArg, WireItem};
use std::sync::{Arc, Mutex, MutexGuard};

/// `data class ShopItem(var itemId, var amount, val restockRate = 100)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopItem {
    pub item_id: i32,
    pub amount: i32,
    pub restock_rate: i32,
}

/// `ShopListener(player, itemView)`. `view` is `None` for the shared default
/// view (`{ _, item -> item }`).
pub struct ShopListener {
    pub player: NodeHandle,
    pub view: View,
}
impl NativeListener for ShopListener {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        vec![self.player.clone()]
    }
}

pub struct Fields {
    pub title: String,
    pub stock: Vec<ShopItem>,
    pub general: bool,
    pub currency: i32,
    pub high_alch: bool,
    pub force_shared: bool,
    pub stock_instances: JavaHashMap<i32, Container>,
    pub player_stock: Container,
    pub needs_update: JavaHashMap<i32, bool>,
    pub restock_rates: JavaHashMap<i32, i32>,
}
/// One Shop object. Clones are the same object.
#[derive(Clone)]
pub struct Shop(Arc<Mutex<Fields>>);
impl Shop {
    pub fn fields(&self) -> MutexGuard<'_, Fields> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    pub fn title(&self) -> String {
        self.fields().title.clone()
    }
    pub fn stock(&self) -> Vec<ShopItem> {
        self.fields().stock.clone()
    }
    pub fn general(&self) -> bool {
        self.fields().general
    }
    pub fn currency(&self) -> i32 {
        self.fields().currency
    }
    pub fn high_alch(&self) -> bool {
        self.fields().high_alch
    }
    pub fn force_shared(&self) -> bool {
        self.fields().force_shared
    }
    pub fn player_stock(&self) -> Container {
        self.fields().player_stock.clone()
    }
    pub fn instance(&self, key: i32) -> Option<Container> {
        self.fields().stock_instances.get(&key).cloned()
    }
    /// `stockInstances` in HashMap iteration order.
    pub fn instances(&self) -> Vec<(i32, Container)> {
        self.fields()
            .stock_instances
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }
    fn put_instance(&self, key: i32, container: Container) {
        self.fields().stock_instances.put(key, container);
    }
    fn stock_item(&self, index: i32) -> Result<ShopItem> {
        let f = self.fields();
        usize::try_from(index)
            .ok()
            .and_then(|i| f.stock.get(i).cloned())
            .ok_or(Error::Bounds)
    }
    fn stock_len(&self) -> usize {
        self.fields().stock.len()
    }
    fn set_needs_update(&self, key: i32) {
        self.fields().needs_update.put(key, true);
    }
}

/// `TransactionStatus`: Success or Failure(reason).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    Failure(String),
}

/// The `{ value -> shop.buy/sell(player, slot, value as Int) }` lambda given to
/// `sendInputDialogue`: it captures the handler's player, the shop found when
/// the button was pressed, and the slot.
#[derive(Clone)]
pub struct AmountScript {
    pub shop: Shop,
    pub player: NodeHandle,
    pub slot: i32,
    /// 620's "Enter the amount to buy:" rather than 621's "Enter the amount to sell:".
    pub buy: bool,
}
impl NativeObject for AmountScript {
    fn captures(&self) -> Captures {
        node_captures(vec![self.player.clone()])
    }
}

/// The handler body: `val amt = value as Int` then `shop.buy` or `shop.sell`;
/// the status is discarded. A value that is not an Int is ClassCastException,
/// which the input prompt turns into its dialogue.
pub fn run_amount<H: Host>(h: &mut H, script: &AmountScript, value: &Response) -> Result<()> {
    let Response::Int(amount) = value else {
        return Err(Error::ClassCast);
    };
    if script.buy {
        buy(h, &script.shop, &script.player, script.slot, *amount)?;
    } else {
        sell(h, &script.shop, &script.player, script.slot, *amount)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ helpers

fn new_item(id: i32, amount: i32, definitions: &Registry) -> Item {
    Shared::new(ItemValue::new(id, amount, 1000, definitions))
}
fn id(item: &Item) -> i32 {
    item.read().id()
}
fn amount(item: &Item) -> i32 {
    item.read().amount()
}
fn set_amount(item: &Item, value: i32) {
    item.write().set_amount(value);
}
fn definition(item: &Item) -> Result<Definition> {
    item.read().definition.clone().ok_or(Error::Null)
}
fn name(item: &Item) -> String {
    item.read().name.clone().unwrap_or_else(|| "null".into())
}
/// `definition.getConfiguration(key, 1)` cast to Int.
fn config_int(definition: &Definition, key: &str) -> Result<i32> {
    match definition.read().handlers.get(key) {
        None | Some(Property::Null) => Ok(1),
        Some(Property::Int(value)) => Ok(*value),
        Some(_) => Err(Error::ClassCast),
    }
}
/// `ItemDefinition.hasShopCurrencyValue(String)`: `getOrDefault(key, "0") != "0"`
/// compares references, so any stored entry (even null) answers true.
fn has_shop_currency_key(definition: &Definition, key: &str) -> bool {
    definition.read().handlers.contains_key(key)
}
/// `ItemDefinition.hasShopCurrencyValue(int currency)`.
pub fn has_shop_currency_value(definition: &Definition, currency: i32) -> Result<bool> {
    Ok(match currency {
        995 => items::definition_tradeable(definition)?,
        6529 => has_shop_currency_key(definition, "tokkul_price"),
        1464 => has_shop_currency_key(definition, "archery_ticket_price"),
        4067 => has_shop_currency_key(definition, "castle_wars_ticket_price"),
        _ => false,
    })
}
/// Kotlin `Double.roundToInt()`: NaN refused, saturating, `Math.round` otherwise.
fn round_to_int(value: f64) -> Result<i32> {
    if value.is_nan() {
        return Err(Error::Argument);
    }
    if value > f64::from(i32::MAX) {
        return Ok(i32::MAX);
    }
    if value < f64::from(i32::MIN) {
        return Ok(i32::MIN);
    }
    Ok(java_round(value) as i32)
}
/// JDK 11 `Math.round(double)`.
fn java_round(a: f64) -> i64 {
    let bits = a.to_bits() as i64;
    let biased = (bits & 0x7FF0_0000_0000_0000) >> 52;
    let shift = (53 - 2 + 1023) - biased;
    if shift & -64 == 0 {
        let mut r = (bits & 0x000F_FFFF_FFFF_FFFF) | (0x000F_FFFF_FFFF_FFFF + 1);
        if bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        a as i64
    }
}

fn protocol<T>(r: std::result::Result<T, rs09_protocol::Error>) -> Result<T> {
    r.map_err(|e| Error::Service(format!("protocol: {e}")))
}
fn wire(item: &Item) -> WireItem {
    let item = item.read();
    WireItem {
        id: item.id(),
        amount: item.amount(),
    }
}
/// `PacketDispatch.sendString` (setInterfaceText).
fn send_string<H: Host>(
    h: &mut H,
    p: &NodeHandle,
    text: &str,
    interface: i32,
    line: i32,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = protocol(updates::interface_text(
        interface.wrapping_shl(16) | (line & 0xFFFF),
        text.as_bytes(),
        m.packet_count(1),
    ))?;
    h.send(p, frame)
}
/// `PacketDispatch.sendIfaceSettings(settings, child, interface, offset, length)`.
pub(crate) fn send_iface_settings<H: Host>(
    h: &mut H,
    p: &NodeHandle,
    settings: i32,
    child: i32,
    interface: i32,
    offset: i32,
    length: i32,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = updates::interface_events(
        interface.wrapping_shl(16) | child,
        offset,
        length,
        settings,
        m.packet_count(1),
    );
    h.send(p, frame)
}
/// `PacketDispatch.sendRunScript(150, "IviiiIsssssssss", ...)` with the shop's arguments.
pub(crate) fn send_options_script<H: Host>(
    h: &mut H,
    p: &NodeHandle,
    options: [&str; 5],
    ints: [i32; 6],
) -> Result<()> {
    let args: Vec<ScriptArg> = ["", "", "", ""]
        .iter()
        .chain(options.iter())
        .map(|s| ScriptArg::Text(s.as_bytes().to_vec()))
        .chain(ints.iter().map(|i| ScriptArg::Int(*i)))
        .collect();
    let m = h.interface_manager(p)?;
    let frame = protocol(updates::run_client_script(
        150,
        b"IviiiIsssssssss",
        &args,
        m.packet_count(1),
    ))?;
    h.send(p, frame)
}
/// `PacketDispatch.sendInterfaceConfig(interface, child, hide)`.
fn send_interface_config<H: Host>(
    h: &mut H,
    p: &NodeHandle,
    interface: i32,
    child: i32,
    hidden: bool,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = updates::interface_hidden(
        hidden,
        m.packet_count(1),
        interface.wrapping_shl(16) | child,
    );
    h.send(p, frame)
}

/// `listenerInstances[uid]`.
fn listener_of<H: Host>(h: &mut H, p: &NodeHandle) -> Result<Option<Listener>> {
    let uid = h.uid(p)?;
    Ok(h.world()?.listener(uid))
}
fn personalized<H: Host>(h: &mut H) -> Result<bool> {
    h.config_bool(PERSONALIZED_SHOPS, false)
}
fn shared_key<H: Host>(h: &mut H) -> Result<i32> {
    Ok(java_hash(&h.server_name()?))
}

// ---------------------------------------------------------------- ShopListener

/// `ShopListener.view(item)`.
pub fn view<H: Host>(h: &mut H, listener: &ShopListener, item: &Item) -> Result<Option<Item>> {
    match &listener.view {
        None => Ok(Some(item.clone())),
        Some(view) => h.view(&listener.player, item, view),
    }
}
/// `ShopListener.update(c, event)`: a ContainerPacket of the viewed event items
/// for the event's slots, to -1/-1 container 92.
pub fn listener_update<H: Host>(
    h: &mut H,
    listener: &ShopListener,
    event: &crate::inventory::container::Event,
    null_item: &Item,
) -> Result<()> {
    let flagged = event.read().items.read().clone();
    let mut viewed = Vec::with_capacity(flagged.len());
    for item in &flagged {
        viewed.push(match item {
            Some(item) => view(h, listener, item)?,
            None => None,
        });
    }
    let slots = event.read().slots();
    let values: Vec<(i32, Option<WireItem>)> = slots
        .iter()
        .map(|&slot| {
            let item = viewed[slot].as_ref().filter(|item| !item.same(null_item));
            (slot as i32, item.map(wire))
        })
        .collect();
    let frame = protocol(updates::inventory_partial(-1, 92, &values))?;
    h.send(&listener.player, frame)
}
/// `ShopListener.refresh(c)`: the whole viewed container.
pub fn listener_refresh<H: Host>(
    h: &mut H,
    listener: &ShopListener,
    container: &Container,
) -> Result<()> {
    let items = container.items().read().clone();
    let mut viewed = Vec::with_capacity(items.len());
    for item in &items {
        viewed.push(match item {
            Some(item) => view(h, listener, item)?.as_ref().map(wire),
            None => None,
        });
    }
    let frame = protocol(updates::inventory_full(-1, 92, &viewed))?;
    h.send(&listener.player, frame)
}

// ------------------------------------------------------------------------ Shop

/// `Shop(title, stock, general, currency, highAlch, forceShared)`: the
/// properties in their order, then `init`.
pub fn new_shop<H: Host>(
    h: &mut H,
    title: String,
    stock: Vec<ShopItem>,
    general: bool,
    currency: i32,
    high_alch: bool,
    force_shared: bool,
) -> Result<Shop> {
    let player_stock = if general {
        h.world()?.general_player_stock()
    } else {
        h.factory()?.create(40, ContainerType::Shop, SortType::Id)?
    };
    let shop = Shop(Arc::new(Mutex::new(Fields {
        title,
        stock,
        general,
        currency,
        high_alch,
        force_shared,
        stock_instances: JavaHashMap::new(),
        player_stock,
        needs_update: JavaHashMap::new(),
        restock_rates: JavaHashMap::new(),
    })));
    if !personalized(h)? || force_shared {
        let key = shared_key(h)?;
        let container = generate_stock_container(h, &shop)?;
        shop.put_instance(key, container);
    }
    Ok(shop)
}

/// `generateStockContainer()`.
pub fn generate_stock_container<H: Host>(h: &mut H, shop: &Shop) -> Result<Container> {
    let mut container = h.factory()?.create(40, ContainerType::Shop, SortType::Id)?;
    let definitions = h.definitions()?;
    for item in shop.stock() {
        let added = new_item(item.item_id, item.amount, &definitions);
        container.add(Some(&added), true, -1, &definitions, &mut Items { host: h })?;
        shop.fields()
            .restock_rates
            .put(item.item_id, item.restock_rate);
    }
    Ok(container)
}

/// `openFor(player, itemView)`.
pub fn open_for<H: Host>(h: &mut H, shop: &Shop, p: &NodeHandle, view: View) -> Result<()> {
    let mut cont = get_container(h, shop, p)?;
    if let Some(listener) = listener_of(h, p)? {
        cont.unregister(Some(listener));
    }
    let uid = h.uid(p)?;
    h.world()?.set_listener(
        uid,
        Listener::new(ShopListener {
            player: p.clone(),
            view,
        }),
    );
    let title = shop.title();
    send_string(h, p, &title, 620, 22)?;
    h.set_shop_attribute(p, shop.clone())?;
    h.set_shop_container(p, cont)?;
    let m = h.interface_manager(p)?;
    let main = h.ui_component(p, 620)?;
    interfaces::open(&mut Ui::new(h, p), &m, Some(main))?;
    let m = h.interface_manager(p)?;
    let side = h.ui_component(p, 621)?;
    interfaces::open_single_tab(&mut Ui::new(h, p), &m, &side)?;
    show_tab(h, shop, p, true)?;
    let name = h.username(p)?;
    h.log(
        LogLevel::Fine,
        &format!("[SHOPS] Opening shop [Title: {title}, Player: {name}]"),
    )
}

/// `showTab(player, main)`.
pub fn show_tab<H: Host>(h: &mut H, shop: &Shop, p: &NodeHandle, main: bool) -> Result<()> {
    let Some(mut main_cont) = h.shop_container(p)? else {
        return Ok(());
    };
    let mut player_stock = shop.player_stock();
    let cont = if main {
        main_cont.clone()
    } else {
        player_stock.clone()
    };
    if !main {
        main_cont.unregister(listener_of(h, p)?);
        player_stock.register(listener_of(h, p)?);
    } else {
        player_stock.unregister(listener_of(h, p)?);
        main_cont.register(listener_of(h, p)?);
    }
    // IfaceSettingsBuilder().enableOptions(0..9).build()
    let settings = 2046;
    let child = if main { 23 } else { 24 };
    send_iface_settings(h, p, settings, child, 620, 0, cont.capacity())?;
    send_options_script(
        h,
        p,
        ["Buy X", "Buy 10", "Buy 5", "Buy 1", "Value"],
        [-1, 0, 4, 10, 92, (620 << 16) | child],
    )?;
    send_interface_config(h, p, 620, 23, !main)?;
    send_interface_config(h, p, 620, 24, main)?;
    send_interface_config(h, p, 620, 29, !main)?;
    send_interface_config(h, p, 620, 25, main)?;
    send_interface_config(h, p, 620, 27, main)?;
    send_interface_config(h, p, 620, 26, false)?;
    let definitions = h.definitions()?;
    if !main {
        player_stock.refresh(None, &definitions, &mut Items { host: h })?;
    } else {
        let mut cont = cont;
        cont.refresh(None, &definitions, &mut Items { host: h })?;
    }
    h.set_shop_main(p, main)
}

/// `getContainer(player)`.
pub fn get_container<H: Host>(h: &mut H, shop: &Shop, p: &NodeHandle) -> Result<Container> {
    let mut container = if personalized(h)? && !shop.force_shared() {
        let uid = h.uid(p)?;
        match shop.instance(uid) {
            Some(c) => c,
            None => {
                let c = generate_stock_container(h, shop)?;
                let uid = h.uid(p)?;
                shop.put_instance(uid, c.clone());
                c
            }
        }
    } else {
        let key = shared_key(h)?;
        match shop.instance(key) {
            Some(c) => c,
            None => {
                let c = generate_stock_container(h, shop)?;
                let key = shared_key(h)?;
                shop.put_instance(key, c.clone());
                c
            }
        }
    };
    let listener = listener_of(h, p)?;
    let owner = listener
        .as_ref()
        .map(|l| l.value::<ShopListener>().map(|s| s.player.clone()));
    if let Some(Some(owner)) = &owner
        && owner != p
    {
        container.unregister(listener.clone());
    }
    let replace = match &owner {
        None => true,
        Some(Some(owner)) => owner != p,
        Some(None) => return Err(Error::ClassCast),
    };
    if replace {
        let uid = h.uid(p)?;
        h.world()?.set_listener(
            uid,
            Listener::new(ShopListener {
                player: p.clone(),
                view: None,
            }),
        );
    }
    Ok(container)
}

/// `getVisibleStockItem(player, slot)`.
pub fn get_visible_stock_item<H: Host>(
    h: &mut H,
    p: &NodeHandle,
    slot: i32,
) -> Result<Option<Item>> {
    let Some(container) = h.shop_container(p)? else {
        return Ok(None);
    };
    let Some(item) = container.get(slot) else {
        return Ok(None);
    };
    let Some(listener) = listener_of(h, p)? else {
        return Ok(Some(item));
    };
    let listener = listener.value::<ShopListener>().ok_or(Error::ClassCast)?;
    view(h, &listener, &item)
}

/// `restock()`.
pub fn restock<H: Host>(h: &mut H, shop: &Shop) -> Result<()> {
    // stockInstances.filter { needsUpdate[it.key] == true }: a new map in iteration order.
    let entries: Vec<(i32, Container)> = {
        let f = shop.fields();
        f.stock_instances
            .iter()
            .filter(|(k, _)| f.needs_update.get(k) == Some(&true))
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    };
    let definitions = h.definitions()?;
    for (player, mut cont) in entries {
        for i in 0..cont.capacity() {
            if cont.get(i).is_none() {
                continue;
            }
            if shop.stock_len() < (i as usize) + 1 {
                break;
            }
            let stock = shop.stock_item(i)?;
            if stock.restock_rate == 0 {
                continue;
            }
            if h.ticks()?.wrapping_rem(stock.restock_rate) != 0 {
                continue;
            }
            let item = cont.get(i).ok_or(Error::Null)?;
            if amount(&item) < stock.amount {
                set_amount(&item, amount(&item).wrapping_add(1));
                cont.event().read().flag(i, cont.get(i))?;
            } else if amount(&item) > stock.amount {
                set_amount(&item, amount(&item).wrapping_sub(1));
                cont.event().read().flag(i, cont.get(i))?;
            }
            if amount(&cont.get(i).ok_or(Error::Null)?) != stock.amount {
                shop.set_needs_update(player);
            }
        }
        cont.update(None, &definitions, &mut Items { host: h })?;
    }
    Ok(())
}

/// `getBuyPrice(player, slot)`.
pub fn get_buy_price<H: Host>(h: &mut H, shop: &Shop, p: &NodeHandle, slot: i32) -> Result<Item> {
    let definitions = h.definitions()?;
    let is_main = h.shop_main(p, true)?;
    let cont = if is_main {
        match h.shop_container(p)? {
            Some(c) => c,
            None => return Ok(new_item(-1, -1, &definitions)),
        }
    } else {
        shop.player_stock()
    };
    let Some(item) = cont.get(slot) else {
        h.message(
            p,
            "That item doesn't appear to be there anymore. Please try again.",
        )?;
        return Ok(new_item(-1, -1, &definitions));
    };
    let visible = if is_main {
        get_visible_stock_item(h, p, slot)?
    } else {
        Some(item.clone())
    };
    let Some(visible) = visible else {
        h.message(
            p,
            "That item doesn't appear to be there anymore. Please try again.",
        )?;
        return Ok(new_item(-1, -1, &definitions));
    };
    let currency = shop.currency();
    let price = match currency {
        6529 => config_int(&definition(&visible)?, "tokkul_price")?,
        1464 => config_int(&definition(&visible)?, "archery_ticket_price")?,
        4067 => config_int(&definition(&visible)?, "castle_wars_ticket_price")?,
        _ => match fixed_price(id(&visible)) {
            Some(price) => price,
            None => {
                let unit = new_item(id(&visible), 1, &definitions);
                let stock_amount = if is_main {
                    shop.stock_item(item.read().index)?.amount
                } else {
                    amount(&shop.player_stock().get(slot).ok_or(Error::Null)?)
                };
                let current = if is_main {
                    amount(&item)
                } else {
                    amount(&shop.player_stock().get(slot).ok_or(Error::Null)?)
                };
                get_gp_cost(&unit, stock_amount, current)?
            }
        },
    };
    Ok(new_item(currency, price, &definitions))
}

/// `getSellPrice(player, slot)`: the container the item would go to and the price.
pub fn get_sell_price<H: Host>(
    h: &mut H,
    shop: &Shop,
    p: &NodeHandle,
    slot: i32,
) -> Result<(Option<Container>, Item)> {
    let definitions = h.definitions()?;
    let Some(shop_cont) = h.shop_container(p)? else {
        return Ok((None, new_item(-1, -1, &definitions)));
    };
    let Some(item) = h.inventory(p)?.get(slot) else {
        h.debug(
            p,
            &format!("Inventory slot {slot} does not contain an item!"),
        )?;
        h.message(
            p,
            "That item doesn't seem to be there anymore. Please try again.",
        )?;
        return Ok((None, new_item(-1, -1, &definitions)));
    };
    let shop_item_id = if definition(&item)?.read().unnoted {
        id(&item)
    } else {
        item.read().note_change()?
    };
    let (mut is_player_stock, shop_slot) = get_stock_slot(h, shop, shop_item_id)?;
    let stock_amt = if is_player_stock {
        0
    } else if shop_slot != -1 {
        shop.stock_item(shop_slot)?.amount
    } else {
        0
    };
    let current_amt = if is_player_stock {
        shop.player_stock().amount_id(shop_item_id, &definitions)
    } else if shop_slot != -1 {
        amount(&shop_cont.get(shop_slot).ok_or(Error::Null)?)
    } else {
        is_player_stock = true;
        0
    };
    let currency = shop.currency();
    let price = match currency {
        6529 => (f64::from(config_int(&definition(&item)?, "tokkul_price")?) / 10.0) as i32,
        1464 => config_int(&definition(&item)?, "archery_ticket_price")?,
        4067 => config_int(&definition(&item)?, "castle_wars_ticket_price")?,
        _ => match fixed_price(shop_item_id) {
            Some(price) => price,
            None => get_gp_sell(
                shop,
                &new_item(shop_item_id, 1, &definitions),
                stock_amt,
                current_amt,
                &definitions,
            )?,
        },
    };
    if !shop.general() && stock_amt == 0 && shop_slot == -1 {
        return Ok((None, new_item(-1, -1, &definitions)));
    }
    let container = if is_player_stock {
        shop.player_stock()
    } else {
        shop_cont
    };
    Ok((Some(container), new_item(currency, price, &definitions)))
}

/// `getGPCost(item, stockAmount, currentAmt)`.
pub fn get_gp_cost(item: &Item, stock_amount: i32, current_amt: i32) -> Result<i32> {
    let mut m = if stock_amount == 0 {
        100
    } else if current_amt == 0 {
        130
    } else if current_amt >= stock_amount {
        100
    } else {
        130i32.wrapping_sub(30i32.wrapping_mul(current_amt).wrapping_div(stock_amount))
    };
    if m < 1 {
        m = 1;
    }
    m = 100.max(130.min(m));
    let value = definition(item)?.read().value;
    let price = (f64::from(value) * f64::from(m) / 100.0).ceil() as i32;
    Ok(price.max(1))
}

/// `getGPSell(item, stockAmount, currentAmt)`.
pub fn get_gp_sell(
    shop: &Shop,
    item: &Item,
    stock_amount: i32,
    current_amt: i32,
    definitions: &Registry,
) -> Result<i32> {
    let def = definition(item)?;
    let base = items::definition_alchemy(&def, shop.high_alch(), definitions)?;
    let mut overstock = current_amt.wrapping_sub(stock_amount);
    if overstock < 0 {
        return Ok(base);
    }
    if overstock > 10 {
        overstock = 10;
    }
    let value = def.read().value;
    let price = round_to_int(f64::from(base) - (f64::from(value) * 0.03 * f64::from(overstock)))?;
    if price < 1 {
        return Ok(1);
    }
    Ok(price)
}

fn needs_update<H: Host>(h: &mut H, shop: &Shop, p: &NodeHandle) -> Result<()> {
    let key = if personalized(h)? {
        h.uid(p)?
    } else {
        shared_key(h)?
    };
    shop.set_needs_update(key);
    Ok(())
}

/// `buy(player, slot, amount)`.
pub fn buy<H: Host>(
    h: &mut H,
    shop: &Shop,
    p: &NodeHandle,
    slot: i32,
    amount_in: i32,
) -> Result<Status> {
    if amount_in < 1 {
        return Ok(Status::Failure(format!("Invalid amount: {amount_in}")));
    }
    let definitions = h.definitions()?;
    let is_main = h.shop_main(p, false)?;
    if !is_main && h.ironman(p)?.is_ironman() {
        h.dialogue(
            p,
            "As an ironman, you cannot buy from player stock in shops.",
        )?;
        return Ok(Status::Failure("Ironman buying from player stock".into()));
    }
    let mut cont = if is_main {
        match h.shop_container(p)? {
            Some(c) => c,
            None => return Ok(Status::Failure("Invalid shop-cont attr".into())),
        }
    } else {
        shop.player_stock()
    };
    let in_stock = cont.get(slot);
    let visible = if is_main {
        get_visible_stock_item(h, p, slot)?
    } else {
        in_stock.clone()
    };
    let Some(visible) = visible else {
        return Ok(Status::Failure(
            "Shop item is not available to this player".into(),
        ));
    };
    let item = new_item(id(&visible), amount_in, &definitions);
    let in_stock = in_stock.ok_or(Error::Null)?;
    if amount(&in_stock) < amount_in {
        set_amount(&item, amount(&in_stock));
    }
    let inventory = h.inventory(p)?;
    if amount(&item) > inventory.maximum_add(Some(&item))? {
        let max = inventory.maximum_add(Some(&item))?;
        set_amount(&item, max);
    }
    if amount(&in_stock) == 0 {
        h.content_message(p, "This item is out of stock.")?;
        return Ok(Status::Failure("Shop item out of stock.".into()));
    }
    if is_main
        && amount(&in_stock) > shop.stock_item(slot)?.amount
        && (!personalized(h)? || shop.force_shared())
        && h.ironman(p)?.is_ironman()
    {
        h.dialogue(
            p,
            "As an ironman, you cannot buy overstocked items from shops.",
        )?;
        return Ok(Status::Failure("Ironman overstock purchase".into()));
    }
    let cost = get_buy_price(h, shop, p, slot)?;
    if id(&cost) == -1 {
        h.content_message(p, "This shop cannot sell that item.")?;
        return Ok(Status::Failure("Shop cannot sell this item".into()));
    }
    if shop.currency() == 995 {
        if let Some(fixed) = fixed_price(id(&item)) {
            set_amount(&cost, fixed.wrapping_mul(amount(&item)));
        } else {
            let mut amt = amount(&item);
            let mut in_stock_amt = amount(&in_stock);
            loop {
                let before = amt;
                amt = amt.wrapping_sub(1);
                if before <= 1 {
                    break;
                }
                let unit = new_item(id(&item), 1, &definitions);
                let stock_amount = if is_main {
                    shop.stock_item(slot)?.amount
                } else {
                    amount(&shop.player_stock().get(slot).ok_or(Error::Null)?)
                };
                in_stock_amt = in_stock_amt.wrapping_sub(1);
                let add = get_gp_cost(&unit, stock_amount, in_stock_amt)?;
                set_amount(&cost, amount(&cost).wrapping_add(add));
            }
        }
    } else {
        set_amount(&cost, amount(&cost).wrapping_mul(amount(&item)));
    }
    let mut inventory = h.inventory(p)?;
    if inventory.contains(id(&cost), amount(&cost)) {
        if inventory.remove(
            Some(&cost),
            None,
            true,
            &definitions,
            &mut Items { host: h },
        )? {
            if amount(&item) == 0 {
                set_amount(&item, 1);
            }
            if !h.inventory(p)?.has_space_for(Some(&item))? {
                let back = new_item(id(&cost), amount(&cost), &definitions);
                h.inventory(p)?
                    .add(Some(&back), true, -1, &definitions, &mut Items { host: h })?;
                h.content_message(p, "You don't have enough inventory space to buy that many.")?;
                return Ok(Status::Failure("Not enough inventory space".into()));
            }
            let current = cont.get(slot).ok_or(Error::Null)?;
            if !is_main && amount(&current).wrapping_sub(amount(&item)) == 0 {
                let current = cont.get(slot);
                cont.remove(
                    current.as_ref(),
                    None,
                    false,
                    &definitions,
                    &mut Items { host: h },
                )?;
                cont.refresh(None, &definitions, &mut Items { host: h })?;
            } else {
                let current = cont.get(slot).ok_or(Error::Null)?;
                set_amount(&current, amount(&current).wrapping_sub(amount(&item)));
                cont.event().read().flag(slot, cont.get(slot))?;
                cont.update(None, &definitions, &mut Items { host: h })?;
            }
            let bought = new_item(id(&item), amount(&item), &definitions);
            h.inventory(p)?.add(
                Some(&bought),
                true,
                -1,
                &definitions,
                &mut Items { host: h },
            )?;
            needs_update(h, shop, p)?;
        }
    } else {
        let currency_name = lowercase(&name(&cost));
        h.content_message(
            p,
            &format!("You don't have enough {currency_name} to buy that many."),
        )?;
        return Ok(Status::Failure("Not enough money in inventory".into()));
    }
    h.shop_event(p, ShopEvent::Purchase, id(&item), amount(&item), &cost)?;
    Ok(Status::Success)
}

/// `sell(player, slot, amount)`.
pub fn sell<H: Host>(
    h: &mut H,
    shop: &Shop,
    p: &NodeHandle,
    slot: i32,
    amount_in: i32,
) -> Result<Status> {
    if amount_in < 1 {
        return Ok(Status::Failure(format!("Invalid amount: {amount_in}")));
    }
    let definitions = h.definitions()?;
    let player_inventory = h.inventory(p)?.get(slot).ok_or(Error::Null)?;
    let inventory_id = id(&player_inventory);
    if [995, 6529, 1464].contains(&inventory_id) {
        h.content_message(p, "You can't sell currency to a shop.")?;
        return Ok(Status::Failure(format!(
            "Tried to sell currency - {inventory_id}"
        )));
    }
    let item = new_item(inventory_id, amount_in, &definitions);
    let def = definitions.for_id(id(&item));
    if def.read().has_destroy_action() {
        h.content_message(p, "You can't sell this item.")?;
        return Ok(Status::Failure(format!(
            "Attempt to sell a destroyable - {inventory_id}."
        )));
    }
    if !items::definition_tradeable(&def)? {
        h.content_message(p, "You can't sell this item.")?;
        return Ok(Status::Failure(format!(
            "Attempt to sell an untradeable - {inventory_id}."
        )));
    }
    let (container, profit) = get_sell_price(h, shop, p, slot)?;
    if amount(&profit) == -1 {
        h.content_message(p, "This item can't be sold to this shop.")?;
        let general = shop.general();
        let text = profit.read().text();
        return Ok(Status::Failure(format!(
            "Can't sell this item to this shop - {inventory_id}, general: {general}, price: {text}"
        )));
    }
    let held = h.inventory(p)?.amount_id(id(&item), &definitions);
    if amount_in > held {
        let held = h.inventory(p)?.amount_id(id(&item), &definitions);
        set_amount(&item, held);
    }
    let sold_id = if !definition(&item)?.read().unnoted {
        item.read().note_change()?
    } else {
        id(&item)
    };
    let (is_player_stock, shop_slot) = get_stock_slot(h, shop, sold_id)?;
    if is_player_stock && shop_slot == -1 && h.world()?.general_player_stock().free_slots() == 0 {
        h.content_message(p, "The shop is too full to buy any more items")?;
        return Ok(Status::Failure("Attempt to sell to full shop.".into()));
    }
    if let Some(fixed) = fixed_price(sold_id) {
        set_amount(&profit, fixed.wrapping_mul(amount(&item)));
    } else if shop.currency() == 995 && amount(&item) > 1 {
        let mut amt = amount(&item);
        let in_container = container.as_ref().ok_or(Error::Null)?.get(shop_slot);
        let mut in_stock_amt = match in_container {
            Some(found) => amount(&found),
            None => shop.player_stock().amount_id(sold_id, &definitions),
        };
        loop {
            let before = amt;
            amt = amt.wrapping_sub(1);
            if before <= 1 {
                break;
            }
            let unit = new_item(id(&item), 1, &definitions);
            let stock_amount = if is_player_stock {
                0
            } else {
                shop.stock_item(shop_slot)?.amount
            };
            in_stock_amt = in_stock_amt.wrapping_add(1);
            let add = get_gp_sell(shop, &unit, stock_amount, in_stock_amt, &definitions)?;
            set_amount(&profit, amount(&profit).wrapping_add(add));
        }
    } else {
        set_amount(&profit, amount(&profit).wrapping_mul(amount(&item)));
    }
    let mut inventory = h.inventory(p)?;
    if inventory.remove(
        Some(&item),
        None,
        true,
        &definitions,
        &mut Items { host: h },
    )? {
        if !h.inventory(p)?.has_space_for(Some(&profit))? {
            h.content_message(p, "You don't have enough space to do that.")?;
            let back = new_item(id(&item), amount(&item), &definitions);
            h.inventory(p)?
                .add(Some(&back), true, -1, &definitions, &mut Items { host: h })?;
            return Ok(Status::Failure(
                "Did not have enough inventory space".into(),
            ));
        }
        let player_stock = shop.player_stock();
        let to_player_stock = container.as_ref().is_some_and(|c| c.same(&player_stock));
        if to_player_stock && h.shop_main(p, false)? {
            show_tab(h, shop, p, false)?;
        } else if !h.shop_main(p, false)? && !to_player_stock {
            show_tab(h, shop, p, true)?;
        }
        let paid = new_item(id(&profit), amount(&profit), &definitions);
        h.inventory(p)?
            .add(Some(&paid), true, -1, &definitions, &mut Items { host: h })?;
        if !definition(&item)?.read().unnoted {
            let change = item.read().note_change()?;
            item.write().set_id(change, &definitions);
        }
        if let Some(mut container) = container {
            container.add(Some(&item), true, -1, &definitions, &mut Items { host: h })?;
            container.refresh(None, &definitions, &mut Items { host: h })?;
        }
        needs_update(h, shop, p)?;
    }
    h.shop_event(p, ShopEvent::Sale, id(&item), amount(&item), &profit)?;
    Ok(Status::Success)
}

/// `getStockSlot(itemId)`: (isPlayerStock, slot); the last match wins.
pub fn get_stock_slot<H: Host>(h: &mut H, shop: &Shop, item_id: i32) -> Result<(bool, i32)> {
    let definitions = h.definitions()?;
    let mut shop_slot = -1;
    let mut is_player_stock = false;
    let notechange = definitions.for_id(item_id).read().note_id;
    for (stock_slot, shop_item) in shop.stock().iter().enumerate() {
        if shop_item.item_id == item_id || shop_item.item_id == notechange {
            shop_slot = stock_slot as i32;
        }
    }
    if shop_slot == -1 {
        let items = shop.player_stock().items().read().clone();
        for (stock_slot, player_stock_item) in items.iter().enumerate() {
            let Some(player_stock_item) = player_stock_item else {
                continue;
            };
            if id(player_stock_item) == item_id || id(player_stock_item) == notechange {
                shop_slot = stock_slot as i32;
                is_player_stock = true;
            }
        }
    }
    if shop_slot == -1 {
        is_player_stock = true;
    }
    Ok((is_player_stock, shop_slot))
}
