//! GrandExchange.kt: the start-up listener (`startup`, `boot`), the "GE
//! Update Worker" loop, matching (`selectPotentialMatches`, `exchange`,
//! `tryExchangeWithBots`), `dispatch`, `addBotOffer`, the queries
//! (`getRecommendedPrice`, `getOfferStats`, `getValidOffers`,
//! `getBotOffers`, `getBotstockForId`, `getOfferByUid`) and the three admin
//! command bodies, in the original order.
use super::{
    Error, Host, Offer, OfferState, Param, Pulse, Result, Worker, bot_prices, db, offer,
    price_index, records,
};
use crate::world::{drop_tables::LogLevel, player_walk::NodeHandle};

pub const GET_SPECIFIC_OFFER_BY_UID: &str = "SELECT * FROM player_offers WHERE uid = ?;";
pub const GET_MATCHES_FROM_PLAYER_OFFERS: &str = "SELECT * FROM player_offers WHERE item_id = ? AND is_sale = ? AND offer_state < 4 AND NOT offer_state = 2;";
pub const GET_MATCH_FROM_BOT_OFFERS: &str = "SELECT * FROM bot_offers WHERE item_id = ?;";

/// The three commands `defineCommands` registers: name, privilege, usage,
/// description.
pub const COMMANDS: [(&str, &str, &str, &str); 3] = [
    (
        "addbotoffer",
        "ADMIN",
        "::addbotoffer <lt>item-id<gt> <lt>amount<gt>",
        "Adds a GE bot sell offer for the given item ID and amount.",
    ),
    (
        "bange",
        "ADMIN",
        "::bange <lt>item-id<gt>",
        "Bans/blacklists the specified item from GE trades.",
    ),
    (
        "allowge",
        "ADMIN",
        "::allowge <lt>item-id<gt>",
        "Allows/whitelists the specified item to be traded on the GE.",
    ),
];

/// `GrandExchange.startup()`: `GEDB.init()` then `boot()`.
pub fn startup<H: Host + ?Sized>(h: &mut H) -> Result<()> {
    db::init_default(h)?;
    boot(h)
}

/// `boot()`: logs and starts the worker once (`isRunning`).
pub fn boot<H: Host + ?Sized>(h: &mut H) -> Result<()> {
    if h.exchange()?.worker() != Worker::NotStarted {
        return Ok(());
    }
    h.log_ge("Initializing GE Update Worker")?;
    h.start_worker()?;
    h.exchange()?.tables().worker = Worker::Waiting;
    Ok(())
}

/// The worker thread's loop, run until `pendingOffers` is empty: each offer
/// is written, read back by uid, matched against the player offers sorted by
/// price (best first, stable) and, for a player's buy offer with something
/// left, against the bot offers. An exception ends the thread for good
/// ([`Worker::Dead`]); offers queued after that are never processed.
pub fn run_worker<H: Host + ?Sized>(h: &mut H) -> Result<()> {
    loop {
        let exchange = h.exchange()?;
        if exchange.worker() != Worker::Waiting {
            return Ok(());
        }
        let Some(next) = exchange.tables().pending.pop_front() else {
            return Ok(());
        };
        if let Err(e) = process(h, next) {
            h.exchange()?.tables().worker = Worker::Dead;
            return Err(e);
        }
    }
}

fn process<H: Host + ?Sized>(h: &mut H, queued: Offer) -> Result<()> {
    offer::write_new(h, &queued)?;
    let uid = queued.uid();
    let Some(current) = offer_by_uid(h, uid)? else {
        return Ok(());
    };
    let matches = select_potential_matches(h, &current)?;
    let sell = current.sell();
    let mut sorted: Vec<Offer> = matches;
    sorted.sort_by_key(|m| {
        let value = m.offered_value();
        if sell { value.wrapping_neg() } else { value }
    });
    for candidate in sorted {
        let keep = if sell {
            candidate.offered_value() >= current.offered_value()
        } else {
            candidate.offered_value() <= current.offered_value()
        };
        if keep {
            exchange(h, &current, &candidate)?;
        }
    }
    if !current.is_bot() && offer::amount_left(h, &current)? > 0 && !current.sell() {
        try_exchange_with_bots(h, &current)?;
    }
    Ok(())
}

/// `tryExchangeWithBots(offer)`: the first bot row of the item, as a bot
/// offer stamped one millisecond before the player's.
fn try_exchange_with_bots<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<()> {
    let db = h.exchange()?.db()?;
    let rows = db
        .prepare(GET_MATCH_FROM_BOT_OFFERS)?
        .query(h, &[Param::Int(o.item_id())])?;
    if !rows.is_empty() {
        let bot = offer::from_bot_query(h, &rows, 0)?;
        let stamp = o.fields().time_stamp.wrapping_sub(1);
        bot.fields().time_stamp = stamp;
        exchange(h, o, &bot)?;
    }
    Ok(())
}

