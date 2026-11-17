//! GrandExchangeOffer.kt and OfferState.java. An offer is a shared mutable
//! object: the "ge-temp" attribute, `pendingOffers` and an exchange may hold
//! the same one. Its withdraw slots are `Item`s only the offer holds.
use super::{Error, Host, Param, Result, db::Rows, records, ui};
use crate::world::player_walk::NodeHandle;
use std::sync::{Arc, Mutex, MutexGuard};

/// `OfferState`, in ordinal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferState {
    Pending,
    Registered,
    Aborted,
    Updated,
    Completed,
    Outdated,
    Removed,
}
impl OfferState {
    pub const ALL: [Self; 7] = [
        Self::Pending,
        Self::Registered,
        Self::Aborted,
        Self::Updated,
        Self::Completed,
        Self::Outdated,
        Self::Removed,
    ];
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    /// `OfferState.values()[ordinal]`.
    pub fn from_ordinal(ordinal: i32) -> Result<Self> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .ok_or(Error::Bounds)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Registered => "REGISTERED",
            Self::Aborted => "ABORTED",
            Self::Updated => "UPDATED",
            Self::Completed => "COMPLETED",
            Self::Outdated => "OUTDATED",
            Self::Removed => "REMOVED",
        }
    }
}

/// A withdraw slot's `Item` (id, amount). `Item(id, amount)` stores the
/// amount as given; `setAmount` (the `+=` of `addWithdrawItem`) clamps a
/// negative amount to 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WithdrawItem {
    pub id: i32,
    pub amount: i32,
}

pub struct Fields {
    pub item_id: i32,
    pub completed_amount: i32,
    pub offered_value: i32,
    pub index: i32,
    pub sell: bool,
    pub state: OfferState,
    pub uid: i64,
    pub time_stamp: i64,
    pub withdraw: [Option<WithdrawItem>; 2],
    pub total_coin_exchange: i32,
    pub player: Option<NodeHandle>,
    pub player_uid: i32,
    pub is_limitation: bool,
    pub is_bot: bool,
    /// The `amount` backing field (the getter clamps it for bots).
    pub amount: i32,
}
/// One GrandExchangeOffer object. Clones are the same object.
#[derive(Clone)]
pub struct Offer(Arc<Mutex<Fields>>);
impl Offer {
    /// `GrandExchangeOffer()`.
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Fields {
            item_id: 0,
            completed_amount: 0,
            offered_value: 0,
            index: 0,
            sell: false,
            state: OfferState::Pending,
            uid: 0,
            time_stamp: 0,
            withdraw: [None, None],
            total_coin_exchange: 0,
            player: None,
            player_uid: 0,
            is_limitation: false,
            is_bot: false,
            amount: 0,
        })))
    }
    pub fn fields(&self) -> MutexGuard<'_, Fields> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    pub fn item_id(&self) -> i32 {
        self.fields().item_id
    }
    pub fn sell(&self) -> bool {
        self.fields().sell
    }
    pub fn uid(&self) -> i64 {
        self.fields().uid
    }
    pub fn index(&self) -> i32 {
        self.fields().index
    }
    pub fn state(&self) -> OfferState {
        self.fields().state
    }
    pub fn offered_value(&self) -> i32 {
        self.fields().offered_value
    }
    pub fn completed_amount(&self) -> i32 {
        self.fields().completed_amount
    }
    pub fn is_bot(&self) -> bool {
        self.fields().is_bot
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.fields().player.clone()
    }
    pub fn withdraw(&self) -> [Option<WithdrawItem>; 2] {
        self.fields().withdraw
    }
}
impl Default for Offer {
    fn default() -> Self {
        Self::new()
    }
}

/// The `amount` getter: `min(field, BOTSTOCK_LIMIT)` for a bot offer.
pub fn amount<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<i32> {
    let (bot, field) = {
        let f = o.fields();
        (f.is_bot, f.amount)
    };
    Ok(if bot {
        field.min(h.botstock_limit()?)
    } else {
        field
    })
}
/// `amountLeft`.
pub fn amount_left<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<i32> {
    if o.is_bot() {
        let limit = h.botstock_limit()?;
        let total = amount(h, o)?;
        Ok(limit.min(total.wrapping_sub(o.completed_amount())))
    } else {
        Ok(amount(h, o)?.wrapping_sub(o.completed_amount()))
    }
}
/// `isActive`.
pub fn is_active(o: &Offer) -> bool {
    !matches!(
        o.state(),
        OfferState::Aborted | OfferState::Pending | OfferState::Completed | OfferState::Removed
    )
}

/// `addWithdrawItem(id, amount)`. With both slots taken by other items the
/// item is lost and the offer is shown to its player (when online).
pub fn add_withdraw_item<H: Host + ?Sized>(
    h: &mut H,
    o: &Offer,
    id: i32,
    amount: i32,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    {
        let mut f = o.fields();
        for item in f.withdraw.iter_mut().flatten() {
            if item.id == id {
                let total = item.amount.wrapping_add(amount);
                item.amount = total.max(0);
                return Ok(());
            }
        }
        for slot in f.withdraw.iter_mut() {
            if slot.is_none() {
                *slot = Some(WithdrawItem { id, amount });
                return Ok(());
            }
        }
    }
    let player = o.player();
    if player.is_some() {
        visualize(h, o, player.as_ref())?;
    }
    Ok(())
}

