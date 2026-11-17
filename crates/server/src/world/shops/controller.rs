//! Shops.kt: the start-up listener (`startup`, `parseStock`), the tick
//! listener, the NPC "trade"/"shop" listeners, the destination override, the
//! interface listeners of 620 and 621 and `openId`, in the original order.
use super::{
    Error, Host, Location, Npc, Quest, Result, SideButtons, SideOpen, TemplateButtons,
    TemplateClose, lowercase,
    shop::{self, AmountScript, Shop, ShopItem},
    ui::{Items, Ui},
};
use crate::{
    inventory::item::Handle as Item,
    startup::loaders::{self, LoadError},
    world::{
        drop_tables::LogLevel,
        interface_listeners::{Button, Callback, Registry},
        interfaces,
        player_walk::NodeHandle,
    },
};
use rs09_game_core::java_json::Json;

fn log<H: Host>(h: &mut H, level: LogLevel, message: &str) -> Result<()> {
    h.log(level, message)
}
fn from_load(e: LoadError) -> Error {
    match e {
        LoadError::File(_) => Error::File,
        LoadError::Json(_) => Error::Parse,
        LoadError::Null => Error::Null,
        LoadError::Type => Error::ClassCast,
        LoadError::Number => Error::Number,
        LoadError::ListIndex => Error::ListBounds,
        LoadError::ArrayIndex => Error::Bounds,
        LoadError::Binding(text) => Error::Service(text),
    }
}

/// `Shops.parseStock(stock, id)`. A row that fails is logged and its exception
/// rethrown; rows before it stay in the list the caller never receives.
pub fn parse_stock<H: Host>(h: &mut H, stock: &str, id: i32) -> Result<Vec<ShopItem>> {
    let mut items: Vec<ShopItem> = Vec::new();
    let mut ids_in_stock = std::collections::HashSet::new();
    if stock.is_empty() {
        return Ok(items);
    }
    for part in stock.split('-') {
        match parse_row(h, part, id, &mut items, &mut ids_in_stock) {
            Ok(()) => {}
            Err(e) => {
                log(
                    h,
                    LogLevel::Warn,
                    &format!("[SHOPS] MALFORMED STOCK IN SHOP ID {id} FOR ITEM {part}"),
                )?;
                return Err(e);
            }
        }
    }
    Ok(items)
}
fn parse_row<H: Host>(
    h: &mut H,
    part: &str,
    id: i32,
    items: &mut Vec<ShopItem>,
    ids_in_stock: &mut std::collections::HashSet<i32>,
) -> Result<()> {
    let cleaned = part.replace(['{', '}'], "");
    let tokens: Vec<&str> = cleaned.split(',').collect();
    let mut amount = loaders::kotlin_trim(tokens.get(1).ok_or(Error::Bounds)?).to_string();
    if amount == "inf" {
        amount = "-1".into();
    }
    let item = loaders::int(tokens[0]).map_err(from_load)?;
    if ids_in_stock.contains(&item) {
        log(
            h,
            LogLevel::Warn,
            &format!("[SHOPS] MALFORMED STOCK IN SHOP ID {id} FOR ITEM {item}"),
        )?;
        for existing in items.iter_mut() {
            if existing.item_id == item {
                let add = loaders::int(&amount).map_err(from_load)?;
                existing.amount = existing.amount.wrapping_add(add);
                return Ok(());
            }
        }
    } else {
        let restock_rate = tokens
            .get(2)
            .and_then(|rate| rs09_game_core::java_string::parse_i32(rate))
            .unwrap_or(100);
        let amount = loaders::int(&amount).map_err(from_load)?;
        items.push(ShopItem {
            item_id: item,
            amount,
            restock_rate,
        });
        ids_in_stock.insert(item);
    }
    Ok(())
}

