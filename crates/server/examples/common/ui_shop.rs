use super::super::{Location, combat_services, now, npc_services, script_services, skilling};
use super::{Context, Error, Factory, Node, Probe, Result, UiContext, inventory_binding, item, ui};
use rs09_protocol::game::{
    actions::ClientAction,
    updates::{self, ScriptArg, WireItem},
};
use rs09_server::{
    definitions::{item::Shared, property::Property},
    inventory::item::Item,
    player::{attributes::AttributeObject, owned::Player},
    world::{action_locks::Kind, interface_listeners::Button, prepared_masks::FaceTarget},
};
use std::collections::{BTreeMap, BTreeSet};

const MAIN: i32 = 620;
const SIDE: i32 = 621;
const COINS: i32 = 995;
const SLOTS: usize = 40;
const ITEM_OPTIONS: i32 = 2046;
const ABSENT: &str = "That item doesn't appear to be there anymore. Please try again.";
type Stock = [Option<WireItem>; SLOTS];

#[derive(serde::Deserialize)]
struct Config<'a> {
    id: &'a str,
    title: String,
    npcs: &'a str,
    stock: &'a str,
    currency: &'a str,
    high_alch: &'a str,
    general_store: &'a str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct View {
    id: i32,
    main: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Route {
    npc: Node,
    shop: i32,
    npc_type: i32,
    destination: Option<[i32; 3]>,
}

struct Shop {
    title: String,
    currency: i32,
    general: bool,
    high_alch: bool,
    main: Stock,
    baseline: [Option<(i32, i32)>; SLOTS],
    resale: Stock,
}

pub(super) struct Catalog {
    shops: BTreeMap<i32, Shop>,
    merchants: BTreeMap<i32, i32>,
    general_stock: Stock,
    restoring: BTreeSet<(i32, usize)>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            shops: BTreeMap::new(),
            merchants: BTreeMap::new(),
            general_stock: [None; SLOTS],
            restoring: BTreeSet::new(),
        }
    }
}

impl Catalog {
    pub(super) fn load(data: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let mut catalog = Self::default();
        for row in serde_json::from_str::<Vec<Config>>(data)? {
            let id = row.id.parse()?;
            let mut main = [None::<WireItem>; SLOTS];
            let mut baseline = [None::<(i32, i32)>; SLOTS];
            for entry in row.stock.split('-').filter(|entry| !entry.is_empty()) {
                let mut fields = entry.trim_matches(['{', '}']).split(',').map(str::trim);
                let item = fields.next().ok_or("missing shop item ID")?.parse()?;
                let amount = match fields.next().ok_or("missing shop item quantity")? {
                    "inf" => -1,
                    amount => amount.parse()?,
                };
                let interval = fields.next().and_then(|s| s.parse().ok()).unwrap_or(100);
                let Some(slot) = main
                    .iter()
                    .position(|slot| slot.is_none_or(|it| it.id == item))
                else {
                    continue; // Original main inventory holds forty slots.
                };
                let amount = main[slot].map_or(amount, |it| it.amount + amount);
                main[slot] = Some(WireItem { id: item, amount });
                baseline[slot] = Some((amount, baseline[slot].map_or(interval, |(_, rate)| rate)));
            }
            for npc in row.npcs.split(',').filter(|part| !part.trim().is_empty()) {
                catalog.merchants.insert(npc.trim().parse()?, id);
            }
            catalog.shops.insert(
                id,
                Shop {
                    title: row.title,
                    currency: row.currency.parse()?,
                    general: row.general_store == "true",
                    high_alch: row.high_alch == "1",
                    main,
                    baseline,
                    resale: [None; SLOTS],
                },
            );
        }
        Ok(catalog)
    }

    fn stock(&self, view: View) -> &Stock {
        let shop = &self.shops[&view.id];
        if view.main {
            &shop.main
        } else if shop.general {
            &self.general_stock
        } else {
            &shop.resale
        }
    }

    fn stock_mut(&mut self, view: View) -> &mut Stock {
        let shop = self.shops.get_mut(&view.id).expect("active shop");
        if view.main {
            &mut shop.main
        } else if shop.general {
            &mut self.general_stock
        } else {
            &mut shop.resale
        }
    }