/// `selectPotentialMatches(offer)`: open player offers of the item on the
/// other side, in row order.
fn select_potential_matches<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<Vec<Offer>> {
    let db = h.exchange()?.db()?;
    let rows = db
        .prepare(GET_MATCHES_FROM_PLAYER_OFFERS)?
        .query(h, &[Param::Int(o.item_id()), Param::Bool(!o.sell())])?;
    let mut matches = Vec::with_capacity(rows.len());
    for row in 0..rows.len() {
        matches.push(offer::from_query(h, &rows, row)?);
    }
    Ok(matches)
}

/// `getOfferByUid(uid)`.
pub fn offer_by_uid<H: Host + ?Sized>(h: &mut H, uid: i64) -> Result<Option<Offer>> {
    let db = h.exchange()?.db()?;
    let rows = db
        .prepare(GET_SPECIFIC_OFFER_BY_UID)?
        .query(h, &[Param::Long(uid)])?;
    if rows.is_empty() {
        return Ok(None);
    }
    Ok(Some(offer::from_query(h, &rows, 0)?))
}

/// `getRecommendedPrice(itemID, from_bot)`: the price index value; for a bot
/// offer 110% of the larger of it and the bot price, truncated.
pub fn recommended_price<H: Host + ?Sized>(h: &mut H, item: i32, from_bot: bool) -> Result<i32> {
    let mut base = price_index::get_value(h, item)?;
    if from_bot {
        let bot = bot_prices::get_price(h, item)?;
        base = java_double_to_int(f64::from(bot.max(base)) * 1.10);
    }
    Ok(base)
}

/// Kotlin `Double.toInt()`: NaN is 0, saturating.
pub fn java_double_to_int(value: f64) -> i32 {
    if value.is_nan() { 0 } else { value as i32 }
}

/// `getOfferStats(itemID, sale)`: the text of interface 105's child 142.
pub fn offer_stats<H: Host + ?Sized>(h: &mut H, item: i32, sale: bool) -> Result<String> {
    let mut sb = String::new();
    let db = h.exchange()?.db()?;
    let mut found = 0i32;
    let mut total = 0i32;
    let mut best = 0i32;
    if !sale {
        let mut bot_amount = 0i32;
        let mut bot_price = 0i32;
        let rows = db.query(
            h,
            &format!("SELECT * from player_offers where item_id = {item} AND is_sale = 1 AND offer_state < 4 AND NOT offer_state = 2"),
        )?;
        for row in 0..rows.len() {
            let o = offer::from_query(h, &rows, row)?;
            found = found.wrapping_add(1);
            total = total.wrapping_add(offer::amount_left(h, &o)?);
            if o.offered_value() < best || best == 0 {
                best = o.offered_value();
            }
        }
        let rows = db.query(
            h,
            &format!("SELECT * from bot_offers where item_id = {item}"),
        )?;
        if !rows.is_empty() {
            let o = offer::from_bot_query(h, &rows, 0)?;
            bot_amount = offer::amount(h, &o)?;
            bot_price = recommended_price(h, item, true)?;
        }
        sb.push_str(&format!("Player Stock: <col=FFFFFF>{total}  "));
        sb.push_str(&format!("</col>  Lowest Price: <col=FFFFFF>{best}<br>"));
        sb.push_str(&"-".repeat(50));
        sb.push_str(&format!("</col><br>Bot Stock: <col=FFFFFF>{bot_amount}  "));
        sb.push_str(&format!("</col>  Bot Price: <col=FFFFFF>{bot_price}"));
    } else {
        let rows = db.query(
            h,
            &format!("SELECT * from player_offers where item_id = {item} AND is_sale = 0 AND offer_state < 4 AND NOT offer_state = 2"),
        )?;
        for row in 0..rows.len() {
            let o = offer::from_query(h, &rows, row)?;
            found = found.wrapping_add(1);
            total = total.wrapping_add(offer::amount_left(h, &o)?);
            if o.offered_value() > best {
                best = o.offered_value();
            }
        }
        sb.push_str(&format!("Buy Offers: <col=FFFFFF>{total}    "));
        sb.push_str(&format!("</col>Highest Offer: <col=FFFFFF>{best}</col>"));
    }
    let _ = found;
    Ok(sb)
}

/// `addBotOffer(itemID, amount)`: refused for an item without a price index
/// row; noted ids are queued as their note's id (the unnoted item).
pub fn add_bot_offer<H: Host + ?Sized>(h: &mut H, item: i32, total: i32) -> Result<bool> {
    if !price_index::can_trade(h, item)? {
        return Ok(false);
    }
    let definitions = h.definitions()?;
    let definition = definitions.for_id(item);
    let (unnoted, note) = {
        let d = definition.read();
        (d.unnoted, d.note_id)
    };
    let o = offer::create_bot_offer(h, if unnoted { item } else { note }, total, true)?;
    h.exchange()?.tables().pending.push_back(o);
    Ok(true)
}