/// `Shops.startup()`: every row of `CONFIG_PATH + "shops.json"` becomes a Shop
/// in `shopsById` and, for each listed NPC, `shopsByNpc`.
pub fn startup<H: Host>(h: &mut H) -> Result<()> {
    let prefix = h.config_path()?;
    let path = format!("{}shops.json", prefix.as_deref().unwrap_or("null"));
    let mut shop_count = 0i32;
    log(
        h,
        LogLevel::Fine,
        &format!("[SHOPS] Using JSON path: {path}"),
    )?;
    let text = h.read_file(&path)?;
    let data = loaders::parse(&text).map_err(from_load)?;
    let rows = loaders::array(Some(&data)).map_err(from_load)?.to_vec();
    for raw in &rows {
        let row = loaders::object(Some(raw)).map_err(from_load)?;
        let id = loaders::int(&loaders::text(row, "id")).map_err(from_load)?;
        let title = loaders::text(row, "title");
        let general = loaders::text(row, "general_store").eq_ignore_ascii_case("true");
        let stock = parse_stock(h, &loaders::text(row, "stock"), id)?;
        let npcs_text = loaders::text(row, "npcs");
        let npcs = if loaders::kotlin_not_blank(&npcs_text) {
            let mut npcs = Vec::new();
            for part in npcs_text.split(',') {
                npcs.push(loaders::int(part).map_err(from_load)?);
            }
            npcs
        } else {
            Vec::new()
        };
        let currency = loaders::int(&loaders::text(row, "currency")).map_err(from_load)?;
        let high_alch = loaders::text(row, "high_alch") == "1";
        let force_shared = row
            .get("force_shared")
            .map_or_else(|| "false".to_string(), Json::text)
            .eq_ignore_ascii_case("true");
        let shop = shop::new_shop(h, title, stock, general, currency, high_alch, force_shared)?;
        let world = h.world()?;
        for npc in npcs {
            world.tables().shops_by_npc.put(npc, shop.clone());
        }
        world.tables().shops_by_id.put(id, shop);
        shop_count = shop_count.wrapping_add(1);
    }
    log(
        h,
        LogLevel::Fine,
        &format!("[SHOPS] Parsed {shop_count} shops."),
    )
}

/// `Shops.openId(player, id)`.
pub fn open_id<H: Host>(h: &mut H, p: &NodeHandle, id: i32) -> Result<()> {
    if let Some(shop) = h.world()?.shop_by_id(id) {
        shop::open_for(h, &shop, p, None)?;
    }
    Ok(())
}

/// `Shops.tick()`: every shop restocks (in `shopsById` order), then the
/// general player stock is offered to the Grand Exchange and cleared every
/// `secondsToTicks(PLAYER_STOCK_CLEAR_INTERVAL * 60)` ticks.
pub fn tick<H: Host>(h: &mut H) -> Result<()> {
    let shops: Vec<Shop> = h
        .world()?
        .tables()
        .shops_by_id
        .iter()
        .map(|(_, shop)| shop.clone())
        .collect();
    for shop in &shops {
        shop::restock(h, shop)?;
    }
    let minutes = h.stock_clear_interval()?;
    // secondsToTicks(seconds) = seconds * 1000 / 600, in Int arithmetic.
    let interval = minutes.wrapping_mul(60).wrapping_mul(1000) / 600;
    let ticks = h.ticks()?;
    if interval == 0 {
        return Err(Error::Arithmetic);
    }
    if ticks.wrapping_rem(interval) == 0 {
        let clear_to_ge = h.stock_recirculate()?;
        let mut general = h.world()?.general_player_stock();
        if clear_to_ge {
            let items: Vec<Item> = general.items().read().iter().flatten().cloned().collect();
            for item in items {
                let (id, amount) = {
                    let item = item.read();
                    (item.id(), item.amount())
                };
                h.ge_offer(id, amount)?;
            }
        }
        let definitions = h.definitions()?;
        general.clear(true, &definitions, &mut Items { host: h })?;
    }
    Ok(())
}

/// The NPC listeners Shops registers, in order: ids (`None` for the
/// option-only listener) and options.
pub const NPC_LISTENERS: [(Option<i32>, &[&str]); 4] = [
    (None, &["trade", "shop"]),
    (Some(933), &["trade"]),
    (Some(1316), &["trade"]),
    (Some(562), &["trade"]),
];
/// The destination override's options.
pub const DESTINATION_OPTIONS: [&str; 2] = ["trade", "shop"];
/// The two commands Shops defines (their bodies are not ported).
pub const COMMANDS: [(&str, &str); 2] = [("openshop", "ADMIN"), ("shopscript", "ADMIN")];

/// Which of the NPC listeners above handles an interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcListener {
    /// `on(IntType.NPC, "trade", "shop")`.
    Generic,
    /// `on(NPCs.SIEGFRIED_ERKLE_933, IntType.NPC, "trade")`.
    Siegfried,
    /// `on(NPCs.FUR_TRADER_1316, IntType.NPC, "trade")`.
    FurTrader,
    /// `on(NPCs.CANDLE_MAKER_562, IntType.NPC, "trade")`.
    CandleMaker,
}