    fn notify(
        &self,
        players: &BTreeMap<Node, super::Player>,
        frames: &mut BTreeMap<Node, Vec<super::Frame>>,
        view: View,
    ) -> Result<()> {
        let frame = updates::inventory_full(-1, 92, self.stock(view)).map_err(binding)?;
        let shared = !view.main && self.shops[&view.id].general;
        for (&node, row) in players {
            if row.shop_view.is_some_and(|open| {
                open == view || shared && !open.main && self.shops[&open.id].general
            }) {
                frames.entry(node).or_default().push(frame.clone());
            }
        }
        Ok(())
    }
}

fn binding(error: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("shop: {error:?}"))
}

pub(in super::super) fn process(
    p: &mut Probe,
    index: i32,
    player: Node,
    action: &ClientAction,
) -> std::result::Result<bool, String> {
    let Some(store) = p.host.ui.as_mut() else {
        return Ok(false);
    };
    let Some(row) = store.players.get_mut(&player) else {
        return Ok(false);
    };
    let ClientAction::NpcAction {
        opt_index,
        npc_index,
    } = *action
    else {
        if matches!(
            action,
            ClientAction::WorldspaceWalk { .. }
                | ClientAction::MinimapWalk { .. }
                | ClientAction::SceneryAction { .. }
                | ClientAction::GroundItemAction { .. }
                | ClientAction::CloseIface
        ) && row.shop_route.take().is_some()
        {
            p.state
                .actor(player)
                .map_err(|e| e.to_string())?
                .queue
                .points()
                .write()
                .clear();
        }
        if row.shop_view.is_some()
            && matches!(
                action,
                ClientAction::WorldspaceWalk { .. } | ClientAction::MinimapWalk { .. }
            )
        {
            let manager = row.manager.clone();
            ui::close(&mut UiContext::new(p, player), &manager).map_err(|e| e.to_string())?;
        }
        return Ok(false);
    };
    if row.shop_route.take().is_some() {
        p.state
            .actor(player)
            .map_err(|e| e.to_string())?
            .queue
            .points()
            .write()
            .clear();
    }
    let Some(npc) = npc_services::visible(p, index, npc_index) else {
        return Ok(false);
    };
    let Some(npc_type) = npc_services::option_type(p, npc, opt_index, &["trade", "shop"]) else {
        return Ok(false);
    };
    let Some(&shop) = p.host.ui.as_ref().unwrap().shops.merchants.get(&npc_type) else {
        return Ok(false);
    };
    if script_services::locked(&mut p.state, player, Kind::Interaction)
        .map_err(|e| e.to_string())?
    {
        return Ok(true);
    }
    let m = p.host.ui.as_ref().unwrap().players[&player].manager.clone();
    if !ui::close(&mut UiContext::new(p, player), &m).map_err(|e| e.to_string())? {
        return Ok(true);
    }
    combat_services::cancel(p, player)?;
    skilling::cancel(p, player);
    let actor = p.state.actor(player).map_err(|e| e.to_string())?;
    let manager = actor.manager.clone();
    actor.queue.points().write().clear();
    p.state
        .context(&mut p.host)
        .clear_standard(player, manager)
        .map_err(|e| e.to_string())?;
    p.host
        .ui
        .as_mut()
        .unwrap()
        .players
        .get_mut(&player)
        .unwrap()
        .shop_route = Some(Route {
        npc,
        shop,
        npc_type,
        destination: None,
    });
    Ok(true)
}

fn clear_route(p: &mut Probe, player: Node) {
    if let Some(row) = p.host.ui.as_mut().and_then(|s| s.players.get_mut(&player)) {
        row.shop_route = None;
    }
    if let Ok(actor) = p.state.actor(player) {
        actor.queue.points().write().clear();
    }
}

fn position(p: &mut Probe, player: Node) -> Option<(Route, Location, Location, i32)> {
    let route = p.host.ui.as_ref()?.players.get(&player)?.shop_route?;
    let Some((from, to, size)) = (|| {
        Some((
            p.state.node_location(player).ok()??,
            p.state.node_location(route.npc).ok()??,
            p.state.node_size(route.npc).ok()?.max(1),
        ))
    })() else {
        clear_route(p, player);
        return None;
    };
    let [x, y, z] = from.coordinates();
    let [tx, ty, tz] = to.coordinates();
    if z != tz || (x - tx).abs() > 15 || (y - ty).abs() > 15 {
        clear_route(p, player);
        return None;
    }
    Some((route, from, to, size))
}