/// `dispatch(player, offer)`: validates the offer, assigns its owner (the
/// shared "2009scape" account for an artificial player, which also makes it
/// a bot offer), announces a player's sale and queues it.
pub fn dispatch<H: Host + ?Sized>(h: &mut H, player: &NodeHandle, o: &Offer) -> Result<bool> {
    if offer::amount(h, o)? < 1 {
        h.content_message(player, "You must choose the quantity you wish to buy!")?;
        return Ok(false);
    }
    if o.offered_value() < 1 {
        h.content_message(player, "You must choose the price you wish to buy for!")?;
        return Ok(false);
    }
    let (state, uid) = {
        let f = o.fields();
        (f.state, f.uid)
    };
    if state != OfferState::Pending || uid != 0 {
        h.log(
            LogLevel::Warn,
            &format!("[GE] DISPATCH FAILURE: {}, UID: {uid}", state.name()),
        )?;
        return Ok(false);
    }
    if h.artificial(player)? {
        let shared = h.details_uid("2009scape")?;
        let mut f = o.fields();
        f.player_uid = shared;
        f.is_bot = true;
    } else {
        let uid = h.uid(player)?;
        o.fields().player_uid = uid;
    }
    o.fields().state = OfferState::Registered;
    if o.sell() && !h.artificial(player)? {
        let name = h.username(player)?;
        let total = offer::amount(h, o)?;
        let item = o.item_id();
        let item_name = item_name(h, item)?;
        h.news(&format!(
            "{name} just offered {total} {item_name} on the GE."
        ))?;
    }
    if h.cheater()? {
        let other = Offer::new();
        let item = o.item_id();
        let total = offer::amount(h, o)?;
        let (sell, value) = {
            let f = o.fields();
            (f.sell, f.offered_value)
        };
        {
            let mut f = other.fields();
            f.item_id = item;
            f.amount = total;
            f.sell = !sell;
            f.offered_value = value;
        }
        offer::write_new(h, o)?;
        h.submit_pulse(Pulse::Cheat {
            offer: o.clone(),
            other,
        })?;
        return Ok(true);
    }
    h.exchange()?.tables().pending.push_back(o.clone());
    Ok(true)
}

/// The `I_AM_A_CHEATER` pulse body: the offer read back by uid is exchanged
/// with the mirrored offer.
pub fn run_cheat_pulse<H: Host + ?Sized>(h: &mut H, o: &Offer, other: &Offer) -> Result<bool> {
    let Some(current) = offer_by_uid(h, o.uid())? else {
        return Ok(false);
    };
    exchange(h, &current, other)?;
    Ok(true)
}

/// `getItemName(id)`: the definition's name (`null` when it has none).
pub fn item_name<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<String> {
    Ok(h.definitions()?
        .for_id(id)
        .read()
        .name
        .clone()
        .unwrap_or_else(|| "null".into()))
}

/// `exchange(offer, other)`: trades what both have left when the buyer pays
/// at least the seller's price. The older offer's price is used: a seller
/// newer than the buyer is paid the buyer's price; otherwise the buyer pays
/// the seller's price and is refunded the difference. Both rows are updated
/// and online owners shown their offers and told at the next timer run.
pub fn exchange<H: Host + ?Sized>(h: &mut H, o: &Offer, other: &Offer) -> Result<()> {
    if o.sell() == other.sell() {
        return Ok(());
    }
    let mine = offer::amount(h, o)?.wrapping_sub(o.completed_amount());
    let theirs = offer::amount(h, other)?.wrapping_sub(other.completed_amount());
    let amount = mine.min(theirs);
    if amount == 0 {
        return Ok(());
    }
    let seller = if o.sell() { o.clone() } else { other.clone() };
    let buyer = if o.same(&seller) {
        other.clone()
    } else {
        o.clone()
    };
    let seller_bias = seller.fields().time_stamp > buyer.fields().time_stamp;
    if seller.offered_value() > buyer.offered_value() {
        return Ok(());
    }
    {
        let mut f = seller.fields();
        f.completed_amount = f.completed_amount.wrapping_add(amount);
    }
    {
        let mut f = buyer.fields();
        f.completed_amount = f.completed_amount.wrapping_add(amount);
    }
    let price = if seller_bias {
        buyer.offered_value()
    } else {
        seller.offered_value()
    };
    offer::add_withdraw_item(h, &seller, super::COINS, amount.wrapping_mul(price))?;
    let item = seller.item_id();
    offer::add_withdraw_item(h, &buyer, item, amount)?;
    if !seller_bias {
        let refund = buyer.offered_value().wrapping_sub(seller.offered_value());
        offer::add_withdraw_item(h, &buyer, super::COINS, amount.wrapping_mul(refund))?;
    }
    if offer::amount_left(h, &seller)? < 1 {
        seller.fields().state = OfferState::Completed;
    }
    if offer::amount_left(h, &buyer)? < 1 {
        buyer.fields().state = OfferState::Completed;
    }
    let total = price.wrapping_mul(amount);
    {
        let mut f = seller.fields();
        f.total_coin_exchange = f.total_coin_exchange.wrapping_add(total);
    }
    {
        let mut f = buyer.fields();
        f.total_coin_exchange = f.total_coin_exchange.wrapping_add(total);
    }
    if can_update_price_index(h, &seller, &buyer)? {
        if amount == 0 {
            return Err(Error::Arithmetic);
        }
        price_index::add_trade(h, o.item_id(), amount, total.wrapping_div(amount))?;
    }
    for entity in [&buyer, &seller] {
        offer::update(h, entity)?;
        let uid = entity.fields().player_uid;
        let Some(player) = h.player_by_uid(uid)? else {
            continue;
        };
        let records = records::get_instance(h, Some(&player))?;
        records::visualize_records(h, &records)?;
        records.fields().update_notification = true;
    }
    Ok(())
}

