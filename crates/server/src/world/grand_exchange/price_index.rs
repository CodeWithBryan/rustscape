//! PriceIndex.kt and its PriceInfo: the `price_index` table (whether an item
//! may be traded, its guide value, the trade volume behind it).
use super::{Error, Host, Param, Result};

pub const SELECT_QUERY: &str = "SELECT * FROM price_index WHERE item_id = ?;";
pub const UPDATE_QUERY: &str = "UPDATE price_index SET value = ?, total_value = ?, unique_trades = ?, last_update = ? WHERE item_id = ?;";
pub const EXISTS_QUERY: &str = "SELECT EXISTS(SELECT 1 FROM price_index WHERE item_id = ?);";
pub const REMOVE_QUERY: &str = "DELETE FROM price_index WHERE item_id = ?;";
pub const INSERT_QUERY: &str = "INSERT INTO price_index (item_id, value, total_value, unique_trades, last_update) VALUES (?,?,?,?,?);";
pub const GET_VALUE_QUERY: &str = "SELECT value FROM price_index WHERE item_id = ?;";

/// `data class PriceInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceInfo {
    pub item_id: i32,
    pub current_value: i32,
    pub total_value: i64,
    pub unique_trades: i32,
    pub last_update: i64,
}

/// `canTrade(id)`: a `price_index` row exists.
pub fn can_trade<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<bool> {
    let db = h.exchange()?.db()?;
    let rows = db.prepare(EXISTS_QUERY)?.query(h, &[Param::Int(id)])?;
    Ok(!rows.is_empty() && rows.int_at(0, 1)? == 1)
}

/// `banItem(id)`.
pub fn ban_item<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<()> {
    let db = h.exchange()?.db()?;
    db.prepare(REMOVE_QUERY)?.execute(h, &[Param::Int(id)])
}

/// `allowItem(id)`: a new row at the high alchemy value unless one exists.
pub fn allow_item<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<()> {
    if can_trade(h, id)? {
        return Ok(());
    }
    let db = h.exchange()?.db()?;
    let insert = db.prepare(INSERT_QUERY)?;
    let alchemy = high_alchemy(h, id)?;
    let total = high_alchemy(h, id)?;
    insert.execute(
        h,
        &[
            Param::Int(id),
            Param::Int(alchemy),
            Param::Int(total),
            Param::Int(0),
            Param::Int(0),
        ],
    )
}

/// `itemDefinition(id).getAlchemyValue(true)`.
pub fn high_alchemy<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<i32> {
    let definitions = h.definitions()?;
    let definition = definitions.for_id(id);
    Ok(crate::inventory::item::definition_alchemy(
        &definition,
        true,
        &definitions,
    )?)
}

/// `getValue(id)`: the row's value, else the high alchemy value (read first).
pub fn get_value<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<i32> {
    let mut value = high_alchemy(h, id)?;
    let db = h.exchange()?.db()?;
    let rows = db.prepare(GET_VALUE_QUERY)?.query(h, &[Param::Int(id)])?;
    if !rows.is_empty() {
        value = rows.int_at(0, 1)?;
    }
    Ok(value)
}

/// `addTrade(id, amount, pricePerUnit)`: a running average over the trade
/// volume, reset to a tenth of the threshold of its price band.
pub fn add_trade<H: Host + ?Sized>(h: &mut H, id: i32, amount: i32, price: i32) -> Result<()> {
    let Some(old) = get_info(h, id)? else {
        return Ok(());
    };
    let mut info = old;
    let threshold = if price >= 1_000_000 {
        500
    } else if price >= 100_000 {
        1000
    } else if price >= 25_000 {
        2500
    } else {
        10000
    };
    info.total_value = info
        .total_value
        .wrapping_add(i64::from(amount.wrapping_mul(price)));
    info.unique_trades = info.unique_trades.wrapping_add(amount);
    info.last_update = h.now()?;
    if info.unique_trades == 0 {
        return Err(Error::Arithmetic);
    }
    info.current_value = info.total_value.wrapping_div(i64::from(info.unique_trades)) as i32;
    if info.unique_trades > threshold {
        let reset = (0.1 * f64::from(threshold)) as i32;
        info.unique_trades = reset;
        info.total_value = i64::from(reset).wrapping_mul(i64::from(info.current_value));
    }
    update_info(h, &info)
}

fn update_info<H: Host + ?Sized>(h: &mut H, info: &PriceInfo) -> Result<()> {
    let db = h.exchange()?.db()?;
    db.prepare(UPDATE_QUERY)?.execute(
        h,
        &[
            Param::Int(info.current_value),
            Param::Long(info.total_value),
            Param::Int(info.unique_trades),
            Param::Long(info.last_update),
            Param::Int(info.item_id),
        ],
    )
}

/// `getInfo(id)`: `PriceInfo.fromQuery` of the row (columns by position).
pub fn get_info<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<Option<PriceInfo>> {
    let db = h.exchange()?.db()?;
    let rows = db.prepare(SELECT_QUERY)?.query(h, &[Param::Int(id)])?;
    if rows.is_empty() {
        return Ok(None);
    }
    Ok(Some(PriceInfo {
        item_id: rows.int_at(0, 1)?,
        current_value: rows.int_at(0, 2)?,
        total_value: rows.long_at(0, 3)?,
        unique_trades: rows.int_at(0, 4)?,
        last_update: rows.long_at(0, 5)?,
    }))
}