pub(in super::super) fn approach(p: &mut Probe, player: Node) -> std::result::Result<(), String> {
    let Some((route, from, to, size)) = position(p, player) else {
        return Ok(());
    };
    let destination = to.coordinates();
    if route.destination == Some(destination) || combat_services::in_reach(p, &from, &to, size, 1)?
    {
        return Ok(());
    }
    let path = combat_services::approach_path(p, player, route.npc, &from, &to, size, 1)?;
    p.state
        .actor(player)
        .map_err(|e| e.to_string())?
        .queue
        .points()
        .write()
        .clear();
    if !path.points.is_empty() {
        p.state
            .context(&mut p.host)
            .walk_path(player, path)
            .map_err(|e| e.to_string())?;
    }
    p.host
        .ui
        .as_mut()
        .unwrap()
        .players
        .get_mut(&player)
        .unwrap()
        .shop_route
        .as_mut()
        .unwrap()
        .destination = Some(destination);
    Ok(())
}

pub(in super::super) fn arrive(p: &mut Probe, player: Node) -> std::result::Result<(), String> {
    let Some((route, from, to, size)) = position(p, player) else {
        return Ok(());
    };
    if !combat_services::in_reach(p, &from, &to, size, 1)? {
        if route.destination == Some(to.coordinates())
            && !p
                .state
                .actor(player)
                .map_err(|e| e.to_string())?
                .queue
                .has_path()
        {
            clear_route(p, player);
            combat_services::message(p, player, "I can't reach that.");
        }
        return Ok(());
    }
    clear_route(p, player);
    if script_services::locked(&mut p.state, player, Kind::Interaction)
        .map_err(|e| e.to_string())?
    {
        return Ok(());
    }
    let index = p.state.actor(player).map_err(|e| e.to_string())?.wire_index;
    let npc = p.state.retain_node(route.npc).map_err(|e| e.to_string())?;
    let view = npc_services::view(&p.host, route.npc).ok_or("merchant retired")?;
    p.host.masks[&player]
        .face_entity(Some(FaceTarget::Npc(npc, view)))
        .map_err(|e| e.to_string())?;
    npc_services::face_player(p, route.npc, index);
    match route.npc_type {
        933 => combat_services::message(
            p,
            player,
            "I'm sorry, adventurer, but you need 40 quest points to buy from me.",
        ),
        1316 => combat_services::message(p, player, "I don't sell to outerlanders."),
        _ => open(&mut UiContext::new(p, player), route.shop).map_err(|e| e.to_string())?,
    }
    Ok(())
}

fn catalog<'a>(ctx: &'a UiContext<'_>) -> Result<&'a Catalog> {
    Ok(&ctx.host.ui.as_ref().ok_or(Error::Null)?.shops)
}

fn active(ctx: &UiContext<'_>) -> Result<Option<View>> {
    let row = ctx.row()?;
    Ok(row
        .shop_view
        .filter(|_| ui::has_main_component(&row.manager, MAIN)))
}

fn open(ctx: &mut UiContext<'_>, id: i32) -> Result<()> {
    let title = catalog(ctx)?.shops[&id].title.clone();
    let m = ctx.row()?.manager.clone();
    ctx.send(
        updates::interface_text(MAIN << 16 | 22, title.as_bytes(), m.packet_count(1))
            .map_err(binding)?,
    )?;
    if ui::open_id(ctx, &m, MAIN)?.is_none() {
        return Ok(());
    }
    let side = ctx.component(SIDE)?;
    ui::open_single_tab(ctx, &m, &side)?;
    slots(ctx, SIDE << 16, 93, 4, 7, "Sell")?;
    let stock: Vec<_> = ctx
        .row()?
        .inventory()
        .items()
        .read()
        .iter()
        .map(|item| {
            item.as_ref().map(|item| {
                let item = item.read();
                WireItem {
                    id: item.id(),
                    amount: item.amount(),
                }
            })
        })
        .collect();
    ctx.send(updates::inventory_full(-1, 93, &stock).map_err(binding)?)?;
    show(ctx, View { id, main: true })
}

pub(super) fn close(ctx: &mut UiContext<'_>) -> Result<()> {
    if ctx.row_mut()?.shop_view.take().is_some() {
        let m = ctx.row()?.manager.clone();
        ui::close_single_tab(ctx, &m)?;
    }
    Ok(())
}