/// The NPC listener bodies. Each returns the handler's Boolean.
pub fn npc_option<H: Host>(
    h: &mut H,
    which: NpcListener,
    p: &NodeHandle,
    npc: &Npc,
) -> Result<bool> {
    match which {
        NpcListener::Generic => {
            if npc.id == 2824 || npc.id == 1041 || npc.id == 804 {
                h.tanning(p, npc.id)?;
                return Ok(true);
            }
            if npc.id == 7601 {
                let m = h.interface_manager(p)?;
                let c = h.ui_component(p, 732)?;
                interfaces::open(&mut Ui::new(h, p), &m, Some(c))?;
                return Ok(true);
            }
            match h.world()?.shop_by_npc(npc.id) {
                Some(shop) => shop::open_for(h, &shop, p, None)?,
                None => return Ok(false),
            }
            Ok(true)
        }
        NpcListener::Siegfried => {
            let points = h.quest_points(p)?;
            if points < 40 {
                h.npc_dialogue(
                    p,
                    933,
                    "I'm sorry, adventurer, but you need 40 quest points to buy from me.",
                    "NEUTRAL",
                )?;
                return Ok(true);
            }
            if let Some(shop) = h.world()?.shop_by_npc(npc.id) {
                shop::open_for(h, &shop, p, None)?;
            }
            Ok(true)
        }
        NpcListener::FurTrader => {
            if !h.quest_complete(p, Quest::FremennikTrials)? {
                h.npc_dialogue(p, 1316, "I don't sell to outerlanders.", "ANNOYED")?;
            } else if let Some(shop) = h.world()?.shop_by_npc(npc.id) {
                shop::open_for(h, &shop, p, None)?;
            }
            Ok(true)
        }
        NpcListener::CandleMaker => {
            if h.quest_stage(p, Quest::MerlinsCrystal)? > 60 {
                open_id(h, p, 56)?;
            } else if let Some(shop) = h.world()?.shop_by_npc(npc.id) {
                shop::open_for(h, &shop, p, None)?;
            }
            Ok(true)
        }
    }
}

/// `setDest(IntType.NPC, "trade", "shop")`: two tiles in front of an NPC
/// facing a booth, else the NPC's own location.
pub fn destination<H: Host>(h: &mut H, npc: &Npc) -> Result<Location> {
    if h.npc_facing_booth(npc)? {
        let offset_x = npc.step.0.wrapping_shl(1);
        let offset_y = npc.step.1.wrapping_shl(1);
        let l = npc.location;
        // Location.transform(diffX, diffY, z): Location(x + diffX, y + diffY, (this.z + z) % 4)
        return Ok(Location {
            x: l.x.wrapping_add(offset_x),
            y: l.y.wrapping_add(offset_y),
            z: l.z.wrapping_add(0).wrapping_rem(4),
        });
    }
    Ok(npc.location)
}

/// `defineInterfaceListeners()`: `on(620)`, `onOpen(621)`, `onClose(620)`,
/// `on(621)` on the world's InterfaceListeners registry.
pub fn define_interface_listeners(registry: &Registry) -> interfaces::Result<()> {
    registry.add(620, None, Callback::button(TemplateButtons))?;
    registry.add_open(621, Callback::open(SideOpen))?;
    registry.add_close(620, Callback::open(TemplateClose))?;
    registry.add(621, None, Callback::button(SideButtons))
}

/// The `on(620)` lambda.
pub fn template_button<H: Host>(h: &mut H, p: &NodeHandle, button: Button) -> Result<bool> {
    let Some(shop) = h.shop_attribute(p)? else {
        return Ok(false);
    };
    let is_main = h.shop_main(p, true)?;
    match button.button {
        26 => {
            shop::show_tab(h, &shop, p, false)?;
            return Ok(true);
        }
        25 => {
            shop::show_tab(h, &shop, p, true)?;
            return Ok(true);
        }
        27 | 29 => return Ok(true),
        _ => {}
    }
    let slot = button.slot;
    let price = shop::get_buy_price(h, &shop, p, slot)?;
    let visible = if is_main {
        shop::get_visible_stock_item(h, p, slot)?
    } else {
        shop.player_stock().get(slot)
    };
    match button.opcode {
        155 => {
            if let Some(visible) = visible {
                let price_id = price.read().id();
                if price_id != -1 {
                    let definitions = h.definitions()?;
                    let name = item_name(&definitions, visible.read().id());
                    let (amount, currency) = {
                        let price = price.read();
                        (
                            price.amount(),
                            price.name.clone().unwrap_or_else(|| "null".into()),
                        )
                    };
                    let currency = lowercase(&currency);
                    h.content_message(
                        p,
                        &format!("{name}: This item currently costs {amount} {currency}."),
                    )?;
                }
            }
        }
        196 => {
            shop::buy(h, &shop, p, slot, 1)?;
        }
        124 => {
            shop::buy(h, &shop, p, slot, 5)?;
        }
        199 => {
            shop::buy(h, &shop, p, slot, 10)?;
        }
        234 => {
            let script = AmountScript {
                shop: shop.clone(),
                player: p.clone(),
                slot,
                buy: true,
            };
            h.input_dialogue(p, "Enter the amount to buy:", script)?;
        }
        9 => {
            if let Some(visible) = visible {
                let id = visible.read().id();
                let text = h.examine(id)?;
                h.content_message(p, &text)?;
            }
        }
        _ => {}
    }
    Ok(true)
}