/// `visualize(player)`: the GrandExchangePacket (whose send swallows any
/// exception) and the withdraw container to -1/-1757, container 523 + index.
pub fn visualize<H: Host + ?Sized>(
    h: &mut H,
    o: &Offer,
    player: Option<&NodeHandle>,
) -> Result<()> {
    let Some(player) = player else {
        return Ok(());
    };
    let (index, state, item, sell, value, total, completed, coins, withdraw) = {
        let f = o.fields();
        (
            f.index,
            f.state,
            f.item_id,
            f.sell,
            f.offered_value,
            f.amount,
            f.completed_amount,
            f.total_coin_exchange,
            f.withdraw,
        )
    };
    // `amount` is read through the getter.
    let total = if o.is_bot() {
        total.min(h.botstock_limit()?)
    } else {
        total
    };
    let frame = ui::exchange_frame(
        index as i8,
        state.ordinal() as i8,
        item as i16,
        sell,
        value,
        total,
        completed,
        coins,
    );
    let _ = h.send(player, frame);
    ui::send_withdraw_container(h, player, 523i32.wrapping_add(index), &withdraw)
}

/// `update()`: the bot row's amount, or the player row's progress.
pub fn update<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<()> {
    let db = h.exchange()?.db()?;
    if o.is_bot() {
        let left = amount_left(h, o)?;
        let item = o.item_id();
        db.prepare("UPDATE bot_offers SET amount = ? WHERE item_id = ?")?
            .update(h, &[Param::Int(left), Param::Int(item)])?;
    } else {
        let (completed, state, coins, withdraw, index, uid) = {
            let f = o.fields();
            (
                f.completed_amount,
                f.state,
                f.total_coin_exchange,
                f.withdraw,
                f.index,
                f.uid,
            )
        };
        db.prepare(
            "UPDATE player_offers SET amount_complete = ?, offer_state = ?, total_coin_xc = ?, withdraw_items = ?, slot_index = ? WHERE uid = ?",
        )?
        .update(
            h,
            &[
                Param::Int(completed),
                Param::Int(state.ordinal()),
                Param::Int(coins),
                Param::Text(Some(encode_withdraw(&withdraw))),
                Param::Int(index),
                Param::Long(uid),
            ],
        )?;
    }
    Ok(())
}

/// `writeNew()`: a bot offer adds to its item's row (or inserts one); a
/// player offer inserts its row, takes the new uid, records it in the
/// player's records at its index, is shown to its player and posted to
/// Discord.
pub fn write_new<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<()> {
    let db = h.exchange()?.db()?;
    if o.is_bot() {
        let item = o.item_id();
        let rows = db.query(
            h,
            &format!("SELECT * from bot_offers where item_id = {item}"),
        )?;
        if !rows.is_empty() {
            let old = rows.int(0, "amount")?;
            let total = old.wrapping_add(amount(h, o)?);
            let item = o.item_id();
            db.update(
                h,
                &format!("UPDATE bot_offers set amount = {total} where item_id = {item}"),
            )?;
        } else {
            let item = o.item_id();
            let total = amount(h, o)?;
            db.update(
                h,
                &format!("INSERT INTO bot_offers(item_id,amount) values({item},{total})"),
            )?;
        }
    } else {
        let (player_uid, item, total, value) = {
            let f = o.fields();
            (f.player_uid, f.item_id, f.amount, f.offered_value)
        };
        let now = h.now()?;
        let (state, sell, index) = {
            let f = o.fields();
            (f.state, f.sell, f.index)
        };
        db.update(
            h,
            &format!(
                "INSERT INTO player_offers(player_uid, item_id, amount_total, offered_value, time_stamp, offer_state, is_sale, slot_index) values({player_uid},{item},{total},{value},{now},{},{}, {index})",
                state.ordinal(),
                if sell { 1 } else { 0 }
            ),
        )?;
        let rows = db.query(h, "SELECT last_insert_rowid()")?;
        let uid = rows.long_at(0, 1)?;
        o.fields().uid = uid;
        let player = o.player();
        let instance = records::get_instance(h, player.as_ref())?;
        let index = o.index();
        instance.set_record(index, Some(records::OfferRecord { uid, slot: index }))?;
        visualize(h, o, player.as_ref())?;
        // Arguments in order; `player?.username ?: return@run` is the last.
        let (sell, item, value) = {
            let f = o.fields();
            (f.sell, f.item_id, f.offered_value)
        };
        let total = amount(h, o)?;
        let Some(player) = o.player() else {
            return Ok(());
        };
        let username = h.username(&player)?;
        h.discord_new_offer(sell, item, value, total, &username)?;
    }
    Ok(())
}