fn slots(
    ctx: &mut UiContext<'_>,
    component: i32,
    container: i32,
    columns: i32,
    rows: i32,
    verb: &str,
) -> Result<()> {
    let m = ctx.row()?.manager.clone();
    ctx.send(updates::interface_events(
        component,
        0,
        rows * columns,
        ITEM_OPTIONS,
        m.packet_count(1),
    ))?;
    let mut args = [
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        format!("{verb} X"),
        format!("{verb} 10"),
        format!("{verb} 5"),
        format!("{verb} 1"),
        "Value".into(),
    ]
    .into_iter()
    .map(|s| ScriptArg::Text(s.into_bytes()))
    .collect::<Vec<_>>();
    args.extend(
        [-1, 0, rows, columns, container, component]
            .into_iter()
            .map(ScriptArg::Int),
    );
    ctx.send(
        updates::run_client_script(150, b"IviiiIsssssssss", &args, m.packet_count(1))
            .map_err(binding)?,
    )
}

fn show(ctx: &mut UiContext<'_>, view: View) -> Result<()> {
    let m = ctx.row()?.manager.clone();
    let child = if view.main { 23 } else { 24 };
    slots(ctx, MAIN << 16 | child, 92, 10, 4, "Buy")?;
    for (child, hidden) in [
        (23, !view.main),
        (24, view.main),
        (29, !view.main),
        (25, view.main),
        (27, view.main),
        (26, false),
    ] {
        ctx.send(updates::interface_hidden(
            hidden,
            m.packet_count(1),
            MAIN << 16 | child,
        ))?;
    }
    let packet = updates::inventory_full(-1, 92, catalog(ctx)?.stock(view)).map_err(binding)?;
    ctx.send(packet)?;
    ctx.row_mut()?.shop_view = Some(view);
    Ok(())
}

fn feedback(ctx: &mut UiContext<'_>, text: &str) -> Result<()> {
    let mut payload = text.as_bytes().to_vec();
    payload.push(0);
    ctx.send(super::Frame {
        opcode: 70,
        payload,
    })
}

#[derive(Clone, Copy)]
pub(super) struct Order {
    view: View,
    slot: i32,
    item: i32,
    buy: bool,
}

impl AttributeObject for Order {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn java_string(&self) -> String {
        "(kotlin.Any) -> kotlin.Unit".into()
    }
    fn function_arity(&self) -> Option<u8> {
        Some(1)
    }
}