/// `getItemName(id)`: `ItemDefinition.forId(id).name`.
fn item_name(definitions: &crate::definitions::item::Registry, id: i32) -> String {
    definitions
        .for_id(id)
        .read()
        .name
        .clone()
        .unwrap_or_else(|| "null".into())
}

/// The `onOpen(621)` lambda: the side interface's settings and options.
pub fn side_open<H: Host>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    // IfaceSettingsBuilder().enableOptions(0 until 9).build()
    shop::send_iface_settings(h, p, 1022, 0, 621, 0, 28)?;
    shop::send_options_script(
        h,
        p,
        ["Sell X", "Sell 10", "Sell 5", "Sell 1", "Value"],
        [-1, 0, 7, 4, 93, 621 << 16],
    )?;
    Ok(true)
}

/// The `onClose(620)` lambda: the player's listener leaves the stock and the
/// player stock, then the single tab closes.
pub fn template_close<H: Host>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    let Some(shop) = h.shop_attribute(p)? else {
        return Ok(true);
    };
    let uid = h.uid(p)?;
    let Some(listener) = h.world()?.listener(uid) else {
        return Ok(true);
    };
    if h.config_bool(super::PERSONALIZED_SHOPS, false)? {
        let uid = h.uid(p)?;
        if let Some(mut c) = shop.instance(uid) {
            c.unregister(Some(listener.clone()));
        }
    } else {
        let key = super::java_hash(&h.server_name()?);
        let mut c = shop.instance(key).ok_or(Error::Null)?;
        c.unregister(Some(listener.clone()));
    }
    shop.player_stock().unregister(Some(listener));
    let m = h.interface_manager(p)?;
    interfaces::close_single_tab(&mut Ui::new(h, p), &m)?;
    Ok(true)
}

/// The `on(621)` lambda.
pub fn side_button<H: Host>(h: &mut H, p: &NodeHandle, button: Button) -> Result<bool> {
    let Some(shop) = h.shop_attribute(p)? else {
        return Ok(false);
    };
    let slot = button.slot;
    if h.inventory(p)?.get(slot).is_none() {
        h.message(
            p,
            "That item doesn't appear to be there anymore. Please try again.",
        )?;
        return Ok(true);
    }
    let (_, price) = shop::get_sell_price(h, &shop, p, slot)?;
    let definitions = h.definitions()?;
    let in_slot = h.inventory(p)?.get(slot).ok_or(Error::Null)?;
    let def = definitions.for_id(in_slot.read().id());
    let (price_id, price_amount, price_name) = {
        let price = price.read();
        (
            price.id(),
            price.amount(),
            price.name.clone().unwrap_or_else(|| "null".into()),
        )
    };
    let refused = price_amount == -1
        || !shop::has_shop_currency_value(&def, price_id)?
        || [995, 6529, 1464, 4067].contains(&def.read().id);
    let value_message = if refused {
        "This shop will not buy that item.".to_string()
    } else {
        let item = h.inventory(p)?.get(slot).ok_or(Error::Null)?;
        let name = item.read().name.clone().unwrap_or_else(|| "null".into());
        format!(
            "{name}: This shop will buy this item for {price_amount} {}.",
            lowercase(&price_name)
        )
    };
    match button.opcode {
        155 => h.content_message(p, &value_message)?,
        196 => {
            shop::sell(h, &shop, p, slot, 1)?;
        }
        124 => {
            shop::sell(h, &shop, p, slot, 5)?;
        }
        199 => {
            shop::sell(h, &shop, p, slot, 10)?;
        }
        234 => {
            let script = AmountScript {
                shop: shop.clone(),
                player: p.clone(),
                slot,
                buy: false,
            };
            h.input_dialogue(p, "Enter the amount to sell:", script)?;
        }
        _ => {}
    }
    Ok(true)
}