fn can_update_price_index<H: Host + ?Sized>(
    h: &mut H,
    seller: &Offer,
    buyer: &Offer,
) -> Result<bool> {
    if seller.fields().player_uid == buyer.fields().player_uid {
        return Ok(false);
    }
    if !h.bots_influence_price_index()? && (seller.is_bot() || buyer.is_bot()) {
        return Ok(false);
    }
    Ok(true)
}

/// `getValidOffers()`: every open player offer, in row order.
pub fn valid_offers<H: Host + ?Sized>(h: &mut H) -> Result<Vec<Offer>> {
    let db = h.exchange()?.db()?;
    let rows = db.query(
        h,
        "SELECT * FROM player_offers WHERE offer_state < 4 AND NOT offer_state = 2",
    )?;
    let mut offers = Vec::with_capacity(rows.len());
    for row in 0..rows.len() {
        offers.push(offer::from_query(h, &rows, row)?);
    }
    Ok(offers)
}

/// `getBotOffers()`: every bot row with stock.
pub fn bot_offers<H: Host + ?Sized>(h: &mut H) -> Result<Vec<Offer>> {
    let db = h.exchange()?.db()?;
    let rows = db.query(h, "SELECT item_id,amount FROM bot_offers WHERE amount > 0")?;
    let mut offers = Vec::with_capacity(rows.len());
    for row in 0..rows.len() {
        offers.push(offer::from_bot_query(h, &rows, row)?);
    }
    Ok(offers)
}

/// `getBotstockForId(itemId)`.
pub fn botstock_for_id<H: Host + ?Sized>(h: &mut H, item: i32) -> Result<i32> {
    let db = h.exchange()?.db()?;
    let rows = db
        .prepare("SELECT sum(amount) FROM bot_offers WHERE amount > 0 AND item_id = ?")?
        .query(h, &[Param::Int(item)])?;
    let mut total = 0i32;
    for row in 0..rows.len() {
        total = total.wrapping_add(rows.int_at(row, 1)?);
    }
    Ok(total)
}

/// The `::addbotoffer`, `::bange` and `::allowge` bodies (`strings` is the
/// split command line).
pub fn command<H: Host + ?Sized>(
    h: &mut H,
    player: &NodeHandle,
    name: &str,
    strings: &[String],
) -> Result<()> {
    let arg = |i: usize| -> Result<i32> {
        let text = strings.get(i).ok_or(Error::Bounds)?;
        rs09_game_core::java_string::parse_i32(text).ok_or(Error::Number)
    };
    match name {
        "addbotoffer" => {
            let id = arg(1)?;
            let amount = arg(2)?;
            add_bot_offer(h, id, amount)?;
            let item = item_name(h, id)?;
            h.notify(
                player,
                &format!("Added {amount}x {item} to the bot offers."),
            )
        }
        "bange" => {
            let id = arg(1)?;
            price_index::ban_item(h, id)?;
            let item = item_name(h, id)?;
            h.notify(player, &format!("Banned {item} from GE trade."))
        }
        "allowge" => {
            let id = arg(1)?;
            price_index::allow_item(h, id)?;
            let item = item_name(h, id)?;
            h.notify(player, &format!("Allowed {item} for GE trade."))
        }
        other => Err(Error::Service(format!("no GE command {other}"))),
    }
}