pub(super) fn button(
    ctx: &mut UiContext<'_>,
    player: &Player,
    group: i32,
    button: Button,
) -> Result<bool> {
    let Some(mut view) = active(ctx)? else {
        return Ok(false);
    };
    let buy = group == MAIN;
    if buy {
        match button.button {
            25 | 26 => {
                view.main = button.button == 25;
                show(ctx, view)?;
                return Ok(true);
            }
            child if child == if view.main { 23 } else { 24 } => (),
            _ => return Ok(false),
        }
    } else if group != SIDE || button.button != 0 {
        return Ok(false);
    }
    if button.slot < 0 || button.slot >= if buy { SLOTS as i32 } else { 28 } {
        return Ok(false);
    }
    let Some(id) = selected(ctx, view, buy, button.slot)? else {
        feedback(ctx, ABSENT)?;
        return Ok(true);
    };
    if button.item >= 0 && button.item != id {
        feedback(ctx, ABSENT)?;
        return Ok(true);
    }
    let order = Order {
        view,
        slot: button.slot,
        item: id,
        buy,
    };
    match button.opcode {
        155 => execute(ctx, player, order, None)?,
        196 => execute(ctx, player, order, Some(1))?,
        124 => execute(ctx, player, order, Some(5))?,
        199 => execute(ctx, player, order, Some(10))?,
        234 => ctx.input_shop_x(
            &player.owner(),
            order,
            if buy {
                "Enter the amount to buy:"
            } else {
                "Enter the amount to sell:"
            },
        )?,
        9 if buy => {
            examine(ctx, id)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub(super) fn examine(ctx: &mut UiContext<'_>, id: i32) -> Result<bool> {
    let definitions = &ctx.host.ui.as_ref().ok_or(Error::Null)?.definitions;
    let Some(definition) = definitions.definitions().get(&id) else {
        return Ok(false);
    };
    let message = {
        let item = definition.read();
        if !item.unnoted {
            "Swap this note at any bank for the equivalent item.".into()
        } else if let Some(message) = &item.examine {
            message.clone()
        } else if let Some(Property::Text(message)) = item.handlers.get("examine") {
            message.clone()
        } else {
            let name = item.name.as_deref().unwrap_or_default();
            let mut letters = name.chars().flat_map(char::to_lowercase);
            match letters.next() {
                None => "null".into(),
                Some('a' | 'e' | 'i' | 'o' | 'u') => format!("It's an {name}."),
                Some('h') if letters.next().is_some_and(|c| c != 'e') => format!("It's an {name}."),
                _ => format!("It's a {name}."),
            }
        }
    };
    feedback(ctx, &message)?;
    Ok(true)
}

fn selected(ctx: &UiContext<'_>, view: View, buy: bool, slot: i32) -> Result<Option<i32>> {
    Ok(if buy {
        catalog(ctx)?
            .stock(view)
            .get(slot as usize)
            .and_then(|entry| entry.map(|item| item.id))
    } else {
        ctx.row()?
            .inventory()
            .get(slot)
            .map(|item| item.read().id())
    })
}

fn currency_property(id: i32) -> Option<&'static str> {
    match id {
        6529 => Some("tokkul_price"),
        1464 => Some("archery_ticket_price"),
        4067 => Some("castle_wars_ticket_price"),
        _ => None,
    }
}

fn fixed_price(id: i32) -> Option<i32> {
    match id {
        12183 => Some(25),
        12155 => Some(1),
        9762 | 9763 => Some(99000),
        _ => None,
    }
}

fn configured(item: &item::ItemDefinition, key: &str, default: i32) -> i32 {
    match item.handlers.get(key) {
        Some(Property::Int(value)) => *value,
        _ => default,
    }
}

fn unit_price(
    shop: &Shop,
    item: &item::ItemDefinition,
    buy: bool,
    baseline: i32,
    balance: i32,
) -> i32 {
    if let Some(property) = currency_property(shop.currency) {
        let price = configured(item, property, 1);
        return if !buy && shop.currency == 6529 {
            price / 10
        } else {
            price
        };
    }
    if let Some(price) = fixed_price(item.id) {
        return price;
    }
    if buy {
        let percentage = if baseline <= 0 || balance >= baseline {
            100
        } else {
            130 - 30 * i64::from(balance.max(0)) / i64::from(baseline)
        };
        return ((i64::from(item.value) * percentage + 99) / 100).clamp(1, i64::from(i32::MAX))
            as i32;
    }
    let (key, factor) = if shop.high_alch {
        ("high_alchemy", 0.6)
    } else {
        ("low_alchemy", 0.4)
    };
    let base = configured(
        item,
        key,
        (f64::from(item.value) * factor).round_ties_even() as i32,
    );
    if balance < baseline {
        return base;
    }
    let extra = i64::from(balance) - i64::from(baseline);
    (f64::from(base) - f64::from(item.value) * 0.03 * extra.min(10) as f64 + 0.5)
        .floor()
        .max(1.0) as i32
}

// Unit prices change monotonically with stock. Sum each price band without
// looping once per item for large Buy X and Sell X requests.
fn total_price(amount: i32, price: impl Fn(i32) -> i32) -> i64 {
    let mut start = 0;
    let mut total = 0;
    while start < amount {
        let value = price(start);
        let mut left = start + 1;
        let mut right = amount;
        while left < right {
            let middle = left + (right - left) / 2;
            if price(middle) == value {
                left = middle + 1;
            } else {
                right = middle;
            }
        }
        total += i64::from(left - start) * i64::from(value);
        start = left;
    }
    total
}

fn sellable(item: &item::ItemDefinition) -> bool {
    matches!(item.handlers.get("tradeable"), Some(Property::Bool(true)))
        && !item.options.as_ref().is_some_and(|options| {
            options.read().iter().flatten().any(|text| {
                text.eq_ignore_ascii_case("destroy") || text.eq_ignore_ascii_case("dissolve")
            })
        })
}

pub(super) fn execute(
    ctx: &mut UiContext<'_>,
    player: &Player,
    order: Order,
    requested: Option<i32>,
) -> Result<()> {
    let Order {
        view,
        slot,
        item,
        buy,
    } = order;
    if active(ctx)? != Some(view) || requested.is_some_and(|n| n <= 0) {
        return Ok(());
    }
    if selected(ctx, view, buy, slot)? != Some(item) {
        return feedback(ctx, ABSENT);
    }

    let definitions = ctx.host.ui.as_ref().ok_or(Error::Null)?.definitions.clone();
    let definition = definitions.for_id(item);
    let unnoted = {
        let d = definition.read();
        if !d.unnoted && d.note_id >= 0 {
            d.note_id
        } else {
            item
        }
    };
    let stock_definition = definitions.for_id(if buy { item } else { unnoted });
    let d = stock_definition.read();
    let name = d.name.as_deref().unwrap_or("Item");
    let catalog = catalog(ctx)?;
    let shop = &catalog.shops[&view.id];
    let currency = shop.currency;
    let currency_name = definitions
        .for_id(currency)
        .read()
        .name
        .as_deref()
        .unwrap_or("currency")
        .to_lowercase();
    let mut stock_view = view;
    let stock_slot = if buy {
        slot as usize
    } else {
        if [COINS, 6529, 1464, 4067, currency].contains(&item) {
            return feedback(
                ctx,
                if requested.is_none() {
                    "This shop will not buy that item."
                } else {
                    "You can't sell currency to a shop."
                },
            );
        }
        if !sellable(&definition.read()) {
            return feedback(
                ctx,
                if requested.is_none() {
                    "This shop will not buy that item."
                } else {
                    "You can't sell this item."
                },
            );
        }
        let matches = |entry: &Option<WireItem>| {
            entry.is_some_and(|entry| entry.id == unnoted || entry.id == d.note_id)
        };
        stock_view.main = true;
        if let Some(slot) = shop.main.iter().rposition(matches) {
            slot
        } else {
            stock_view.main = false;
            let resale = catalog.stock(stock_view);
            let existing = resale.iter().rposition(matches);
            if !shop.general && existing.is_none() {
                return feedback(
                    ctx,
                    if requested.is_none() {
                        "This shop will not buy that item."
                    } else {
                        "This item can't be sold to this shop."
                    },
                );
            }
            let Some(slot) = existing.or_else(|| resale.iter().position(Option::is_none)) else {
                return feedback(ctx, "The shop is too full to buy any more items");
            };
            slot
        }
    };
    let entry = catalog.stock(stock_view)[stock_slot];
    let balance = entry.map_or(0, |entry| entry.amount);
    let baseline = if stock_view.main {
        shop.baseline[stock_slot].map_or(0, |(amount, _)| amount)
    } else if buy {
        balance
    } else {
        0
    };
    let initial_price = unit_price(shop, &d, buy, baseline, balance);
    let Some(requested) = requested else {
        if !buy
            && currency_property(currency)
                .is_some_and(|key| !definition.read().handlers.contains_key(key))
        {
            return feedback(ctx, "This shop will not buy that item.");
        }
        let message = if buy {
            format!("{name}: This item currently costs {initial_price} {currency_name}.")
        } else {
            format!("{name}: This shop will buy this item for {initial_price} {currency_name}.")
        };
        return feedback(ctx, &message);
    };
    if buy && balance == 0 {
        return feedback(ctx, "This item is out of stock.");
    }
    let inventory = player.inventory();
    let make = |id, count| Shared::new(Item::new(id, count, 1000, &definitions));
    let amount = if buy {
        let offered = if balance < 0 {
            requested
        } else {
            requested.min(balance)
        };
        offered.min(
            inventory
                .maximum_add(Some(&make(item, offered)))
                .map_err(binding)?
                .max(1),
        )
    } else {
        requested
            .min(inventory.amount_id(item, &definitions))
            .min(i32::MAX - balance.max(0))
    };
    if amount <= 0 {
        return Ok(());
    }
    let total = if !buy && let Some(price) = fixed_price(unnoted) {
        i64::from(price) * i64::from(amount)
    } else if currency != COINS {
        i64::from(initial_price) * i64::from(amount)
    } else {
        total_price(amount, |offset| {
            unit_price(
                shop,
                &d,
                buy,
                baseline,
                if buy {
                    balance.saturating_sub(offset)
                } else {
                    balance.saturating_add(offset)
                },
            )
        })
    };
    if buy && total > i64::from(inventory.amount_id(currency, &definitions)) {
        return feedback(
            ctx,
            &format!("You don't have enough {currency_name} to buy that many."),
        );
    }
    let Ok(payment) = i32::try_from(total) else {
        return feedback(ctx, "You don't have enough space to do that.");
    };
    let (take, give) = if buy {
        (make(currency, payment), make(item, amount))
    } else {
        (make(item, amount), make(currency, payment))
    };
    drop(d);
    let success = player
        .apply(
            ctx.state,
            &mut ctx.host.frames,
            now,
            |inventory, definitions, services| {
                let mut staged = Factory::new(definitions).create(
                    inventory.capacity(),
                    inventory.kind(),
                    inventory.sort(),
                )?;
                staged.copy_from(inventory, definitions);
                if (take.read().amount() > 0
                    && !staged.remove(
                        Some(&take),
                        (!buy).then_some(slot),
                        false,
                        definitions,
                        services,
                    )?)
                    || (give.read().amount() > 0
                        && !staged.add(Some(&give), false, -1, definitions, services)?)
                {
                    return Ok(false);
                }
                *inventory.items().write() = staged.items().read().clone();
                Ok(true)
            },
        )
        .map_err(binding)?;
    if !success {
        return feedback(
            ctx,
            if buy {
                "You don't have enough inventory space to buy that many."
            } else {
                "You don't have enough space to do that."
            },
        );
    }

    let store = ctx.host.ui.as_mut().ok_or(Error::Null)?;
    let remaining = if balance < 0 {
        balance
    } else if buy {
        balance - amount
    } else {
        balance + amount
    };
    store.shops.stock_mut(stock_view)[stock_slot] =
        (stock_view.main || remaining != 0).then_some(WireItem {
            id: entry.map_or(unnoted, |entry| entry.id),
            amount: remaining,
        });
    if stock_view.main && remaining != baseline {
        store.shops.restoring.insert((stock_view.id, stock_slot));
    }
    store
        .shops
        .notify(&store.players, &mut ctx.host.frames, stock_view)?;
    if stock_view != view {
        show(ctx, stock_view)?;
    }
    inventory_binding::refresh(ctx.state, ctx.host, ctx.node)
}

pub(in super::super) fn restock(p: &mut Probe) -> Result<()> {
    let Some(store) = p.host.ui.as_mut() else {
        return Ok(());
    };
    let ticks = p.state.ticks;
    let mut changed = BTreeSet::new();
    store.shops.restoring.retain(|&(id, slot)| {
        let shop = store.shops.shops.get_mut(&id).expect("restocking shop");
        let Some((baseline, interval)) = shop.baseline[slot] else {
            return false;
        };
        let Some(entry) = &mut shop.main[slot] else {
            return false;
        };
        if interval <= 0 || entry.amount == baseline {
            return false;
        }
        if ticks % interval == 0 {
            entry.amount += if entry.amount < baseline { 1 } else { -1 };
            changed.insert(id);
        }
        entry.amount != baseline
    });
    for id in changed {
        store
            .shops
            .notify(&store.players, &mut p.host.frames, View { id, main: true })?;
    }
    // The default 180-minute general-store second-hand stock reset.
    if ticks != 0 && ticks % 18_000 == 0 && store.shops.general_stock.iter().any(Option::is_some) {
        store.shops.general_stock = [None; SLOTS];
        if let Some((&id, _)) = store.shops.shops.iter().find(|(_, shop)| shop.general) {
            store
                .shops
                .notify(&store.players, &mut p.host.frames, View { id, main: false })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::{
        buffer::{Reader, Writer},
        game::updates::ServerUpdate,
    };
    use rs09_server::player::details::ClientInfo;

    const CATALOG: &str = include_str!("../../../../fixtures/shops/shops.json");

    fn click(p: &mut Probe, player: Node, group: i32, child: i32, slot: i32, opcode: u8) {
        let mut bytes = Writer::new();
        bytes.p4(group << 16 | child);
        bytes.p2(slot);
        let packet = bytes.into_vec();
        let action = ClientAction::decode(opcode, &mut Reader::new(&packet))
            .unwrap()
            .unwrap();
        assert!(super::super::process(p, player, &action).unwrap());
    }

    #[test]
    fn original_lumbridge_stock_opens_both_panes_and_trades_through_button_packets() {
        let (mut p, node, _) = super::super::super::tests::probe();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        let mut store = super::super::Ui::load(&path).unwrap();
        let mut defs = item::Registry::default();
        for (id, name, worth, stackable) in [
            (995, "Coins", 1, true),
            (1931, "Pot", 2, false),
            (1511, "Logs", 4, false),
        ] {
            let mut def = item::ItemDefinition {
                id,
                name: Some(name.into()),
                value: worth,
                stackable,
                ..Default::default()
            };
            def.handlers
                .insert("tradeable".into(), Property::Bool(true));
            if id == 1931 {
                def.handlers.insert(
                    "examine".into(),
                    Property::Text("This pot is empty.".into()),
                );
            }
            defs.insert(id, Shared::new(def));
        }
        store.use_definitions(std::sync::Arc::new(defs)).unwrap();
        store.shops = Catalog::load(CATALOG).unwrap();
        assert_eq!(store.shops.shops.len(), 245);
        assert_eq!(store.shops.merchants[&520], 25);
        assert_eq!(store.shops.merchants[&521], 25);
        p.host.ui = Some(store);
        super::super::admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let owner = p.state.retain_node(node).unwrap();
        let player = p.state.player_fields(&owner).unwrap();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                now,
                |inventory, definitions, services| {
                    for (id, qty) in [(995, 100), (1511, 1)] {
                        assert!(inventory.add(
                            Some(&Shared::new(Item::new(id, qty, 1000, definitions))),
                            false,
                            -1,
                            definitions,
                            services
                        )?);
                    }
                    Ok(())
                },
            )
            .unwrap();
        let inv = player.inventory();
        let count = |id| {
            inv.items()
                .read()
                .iter()
                .flatten()
                .map(|i| i.read())
                .filter(|i| i.id() == id)
                .map(|i| i.amount())
                .sum::<i32>()
        };
        p.host.frames.clear();
        open(&mut UiContext::new(&mut p, node), 25).unwrap();
        for group in [MAIN, SIDE] {
            assert!(p.host.frames[&node].iter().any(|f| matches!(ServerUpdate::decode(f), Ok(Some(ServerUpdate::OpenInterface { interface, .. })) if interface == group)));
        }

        for (opcode, expected) in [
            (155, "Pot: This item currently costs 2 coins."),
            (9, "This pot is empty."),
        ] {
            click(&mut p, node, MAIN, 23, 0, opcode);
            assert!(
                matches!(ServerUpdate::decode(p.host.frames[&node].last().unwrap()),
                Ok(Some(ServerUpdate::GameMessage(text))) if text == expected.as_bytes())
            );
        }

        click(&mut p, node, MAIN, 23, 0, 124);
        // The first pot costs 2, then four pots cost 3 each as stock falls.
        assert_eq!((count(995), count(1931)), (86, 5));
        click(&mut p, node, MAIN, 23, 0, 234);
        super::super::process(
            &mut p,
            node,
            &ClientAction::InputStringResponse {
                response: "2".into(),
            },
        )
        .unwrap();
        assert_eq!((count(995), count(1931)), (80, 7));

        click(&mut p, node, SIDE, 0, 1, 196);
        let resale = View { id: 2, main: false };
        assert_eq!(
            p.host.ui.as_ref().unwrap().shops.stock(resale)[0],
            Some(WireItem {
                id: 1511,
                amount: 1
            })
        );
        click(&mut p, node, MAIN, 24, 0, 196);
        assert_eq!(count(1511), 1);
        assert_eq!(p.host.ui.as_ref().unwrap().shops.stock(resale)[0], None);

        p.state.ticks = 100;
        restock(&mut p).unwrap();
        let main = View { id: 25, main: true };
        assert_eq!(
            p.host.ui.as_ref().unwrap().shops.stock(main)[0]
                .unwrap()
                .amount,
            24
        );
        click(&mut p, node, MAIN, 25, -1, 155);
        click(&mut p, node, MAIN, 23, 0, 234);
        super::super::process(&mut p, node, &ClientAction::CloseIface).unwrap();
        super::super::process(
            &mut p,
            node,
            &ClientAction::InputStringResponse {
                response: "3".into(),
            },
        )
        .unwrap();
        assert!(!ui::has_main_component(&player.manager(), MAIN));
        assert_eq!(count(1931), 7);
    }

    #[test]
    fn large_price_batches_sum_the_same_tiers_as_individual_units() {
        let catalog = Catalog::load(CATALOG).unwrap();
        let shop = &catalog.shops[&25];
        let item = item::ItemDefinition {
            value: 143,
            ..Default::default()
        };
        for buy in [false, true] {
            let price = |offset| {
                unit_price(
                    shop,
                    &item,
                    buy,
                    100,
                    if buy { 100 - offset } else { 15 + offset },
                )
            };
            assert_eq!(
                total_price(100, price),
                (0..100).map(|offset| i64::from(price(offset))).sum::<i64>()
            );
        }
        assert_eq!(total_price(i32::MAX, |_| 2000), i64::from(i32::MAX) * 2000);
    }
}