/// `encodeWithdraw()`: `index,id,amount` per slot (`null` for an empty
/// slot), joined with `:`.
pub fn encode_withdraw(withdraw: &[Option<WithdrawItem>; 2]) -> String {
    let mut out = String::new();
    for (index, item) in withdraw.iter().enumerate() {
        out.push_str(&index.to_string());
        out.push(',');
        match item {
            None => out.push_str("null"),
            Some(item) => out.push_str(&item.id.to_string()),
        }
        out.push(',');
        match item {
            None => out.push_str("null"),
            Some(item) => out.push_str(&item.amount.to_string()),
        }
        if index + 1 < withdraw.len() {
            out.push(':');
        }
    }
    out
}

fn parse_int(text: &str) -> Result<i32> {
    rs09_game_core::java_string::parse_i32(text).ok_or(Error::Number)
}

/// `GrandExchangeOffer.fromQuery(result)` of row `row`.
pub fn from_query<H: Host + ?Sized>(h: &mut H, rows: &Rows, row: usize) -> Result<Offer> {
    let o = Offer::new();
    let item = rows.int(row, "item_id")?;
    o.fields().item_id = item;
    let total = rows.int(row, "amount_total")?;
    o.fields().amount = total;
    let completed = rows.int(row, "amount_complete")?;
    o.fields().completed_amount = completed;
    let value = rows.int(row, "offered_value")?;
    o.fields().offered_value = value;
    let sell = rows.int(row, "is_sale")? == 1;
    o.fields().sell = sell;
    let state = OfferState::from_ordinal(rows.int(row, "offer_state")?)?;
    o.fields().state = state;
    let uid = rows.long(row, "uid")?;
    o.fields().uid = uid;
    let stamp = rows.long(row, "time_stamp")?;
    o.fields().time_stamp = stamp;
    if let Some(text) = rows.string(row, "withdraw_items")?
        && !text.is_empty()
    {
        for item in text.split(':') {
            if item.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = item.split(',').collect();
            let index = parse_int(tokens[0])?;
            let id_token = *tokens.get(1).ok_or(Error::ListBounds)?;
            if id_token == "null" {
                continue;
            }
            let id = parse_int(id_token)?;
            let amount = parse_int(tokens.get(2).ok_or(Error::ListBounds)?)?;
            let slot = usize::try_from(index)
                .ok()
                .filter(|i| *i < 2)
                .ok_or(Error::Bounds)?;
            o.fields().withdraw[slot] = Some(WithdrawItem { id, amount });
        }
    }
    let coins = rows.int(row, "total_coin_xc")?;
    o.fields().total_coin_exchange = coins;
    let player_uid = rows.int(row, "player_uid")?;
    o.fields().player_uid = player_uid;
    let index = rows.int(row, "slot_index")?;
    o.fields().index = index;
    let player = h.player_by_account(player_uid)?;
    o.fields().player = player;
    Ok(o)
}

/// `GrandExchangeOffer.fromBotQuery(result)` of row `row`.
pub fn from_bot_query<H: Host + ?Sized>(h: &mut H, rows: &Rows, row: usize) -> Result<Offer> {
    let o = Offer::new();
    o.fields().sell = true;
    let total = rows.int(row, "amount")?;
    o.fields().amount = total;
    o.fields().state = OfferState::Registered;
    let item = rows.int(row, "item_id")?;
    o.fields().item_id = item;
    let value = super::exchange::recommended_price(h, item, true)?;
    o.fields().offered_value = value;
    o.fields().is_bot = true;
    let now = h.now()?;
    o.fields().time_stamp = now;
    Ok(o)
}

/// `GrandExchangeOffer.createBotOffer(itemId, amount, sale)`.
pub fn create_bot_offer<H: Host + ?Sized>(
    h: &mut H,
    item: i32,
    total: i32,
    sale: bool,
) -> Result<Offer> {
    let o = Offer::new();
    {
        let mut f = o.fields();
        f.sell = sale;
        f.item_id = item;
        f.amount = total;
    }
    let value = super::exchange::recommended_price(h, item, true)?;
    let mut f = o.fields();
    f.offered_value = value;
    f.state = OfferState::Registered;
    f.is_bot = true;
    drop(f);
    Ok(o)
}

/// `cacheValue()`: the unsold items at the definition value (selling) or the
/// unspent coins (buying), plus the withdraw items at their values.
pub fn cache_value<H: Host + ?Sized>(h: &mut H, o: &Offer) -> Result<i32> {
    let definitions = h.definitions()?;
    let mut value = 0i32;
    if o.sell() {
        let unit = definitions.for_id(o.item_id()).read().value;
        value = value.wrapping_add(unit.wrapping_mul(amount_left(h, o)?));
    } else {
        let unit = o.offered_value();
        value = value.wrapping_add(unit.wrapping_mul(amount_left(h, o)?));
    }
    for item in o.withdraw().iter().flatten() {
        let unit = definitions.for_id(item.id).read().value;
        value = value.wrapping_add(unit.wrapping_mul(item.amount));
    }
    Ok(value)
}
